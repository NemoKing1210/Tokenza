//! Cursor: session token from the editor's `state.vscdb`, usage from the web API.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use super::{http, rfc3339};
use crate::error::check_response;
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const USAGE_URL: &str = "https://cursor.com/api/usage-summary";

pub struct Cursor;

fn db_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let p = PathBuf::from(appdata)
        .join("Cursor")
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    p.exists().then_some(p)
}

/// Reads the access token read-only; the editor keeps the file open.
fn access_token() -> Option<String> {
    let conn = Connection::open_with_flags(
        db_path()?,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    conn.busy_timeout(Duration::from_secs(2)).ok()?;
    let token: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken'",
            [],
            |r| r.get(0),
        )
        .ok()?;
    let token = token.trim().trim_matches('"').to_string();
    (!token.is_empty()).then_some(token)
}

/// The web API wants `<userId>%3A%3A<jwt>`; the user id is the JWT `sub` tail.
fn session_cookie(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    let sub = claims.get("sub")?.as_str()?;
    let user_id = sub.rsplit('|').next()?;
    Some(format!("WorkosCursorSessionToken={user_id}%3A%3A{jwt}"))
}

#[async_trait]
impl Provider for Cursor {
    fn id(&self) -> &str {
        "cursor"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(300)
    }

    fn detect(&self) -> Option<Account> {
        access_token()?;
        Some(Account {
            label: "cursor".into(),
        })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let jwt = access_token().ok_or(ProviderError::NotLoggedIn)?;
        let cookie = session_cookie(&jwt)
            .ok_or_else(|| ProviderError::SchemaChanged("unreadable access token".into()))?;

        let resp = http()?
            .get(USAGE_URL)
            .header("Cookie", cookie)
            .send()
            .await?;
        check_response(&resp)?;
        let body: Value = resp.json().await?;

        let windows = parse(&body);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged("no known usage fields".into()));
        }
        let account = body
            .get("membershipType")
            .and_then(Value::as_str)
            .unwrap_or(&acc.label)
            .to_string();
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account,
            windows,
            fetched_at: Utc::now(),
        })
    }
}

fn parse(body: &Value) -> Vec<UsageWindow> {
    let resets_at = body.get("billingCycleEnd").and_then(rfc3339);
    let plan = body.pointer("/individualUsage/plan");

    // Prefer the server-computed percentage; fall back to used / limit.
    let pct = plan
        .and_then(|p| p.get("totalPercentUsed").and_then(Value::as_f64))
        .or_else(|| {
            let p = plan?;
            let used = p.get("used")?.as_f64()?;
            let limit = p.get("limit")?.as_f64().filter(|l| *l > 0.0)?;
            Some(used / limit * 100.0)
        });

    pct.map(|pct| UsageWindow {
        kind: WindowKind::Monthly,
        label: "Included usage".into(),
        used_pct: Some(pct as f32),
        resets_at,
        detail: None,
    })
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_uses_sub_tail() {
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"sub":"auth0|user_123"}"#);
        let jwt = format!("h.{payload}.s");
        let c = session_cookie(&jwt).unwrap();
        assert!(c.starts_with("WorkosCursorSessionToken=user_123%3A%3A"));
    }

    #[test]
    fn falls_back_to_used_over_limit() {
        let body = serde_json::json!({
            "billingCycleEnd": "2026-11-01T00:00:00.000Z",
            "individualUsage": {"plan": {"used": 500, "limit": 2000}}
        });
        let w = parse(&body);
        assert_eq!(w[0].used_pct, Some(25.0));
        assert!(w[0].resets_at.is_some());
    }
}
