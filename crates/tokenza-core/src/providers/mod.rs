pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod commandcode;
pub mod cursor;
pub mod custom;
pub mod devin;
pub mod opencode;

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

/// Same client, but a redirect is an error: user-defined endpoints must not forward our credentials elsewhere.
pub(crate) fn http_no_redirect() -> Result<reqwest::Client, ProviderError> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("tokenza/0.1")
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

/// A JSON number, or a string holding one.
pub(crate) fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

/// A timestamp as RFC 3339 text or as epoch seconds / milliseconds (told apart by magnitude).
pub(crate) fn timestamp(v: &Value) -> Option<DateTime<Utc>> {
    if let Some(d) = v
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    {
        return Some(d.with_timezone(&Utc));
    }
    let n = number(v)? as i64;
    if n >= 1_000_000_000_000 {
        DateTime::from_timestamp_millis(n)
    } else {
        DateTime::from_timestamp(n, 0)
    }
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
