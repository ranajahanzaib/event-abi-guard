//! Offline Ethereum event ABI normalization, structural comparison and consumer checks.
//!
//! ```
//! use eventabi_guard::{abi, diff};
//! let previous = abi::parse("[]")?;
//! let proposed = abi::parse("[]")?;
//! assert!(diff::compare_abis(&previous, &proposed).is_empty());
//! # Ok::<(), eventabi_guard::Error>(())
//! ```
pub mod abi;
pub mod consumer;
pub mod diff;
pub mod report;

/// Errors from untrusted inputs and local IO.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot access {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{path}: {source}")]
    Input {
        path: std::path::PathBuf,
        source: Box<Error>,
    },
    #[error("invalid ABI: {0}")]
    Abi(String),
    #[error("invalid manifest: {0}")]
    Manifest(String),
}
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn read(path: &std::path::Path) -> Result<String> {
    // Bound file memory use before parsing, including files that grow while read.
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(Error::Abi(format!(
            "{} exceeds 16 MiB input limit",
            path.display()
        )));
    }
    String::from_utf8(bytes).map_err(|_| Error::Abi(format!("{} is not UTF-8", path.display())))
}
