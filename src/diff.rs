//! Structural changes. Same-name relationships are explanatory candidates, never identities.
use crate::abi::{ContractAbi, Event, Parameter};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Category {
    EventAdded,
    EventRemoved,
    EventSignatureChanged,
    EventParameterTypeChanged,
    EventParameterAdded,
    EventParameterRemoved,
    EventIndexedChanged,
    EventParameterRenamed,
    EventParameterReordered,
    EventAnonymousChanged,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    Definite,
    Possible,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub category: Category,
    pub previous_event: Option<String>,
    pub proposed_event: Option<String>,
    pub field: Option<String>,
    pub previous_value: Option<String>,
    pub proposed_value: Option<String>,
    pub certainty: Certainty,
}
pub type AbiDiff = Vec<Change>;
fn change(
    category: Category,
    old: Option<&Event>,
    new: Option<&Event>,
    certainty: Certainty,
) -> Change {
    Change {
        category,
        previous_event: old.map(|e| e.signature.clone()),
        proposed_event: new.map(|e| e.signature.clone()),
        field: None,
        previous_value: None,
        proposed_value: None,
        certainty,
    }
}
fn property(
    out: &mut AbiDiff,
    category: Category,
    old: &Event,
    new: &Event,
    field: String,
    values: (String, String),
    certainty: &Certainty,
) {
    let mut c = change(category, Some(old), Some(new), certainty.clone());
    c.field = Some(field);
    c.previous_value = Some(values.0);
    c.proposed_value = Some(values.1);
    out.push(c);
}
fn parameters(
    out: &mut AbiDiff,
    events: (&Event, &Event),
    old: &[Parameter],
    new: &[Parameter],
    prefix: &str,
    certainty: &Certainty,
) {
    let (a, b) = events;
    // Only infer reorder from unique, nonempty names and an unchanged set of name/type pairs.
    let keys = |ps: &[Parameter]| {
        let mut v: Vec<_> = ps
            .iter()
            .map(|p| (p.name.clone(), p.canonical_type.clone()))
            .collect();
        v.sort();
        v
    };
    let unique = |ps: &[Parameter]| {
        ps.iter()
            .all(|p| !p.name.is_empty() && ps.iter().filter(|q| q.name == p.name).count() == 1)
    };
    let reordered = old.len() == new.len()
        && unique(old)
        && unique(new)
        && keys(old) == keys(new)
        && old.iter().zip(new).any(|(p, q)| p.name != q.name);
    // Indexed layout is positional, even when named parameters were reordered.
    for (i, (p, q)) in old.iter().zip(new).enumerate() {
        if p.indexed != q.indexed {
            property(
                out,
                Category::EventIndexedChanged,
                a,
                b,
                format!("{prefix}{i}"),
                (p.indexed.to_string(), q.indexed.to_string()),
                certainty,
            );
        }
    }
    if reordered {
        property(
            out,
            Category::EventParameterReordered,
            a,
            b,
            prefix.into(),
            (
                old.iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                new.iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            certainty,
        );
    }
    for (i, p) in old.iter().enumerate() {
        let field = format!("{prefix}{i}");
        let q = if reordered {
            new.iter().find(|q| q.name == p.name)
        } else {
            new.get(i)
        };
        if let Some(q) = q {
            if p.canonical_type != q.canonical_type {
                property(
                    out,
                    Category::EventParameterTypeChanged,
                    a,
                    b,
                    field.clone(),
                    (p.canonical_type.clone(), q.canonical_type.clone()),
                    certainty,
                );
            }
            if p.name != q.name {
                property(
                    out,
                    Category::EventParameterRenamed,
                    a,
                    b,
                    field.clone(),
                    (p.name.clone(), q.name.clone()),
                    certainty,
                );
            }
            parameters(
                out,
                events,
                &p.components,
                &q.components,
                &format!("{field}."),
                certainty,
            );
        } else {
            property(
                out,
                Category::EventParameterRemoved,
                a,
                b,
                field,
                (p.canonical_type.clone(), String::new()),
                certainty,
            );
        }
    }
    for (i, q) in new.iter().enumerate().skip(old.len()) {
        property(
            out,
            Category::EventParameterAdded,
            a,
            b,
            format!("{prefix}{i}"),
            (String::new(), q.canonical_type.clone()),
            certainty,
        );
    }
}
/// Return a sole unmatched same-name candidate, without claiming it is a replacement.
pub fn candidate<'a>(
    old: &Event,
    previous: &ContractAbi,
    proposed: &'a ContractAbi,
) -> Option<&'a Event> {
    let removed = previous
        .events
        .values()
        .filter(|e| e.name == old.name && !proposed.events.contains_key(&e.signature))
        .count();
    let mut added = proposed
        .events
        .values()
        .filter(|e| e.name == old.name && !previous.events.contains_key(&e.signature));
    let one = added.next()?;
    (removed == 1 && added.next().is_none()).then_some(one)
}
pub fn compare_abis(previous: &ContractAbi, proposed: &ContractAbi) -> AbiDiff {
    let mut out = Vec::new();
    for old in previous.events.values() {
        if let Some(new) = proposed.events.get(&old.signature) {
            if old.anonymous != new.anonymous {
                property(
                    &mut out,
                    Category::EventAnonymousChanged,
                    old,
                    new,
                    String::new(),
                    (old.anonymous.to_string(), new.anonymous.to_string()),
                    &Certainty::Definite,
                );
            }
            parameters(
                &mut out,
                (old, new),
                &old.inputs,
                &new.inputs,
                "",
                &Certainty::Definite,
            );
        } else {
            out.push(change(
                Category::EventRemoved,
                Some(old),
                None,
                Certainty::Definite,
            ));
            if let Some(new) = candidate(old, previous, proposed) {
                out.push(change(
                    Category::EventSignatureChanged,
                    Some(old),
                    Some(new),
                    Certainty::Possible,
                ));
                if old.anonymous != new.anonymous {
                    property(
                        &mut out,
                        Category::EventAnonymousChanged,
                        old,
                        new,
                        String::new(),
                        (old.anonymous.to_string(), new.anonymous.to_string()),
                        &Certainty::Possible,
                    );
                }
                parameters(
                    &mut out,
                    (old, new),
                    &old.inputs,
                    &new.inputs,
                    "",
                    &Certainty::Possible,
                );
            }
        }
    }
    for new in proposed.events.values() {
        if !previous.events.contains_key(&new.signature) {
            out.push(change(
                Category::EventAdded,
                None,
                Some(new),
                Certainty::Definite,
            ));
        }
    }
    out
}
