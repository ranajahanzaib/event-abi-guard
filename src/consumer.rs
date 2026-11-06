//! Strict manifest validation and consumer-specific compatibility evaluation.
use crate::{
    Error, Result,
    abi::{self, ContractAbi, Event, Parameter},
    diff::{self, Category, Certainty},
    report::{Finding, Report, Severity},
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub contracts: Vec<Contract>,
    pub consumers: Vec<Consumer>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub id: String,
    pub old: PathBuf,
    pub new: PathBuf,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumer {
    pub id: String,
    pub description: Option<String>,
    pub contract: String,
    pub events: Vec<Dependency>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub signature: String,
    pub mode: Mode,
    #[serde(default)]
    pub fields: Vec<Field>,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Decoded,
    TopicOnly,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: Option<String>,
    pub position: Option<usize>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub indexed: Option<bool>,
}
/// A fully validated manifest with both ABI snapshots loaded. Paths resolve beside the manifest.
pub struct ValidatedManifest {
    manifest: Manifest,
    abis: BTreeMap<String, (ContractAbi, ContractAbi)>,
}
fn invalid(s: impl Into<String>) -> Error {
    Error::Manifest(s.into())
}
fn field<'a>(f: &Field, inputs: &'a [Parameter]) -> Option<&'a Parameter> {
    match f.position {
        Some(i) => inputs.get(i),
        None => {
            let name = f.name.as_ref()?;
            let mut matches = inputs.iter().filter(|p| &p.name == name);
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        }
    }
}
pub fn load(path: &Path) -> Result<ValidatedManifest> {
    let manifest: Manifest =
        serde_saphyr::from_str(&crate::read(path)?).map_err(|e| invalid(e.to_string()))?;
    validate(manifest, path.parent().unwrap_or_else(|| Path::new(".")))
}
pub fn validate(manifest: Manifest, base: &Path) -> Result<ValidatedManifest> {
    if manifest.version != 1 {
        return Err(invalid("supported version is 1"));
    }
    let mut abis = BTreeMap::new();
    for c in &manifest.contracts {
        if c.id.trim().is_empty() || abis.contains_key(&c.id) {
            return Err(invalid(format!(
                "empty or duplicate contract id `{}`",
                c.id
            )));
        }
        abis.insert(
            c.id.clone(),
            (
                abi::load(&base.join(&c.old))?,
                abi::load(&base.join(&c.new))?,
            ),
        );
    }
    let mut ids = BTreeSet::new();
    for c in &manifest.consumers {
        if c.id.trim().is_empty() || !ids.insert(&c.id) {
            return Err(invalid(format!(
                "empty or duplicate consumer id `{}`",
                c.id
            )));
        }
        let (old, _) = abis.get(&c.contract).ok_or_else(|| {
            invalid(format!(
                "consumer {}: unknown contract {}",
                c.id, c.contract
            ))
        })?;
        let mut events = BTreeSet::new();
        for dep in &c.events {
            let context = format!("consumer {} event {}", c.id, dep.signature);
            abi::validate_signature(&dep.signature)
                .map_err(|e| invalid(format!("{context}: {e}")))?;
            if !events.insert(&dep.signature) {
                return Err(invalid(format!("{context}: duplicate event dependency")));
            }
            let event = old
                .events
                .get(&dep.signature)
                .ok_or_else(|| invalid(format!("{context}: event absent from baseline")))?;
            if dep.mode == Mode::TopicOnly && (!dep.fields.is_empty() || event.anonymous) {
                return Err(invalid(format!(
                    "{context}: topic_only requires a non-anonymous event and no fields"
                )));
            }
            let mut positions = BTreeSet::new();
            for f in &dep.fields {
                if f.name.is_none() && f.position.is_none()
                    || f.name.as_ref().is_some_and(|s| s.is_empty())
                {
                    return Err(invalid(format!(
                        "{context}: field requires nonempty name or zero-based position"
                    )));
                }
                let p = field(f,&event.inputs).ok_or_else(||invalid(format!("{context}: field absent, ambiguous, or position out of range in baseline: {f:?}")))?;
                if !positions.insert(p.position) {
                    return Err(invalid(format!(
                        "{context}: duplicate field expectation at {}",
                        p.position
                    )));
                }
                if f.name.as_ref().is_some_and(|s| s != &p.name)
                    || f.kind.as_ref().is_some_and(|s| s != &p.canonical_type)
                    || f.indexed.is_some_and(|v| v != p.indexed)
                {
                    return Err(invalid(format!(
                        "{context}: field expectations contradict baseline: {f:?}"
                    )));
                }
            }
        }
    }
    Ok(ValidatedManifest { manifest, abis })
}
fn names_changed(a: &[Parameter], b: &[Parameter]) -> bool {
    a.iter()
        .zip(b)
        .any(|(p, q)| p.name != q.name || names_changed(&p.components, &q.components))
}
fn finding_value(event: &Event, category: Category, location: Option<&String>) -> Option<String> {
    match category {
        Category::EventRemoved => Some(event.signature.clone()),
        Category::EventAnonymousChanged => Some(event.anonymous.to_string()),
        Category::EventIndexedChanged => Some(
            event
                .inputs
                .iter()
                .map(|p| p.indexed.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        _ => location.and_then(|name| {
            let p = event
                .inputs
                .iter()
                .find(|p| &p.name == name)
                .or_else(|| name.parse::<usize>().ok().and_then(|i| event.inputs.get(i)))?;
            Some(format!(
                "{}: {} at position {} (indexed={})",
                p.name, p.canonical_type, p.position, p.indexed
            ))
        }),
    }
}
fn violation(
    report: &mut Report,
    context: (&str, &str, &Event, Option<&Event>),
    rule: &str,
    category: Category,
    location: Option<String>,
    message: &str,
) {
    let (contract, consumer, old, new) = context;
    let mut f = Finding::new(
        contract,
        Some(consumer),
        rule,
        Severity::Breaking,
        category,
        Some(old),
        new,
    );
    f.previous_value = finding_value(old, category, location.as_ref());
    f.proposed_value = new.and_then(|event| finding_value(event, category, location.as_ref()));
    f.field = location;
    f.explanation = message.into();
    f.remediation =
        "Preserve the baseline event or migrate this consumer before deploying the change.".into();
    report.findings.push(f);
}
/// Analyze already validated dependencies. Missing old dependencies are configuration errors.
pub fn check(validated: &ValidatedManifest) -> Report {
    let mut report = Report::new(validated.abis.len(), validated.manifest.consumers.len());
    for (contract, (old, new)) in &validated.abis {
        report.add_diff(contract, old, new);
    }
    for consumer in &validated.manifest.consumers {
        let Some((old, new)) = validated.abis.get(&consumer.contract) else {
            continue;
        };
        for dep in &consumer.events {
            let Some(a) = old.events.get(&dep.signature) else {
                continue;
            };
            let b = new.events.get(&dep.signature);
            let mut emit = |rule, category, location, message| {
                violation(
                    &mut report,
                    (&consumer.contract, &consumer.id, a, b),
                    rule,
                    category,
                    location,
                    message,
                )
            };
            let Some(b) = b else {
                let possible = diff::candidate(a, old, new);
                emit(
                    "EAG001",
                    Category::EventRemoved,
                    None,
                    "Required baseline signature is absent. Existing topic filters and ABI decoders cannot consume it under this proposed ABI.",
                );
                if let Some(candidate) = possible {
                    let mut f = Finding::new(
                        &consumer.contract,
                        Some(&consumer.id),
                        "EAG002",
                        Severity::Warning,
                        Category::EventSignatureChanged,
                        Some(a),
                        Some(candidate),
                    );
                    f.certainty = Certainty::Possible;
                    f.explanation = "A sole unmatched same-name event may be a replacement; this relationship is uncertain. The required signature removal is definite (EAG001).".into();
                    report.findings.push(f);
                }
                continue;
            };
            if a.anonymous != b.anonymous {
                emit(
                    "EAG008",
                    Category::EventAnonymousChanged,
                    None,
                    "Anonymous status changed, altering the emitted topic layout.",
                );
            }
            if dep.mode == Mode::TopicOnly {
                continue;
            }
            if a.inputs
                .iter()
                .zip(&b.inputs)
                .any(|(p, q)| p.indexed != q.indexed)
            {
                emit(
                    "EAG003",
                    Category::EventIndexedChanged,
                    None,
                    "Indexed layout changed; the baseline decoder may read topics and data incorrectly, including undeclared fields.",
                );
            }
            for f in &dep.fields {
                let Some(p) = field(f, &a.inputs) else {
                    continue;
                };
                let q = field(f, &b.inputs);
                let location = Some(f.name.clone().unwrap_or_else(|| p.position.to_string()));
                match q {
                    None => {
                        let renamed = b
                            .inputs
                            .get(p.position)
                            .is_some_and(|q| q.canonical_type == p.canonical_type);
                        emit(
                            if renamed { "EAG006" } else { "EAG004" },
                            if renamed {
                                Category::EventParameterRenamed
                            } else {
                                Category::EventParameterRemoved
                            },
                            location,
                            "Required named field no longer resolves uniquely. Name-based dependencies cannot be rebound by position.",
                        );
                    }
                    Some(q) => {
                        if p.canonical_type != q.canonical_type {
                            emit(
                                "EAG005",
                                Category::EventParameterTypeChanged,
                                location.clone(),
                                "Required field has a different canonical type.",
                            );
                        }
                        if p.position != q.position {
                            emit(
                                "EAG007",
                                Category::EventParameterReordered,
                                location.clone(),
                                "Required field moved to another position.",
                            );
                        }
                        if f.name.as_ref().is_some_and(|name| name != &q.name) {
                            emit(
                                "EAG006",
                                Category::EventParameterRenamed,
                                location.clone(),
                                "Explicitly required field name changed at this position.",
                            );
                        }
                        // A named tuple binding includes its decoded component property names.
                        if f.name.is_some() && names_changed(&p.components, &q.components) {
                            emit(
                                "EAG006",
                                Category::EventParameterRenamed,
                                location,
                                "Required tuple component names changed.",
                            );
                        }
                    }
                }
            }
        }
    }
    report.finish();
    report
}
