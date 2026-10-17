pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod commandcode;
pub mod cursor;
pub mod custom;
pub mod devin;
pub mod opencode;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::ProviderError;

pub(crate) fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Largest response body any provider reads; usage answers are a few kilobytes.
const MAX_BODY: usize = 1 << 20;

fn client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(10))
        // Polls are minutes apart, so a kept-alive socket would mostly sit idle.
        .pool_idle_timeout(Duration::from_secs(30))
        .user_agent("tokenza/0.1")
}

/// One client for the whole process: building one loads the system root certificates, which is
/// too costly to repeat on every poll. A failed build is retried on the next call.
fn shared(
    cell: &'static OnceLock<reqwest::Client>,
    build: fn() -> reqwest::ClientBuilder,
) -> Result<reqwest::Client, ProviderError> {
    if let Some(c) = cell.get() {
        return Ok(c.clone());
    }
    let c = build().build()?;
    Ok(cell.get_or_init(|| c).clone())
}

pub(crate) fn http() -> Result<reqwest::Client, ProviderError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    shared(&CLIENT, client_builder)
}

/// Same client, but a redirect is an error: user-defined endpoints must not forward our credentials elsewhere.
pub(crate) fn http_no_redirect() -> Result<reqwest::Client, ProviderError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    shared(&CLIENT, || {
        client_builder().redirect(reqwest::redirect::Policy::none())
    })
}

/// The response body, refused once it grows past [`MAX_BODY`] instead of being buffered whole.
async fn body_bytes(mut resp: reqwest::Response) -> Result<Vec<u8>, ProviderError> {
    let too_large = || ProviderError::SchemaChanged("response is too large".into());
    if resp.content_length().is_some_and(|n| n > MAX_BODY as u64) {
        return Err(too_large());
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if buf.len() + chunk.len() > MAX_BODY {
            return Err(too_large());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// The response body as JSON; a body that is not JSON means the endpoint changed, not that we are offline.
pub(crate) async fn body_json(resp: reqwest::Response) -> Result<Value, ProviderError> {
    serde_json::from_slice(&body_bytes(resp).await?)
        .map_err(|_| ProviderError::SchemaChanged("response is not JSON".into()))
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
