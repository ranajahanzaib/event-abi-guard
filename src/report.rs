//! Versioned report model shared by every formatter.
use crate::{
    abi::{ContractAbi, Event, keccak},
    diff::{self, Category, Certainty},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Breaking,
    Warning,
    Info,
}
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: String,
    pub rule: String,
    pub contract: String,
    pub consumer: Option<String>,
    pub severity: Severity,
    pub category: Category,
    pub certainty: Certainty,
    pub previous_event: Option<String>,
    pub proposed_event: Option<String>,
    pub previous_topic0: Option<String>,
    pub proposed_topic0: Option<String>,
    pub field: Option<String>,
    pub previous_value: Option<String>,
    pub proposed_value: Option<String>,
    pub explanation: String,
    pub remediation: String,
}
impl Finding {
    pub(crate) fn new(
        contract: &str,
        consumer: Option<&str>,
        rule: &str,
        severity: Severity,
        category: Category,
        old: Option<&Event>,
        new: Option<&Event>,
    ) -> Self {
        Self {
            id: String::new(),
            rule: rule.into(),
            contract: contract.into(),
            consumer: consumer.map(str::to_owned),
            severity,
            category,
            certainty: Certainty::Definite,
            previous_event: old.map(|e| e.signature.clone()),
            proposed_event: new.map(|e| e.signature.clone()),
            previous_topic0: old.and_then(|e| e.topic0.clone()),
            proposed_topic0: new.and_then(|e| e.topic0.clone()),
            field: None,
            previous_value: None,
            proposed_value: None,
            explanation: String::new(),
            remediation: "Review the ABI change and coordinate any affected consumers.".into(),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub version: &'static str,
}
#[derive(Debug, Serialize)]
pub struct Summary {
    pub contracts: usize,
    pub consumers: usize,
    pub events_examined: usize,
    pub breaking: usize,
    pub warnings: usize,
    pub info: usize,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub tool: Tool,
    pub summary: Summary,
    pub findings: Vec<Finding>,
}
impl Report {
    pub fn new(contracts: usize, consumers: usize) -> Self {
        Self {
            schema_version: 1,
            tool: Tool {
                name: "eventabi",
                version: env!("CARGO_PKG_VERSION"),
            },
            summary: Summary {
                contracts,
                consumers,
                events_examined: 0,
                breaking: 0,
                warnings: 0,
                info: 0,
            },
            findings: Vec::new(),
        }
    }
    pub fn add_diff(&mut self, contract: &str, old: &ContractAbi, new: &ContractAbi) {
        self.summary.events_examined += old.events.len() + new.events.len();
        for c in diff::compare_abis(old, new) {
            let a = c.previous_event.as_ref().and_then(|s| old.events.get(s));
            let b = c.proposed_event.as_ref().and_then(|s| new.events.get(s));
            let severity = if c.category == Category::EventAdded {
                Severity::Info
            } else {
                Severity::Warning
            };
            let mut f = Finding::new(contract, None, "EAG100", severity, c.category, a, b);
            f.field = c.field;
            f.previous_value = c.previous_value;
            f.proposed_value = c.proposed_value;
            f.certainty = c.certainty;
            f.explanation = if f.certainty == Certainty::Possible { "Possible same-name relationship; removal and addition remain the definite facts." } else { "Structural ABI change. This finding alone does not establish a declared-consumer violation." }.into();
            self.findings.push(f);
        }
    }
    pub fn finish(&mut self) {
        self.summary.breaking = 0;
        self.summary.warnings = 0;
        self.summary.info = 0;
        for f in &mut self.findings {
            // Length-delimited JSON avoids ambiguous concatenations in stable identities.
            let identity = serde_json::to_string(&(
                &f.contract,
                &f.consumer,
                &f.rule,
                &f.category,
                &f.previous_event,
                &f.proposed_event,
                &f.field,
                &f.previous_value,
                &f.proposed_value,
            ))
            .unwrap_or_default();
            f.id = keccak(&identity);
            match f.severity {
                Severity::Breaking => self.summary.breaking += 1,
                Severity::Warning => self.summary.warnings += 1,
                Severity::Info => self.summary.info += 1,
            }
        }
        self.findings.sort_by(|a, b| {
            (
                &a.contract,
                &a.previous_event,
                &a.proposed_event,
                &a.consumer,
                &a.rule,
                &a.field,
                &a.id,
            )
                .cmp(&(
                    &b.contract,
                    &b.previous_event,
                    &b.proposed_event,
                    &b.consumer,
                    &b.rule,
                    &b.field,
                    &b.id,
                ))
        });
    }
    pub fn human(&self) -> String {
        let s = &self.summary;
        let mut out = format!(
            "EventABI Guard\nContracts: {} | Events examined (both snapshots): {} | Consumers: {}\nBreaking: {} | Warning: {} | Info: {}\n",
            s.contracts, s.events_examined, s.consumers, s.breaking, s.warnings, s.info
        );
        if self.findings.is_empty() {
            out.push_str("No ABI changes or consumer violations.\n");
        }
        for f in &self.findings {
            out.push_str(&format!("\n[{}] {:?} {:?} ({:?})\nContract: {} | Consumer: {}\nEvent: {} -> {}\nTopic0: {} -> {}\nField: {} | {} -> {}\n{}\nFix: {}\n", f.rule,f.severity,f.category,f.certainty,plain(&f.contract),plain(f.consumer.as_deref().unwrap_or("none")),plain(f.previous_event.as_deref().unwrap_or("none")),plain(f.proposed_event.as_deref().unwrap_or("none")),f.previous_topic0.as_deref().unwrap_or("none"),f.proposed_topic0.as_deref().unwrap_or("none"),plain(f.field.as_deref().unwrap_or("none")),plain(f.previous_value.as_deref().unwrap_or("none")),plain(f.proposed_value.as_deref().unwrap_or("none")),f.explanation,f.remediation));
        }
        out
    }
    pub fn markdown(&self) -> String {
        let s = &self.summary;
        let mut out = format!(
            "# EventABI Guard\n\nResult: **{}**\n\nContracts: {} · Consumers: {} · Breaking: {} · Warnings: {} · Info: {}\n\n",
            if s.breaking > 0 {
                "breaking changes"
            } else {
                "no declared consumer violations"
            },
            s.contracts,
            s.consumers,
            s.breaking,
            s.warnings,
            s.info
        );
        if self.findings.is_empty() {
            out.push_str("No ABI changes.\n");
        }
        for f in &self.findings {
            out.push_str(&format!("## {} — {:?}\n\nContract: {} · Consumer: {}\n\nEvent: {} → {}\n\nCategory: {:?} · Certainty: {:?} · Field: {}\n\nTopic0: {} → {}\n\n{}\n\nRemediation: {}\n\n",f.rule,f.severity,markdown_escape(&f.contract),markdown_escape(f.consumer.as_deref().unwrap_or("none")),markdown_escape(f.previous_event.as_deref().unwrap_or("none")),markdown_escape(f.proposed_event.as_deref().unwrap_or("none")),f.category,f.certainty,markdown_escape(f.field.as_deref().unwrap_or("none")),f.previous_topic0.as_deref().unwrap_or("none"),f.proposed_topic0.as_deref().unwrap_or("none"),markdown_escape(&f.explanation),markdown_escape(&f.remediation)));
        }
        out
    }
    /// Workflow commands contain no invented source coordinates. Emit separately from JSON stdout.
    pub fn github(&self) -> String {
        self.findings
            .iter()
            .map(|f| {
                format!(
                    "::{} title={}::{}\n",
                    match f.severity {
                        Severity::Breaking => "error",
                        Severity::Warning => "warning",
                        Severity::Info => "notice",
                    },
                    workflow_escape(&f.rule, true),
                    workflow_escape(
                        &format!(
                            "{} / {} / {}: {}",
                            f.contract,
                            f.consumer.as_deref().unwrap_or("no declared consumer"),
                            f.previous_event
                                .as_deref()
                                .or(f.proposed_event.as_deref())
                                .unwrap_or("event"),
                            f.explanation
                        ),
                        false
                    )
                )
            })
            .collect()
    }
}
fn plain(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
fn markdown_escape(s: &str) -> String {
    let mut out = String::new();
    for c in plain(s).chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' => {
                out.push('\\');
                out.push(c)
            }
            _ => out.push(c),
        }
    }
    out
}
fn workflow_escape(s: &str, metadata: bool) -> String {
    let s = s
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A");
    if metadata {
        s.replace(':', "%3A").replace(',', "%2C")
    } else {
        s
    }
}
