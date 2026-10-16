pub mod claude;
pub mod codex;
pub mod cursor;

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::ProviderError;

pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub(crate) fn http() -> Result<reqwest::Client, ProviderError> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("tokenza/0.1")
        .build()?)
}

/// Reads and parses a JSON file; any failure means "no usable login".
pub(crate) fn read_json(path: &std::path::Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub(crate) fn rfc3339(v: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.as_str()?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}
