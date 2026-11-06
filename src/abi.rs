//! Bounded ABI type grammar and canonical Ethereum event definitions.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Parameter {
    pub name: String,
    pub canonical_type: String,
    pub indexed: bool,
    pub position: usize,
    pub components: Vec<Parameter>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub name: String,
    pub signature: String,
    pub topic0: Option<String>,
    pub anonymous: bool,
    pub inputs: Vec<Parameter>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContractAbi {
    pub events: BTreeMap<String, Event>,
}
#[derive(Deserialize)]
struct RawEvent {
    name: String,
    inputs: Vec<RawParameter>,
    anonymous: bool,
}
#[derive(Deserialize)]
struct RawParameter {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    indexed: Option<bool>,
    components: Option<Vec<RawParameter>>,
}
fn invalid(message: impl Into<String>) -> Error {
    Error::Abi(message.into())
}
pub fn identifier(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$'))
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
}
/// Lowercase Ethereum Keccak-256, deliberately not FIPS SHA3-256.
pub fn keccak(value: &str) -> String {
    let digest = Keccak256::digest(value.as_bytes());
    let mut out = String::from("0x");
    use std::fmt::Write;
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
fn scalar(s: &str) -> Result<String> {
    match s {
        "address" | "bool" | "string" | "bytes" | "function" => Ok(s.into()),
        "uint" => Ok("uint256".into()),
        "int" => Ok("int256".into()),
        _ => {
            for prefix in ["uint", "int", "bytes"] {
                if let Some(n) = s.strip_prefix(prefix)
                    && let Ok(size) = n.parse::<usize>()
                {
                    let valid = if prefix == "bytes" {
                        (1..=32).contains(&size)
                    } else {
                        (8..=256).contains(&size) && size % 8 == 0
                    };
                    if valid && n == size.to_string() {
                        return Ok(s.into());
                    }
                }
            }
            Err(invalid(format!("unsupported or malformed type `{s}`")))
        }
    }
}
fn array_suffix(s: &str) -> Result<()> {
    let mut rest = s;
    let mut dimensions = 0;
    while !rest.is_empty() {
        dimensions += 1;
        if dimensions > 32 || !rest.starts_with('[') {
            return Err(invalid("malformed or excessively nested array"));
        }
        let end = rest.find(']').ok_or_else(|| invalid("unclosed array"))?;
        let len = &rest[1..end];
        if !len.is_empty()
            && (!len.bytes().all(|b| b.is_ascii_digit())
                || len.starts_with('0')
                || len.parse::<u64>().is_err())
        {
            return Err(invalid("invalid fixed array length"));
        }
        rest = &rest[end + 1..];
    }
    Ok(())
}
fn parameter(raw: RawParameter, position: usize, depth: usize) -> Result<Parameter> {
    if depth > 24 {
        return Err(invalid("tuple nesting exceeds 24"));
    }
    if !raw.name.is_empty() && !identifier(&raw.name) {
        return Err(invalid("invalid parameter name"));
    }
    let split = raw.kind.find('[').unwrap_or(raw.kind.len());
    let (base, suffix) = raw.kind.split_at(split);
    array_suffix(suffix)?;
    let components = if base == "tuple" {
        raw.components
            .ok_or_else(|| invalid("tuple requires components"))?
            .into_iter()
            .enumerate()
            .map(|(i, p)| parameter(p, i, depth + 1))
            .collect::<Result<Vec<_>>>()?
    } else {
        if raw.components.is_some() {
            return Err(invalid("components only allowed on tuple types"));
        }
        Vec::new()
    };
    if depth > 0 && raw.indexed.is_some() {
        return Err(invalid("tuple components cannot declare indexed"));
    }
    let canonical = if base == "tuple" {
        format!(
            "({})",
            components
                .iter()
                .map(|p| p.canonical_type.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )
    } else {
        scalar(base)?
    };
    Ok(Parameter {
        name: raw.name,
        canonical_type: format!("{canonical}{suffix}"),
        indexed: if depth == 0 {
            raw.indexed
                .ok_or_else(|| invalid("event input requires indexed boolean"))?
        } else {
            false
        },
        position,
        components,
    })
}
/// Parse a raw ABI array or a compiler artifact containing an `abi` array.
pub fn parse(input: &str) -> Result<ContractAbi> {
    let value: serde_json::Value = serde_json::from_str(input)?;
    let items = value
        .as_array()
        .or_else(|| value.get("abi").and_then(|v| v.as_array()))
        .ok_or_else(|| invalid("expected an ABI array or object with an abi array"))?;
    let mut abi = ContractAbi::default();
    for (index, item) in items.iter().enumerate() {
        let kind = item
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| invalid(format!("ABI item {index} requires type")))?;
        if ![
            "event",
            "function",
            "constructor",
            "error",
            "fallback",
            "receive",
        ]
        .contains(&kind)
        {
            return Err(invalid(format!("unknown ABI item type `{kind}`")));
        }
        if kind != "event" {
            continue;
        }
        let raw: RawEvent = serde_json::from_value(item.clone())?;
        if !identifier(&raw.name) {
            return Err(invalid("invalid event name"));
        }
        let inputs = raw
            .inputs
            .into_iter()
            .enumerate()
            .map(|(i, p)| parameter(p, i, 0))
            .collect::<Result<Vec<_>>>()?;
        if inputs.iter().filter(|p| p.indexed).count() > if raw.anonymous { 4 } else { 3 } {
            return Err(invalid("event exceeds indexed topic limit"));
        }
        let signature = format!(
            "{}({})",
            raw.name,
            inputs
                .iter()
                .map(|p| p.canonical_type.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        let event = Event {
            name: raw.name,
            topic0: (!raw.anonymous).then(|| keccak(&signature)),
            signature: signature.clone(),
            anonymous: raw.anonymous,
            inputs,
        };
        if abi.events.insert(signature.clone(), event).is_some() {
            return Err(invalid(format!("duplicate event `{signature}`")));
        }
    }
    Ok(abi)
}
pub fn load(path: &Path) -> Result<ContractAbi> {
    parse(&crate::read(path)?).map_err(|source| Error::Input {
        path: path.into(),
        source: Box::new(source),
    })
}

/// Validate canonical manifest signatures, including nested tuples and arrays.
pub fn validate_signature(s: &str) -> Result<()> {
    fn ty(s: &str, depth: usize) -> Result<()> {
        if depth > 24 {
            return Err(invalid("signature nesting exceeds 24"));
        }
        let suffix_start = if s.starts_with('(') {
            let mut level = 0;
            let mut end = None;
            for (i, c) in s.char_indices() {
                if c == '(' {
                    level += 1;
                }
                if c == ')' {
                    level -= 1;
                    if level == 0 {
                        end = Some(i);
                        break;
                    }
                }
            }
            let end = end.ok_or_else(|| invalid("unclosed tuple signature"))?;
            list(&s[1..end], depth + 1)?;
            end + 1
        } else {
            let end = s.find('[').unwrap_or(s.len());
            if scalar(&s[..end])? != s[..end] {
                return Err(invalid("manifest signatures must use canonical types"));
            }
            end
        };
        array_suffix(&s[suffix_start..])
    }
    fn list(s: &str, depth: usize) -> Result<()> {
        if s.is_empty() {
            return Ok(());
        }
        let mut level = 0i32;
        let mut start = 0;
        for (i, c) in s.char_indices() {
            match c {
                '(' => level += 1,
                ')' => {
                    level -= 1;
                    if level < 0 {
                        return Err(invalid("unbalanced signature"));
                    }
                }
                ',' if level == 0 => {
                    ty(&s[start..i], depth)?;
                    start = i + 1;
                }
                _ => {}
            }
        }
        if level != 0 {
            return Err(invalid("unbalanced signature"));
        }
        ty(&s[start..], depth)
    }
    let open = s
        .find('(')
        .ok_or_else(|| invalid("event signature requires parentheses"))?;
    if !identifier(&s[..open]) || !s.ends_with(')') {
        return Err(invalid("invalid event signature"));
    }
    list(&s[open + 1..s.len() - 1], 0)
}
