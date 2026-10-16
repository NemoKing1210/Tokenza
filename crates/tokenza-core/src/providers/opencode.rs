//! OpenCode Go: subscription meters from the OpenCode console API.
//!
//! The console authenticates with a browser session cookie, which the `opencode` CLI never
//! stores (its `auth.json` only holds the inference API key). Reading the browser's cookie jar
//! is off the table, so the user hands us the cookie and workspace id: typed into Tokenza's
//! provider settings, or through environment variables (the convention community trackers use).

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::{http, timestamp};
use crate::error::check_response;
use crate::{secrets, Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const STATUS_URL: &str = "https://opencode.ai/console/api/go/status";
const WORKSPACE_VAR: &str = "OPENCODE_GO_WORKSPACE_ID";
const COOKIE_VAR: &str = "OPENCODE_GO_AUTH_COOKIE";
const COOKIE_NAME: &str = "__Host-console_session";

pub struct OpenCode;

/// A value typed into Tokenza wins over the environment variable.
fn setting(field: &str, var: &str) -> Option<String> {
    secrets::get("opencode", field).or_else(|| {
        let v = std::env::var(var).ok()?.trim().to_string();
        (!v.is_empty()).then_some(v)
    })
}

/// A bare value is the session id; a full `name=value` pair is passed through as given.
fn cookie_header(raw: &str) -> String {
    if raw.contains('=') {
        raw.to_string()
    } else {
        format!("{COOKIE_NAME}={raw}")
    }
}

fn credentials() -> Option<(String, String)> {
    Some((
        setting("workspace_id", WORKSPACE_VAR)?,
        setting("cookie", COOKIE_VAR)?,
    ))
}

#[async_trait]
impl Provider for OpenCode {
    fn id(&self) -> &str {
        "opencode"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(300)
    }

    fn detect(&self) -> Option<Account> {
        credentials()?;
        Some(Account { label: "go".into() })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let (workspace, cookie) = credentials().ok_or(ProviderError::NotLoggedIn)?;

        let resp = http()?
            .get(STATUS_URL)
            .header("x-org-id", workspace)
            .header("Cookie", cookie_header(&cookie))
            .send()
            .await?;
        check_response(&resp)?;
        let body: Value = resp.json().await?;

        let windows = parse(&body);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged("no known usage meters".into()));
        }
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account: acc.label.clone(),
            windows,
            fetched_at: Utc::now(),
        })
    }
}

fn parse(body: &Value) -> Vec<UsageWindow> {
    let mut out = Vec::new();
    for (key, kind, label) in [
        ("fiveHour", WindowKind::Session5h, "5 h"),
        ("week", WindowKind::Weekly, "Week"),
        ("month", WindowKind::Monthly, "Month"),
    ] {
        let Some(m) = body.pointer(&format!("/access/meters/{key}")) else {
            continue;
        };
        // Both amounts share one unit (micro-cents), so only their ratio matters.
        let (Some(used), Some(limit)) = (
            m.get("usedMicroCents").and_then(Value::as_f64),
            m.get("limitMicroCents")
                .and_then(Value::as_f64)
                .filter(|l| *l > 0.0),
        ) else {
            continue;
        };
        out.push(UsageWindow {
            kind,
            label: label.into(),
            used_pct: Some((used / limit * 100.0) as f32),
            resets_at: m.get("resetsAt").and_then(timestamp),
            detail: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_three_meters() {
        let body = serde_json::json!({"access": {"meters": {
            "fiveHour": {"resetsAt": "2026-10-16T18:00:00Z", "limitMicroCents": 1200000000i64, "usedMicroCents": 300000000},
            "week": {"resetsAt": 1790000000000i64, "limitMicroCents": 3000000000i64, "usedMicroCents": 3000000000i64},
            "month": {"limitMicroCents": 6000000000i64, "usedMicroCents": 0}
        }}});
        let w = parse(&body);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[0].used_pct, Some(25.0));
        assert!(w[0].resets_at.is_some());
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_pct, Some(100.0));
        assert_eq!(w[2].kind, WindowKind::Monthly);
        assert_eq!(w[2].used_pct, Some(0.0));
        assert!(w[2].resets_at.is_none());
    }

    #[test]
    fn unrecognised_response_yields_no_windows() {
        assert!(parse(&serde_json::json!({"error": "no subscription"})).is_empty());
    }

    #[test]
    fn bare_cookie_gets_its_name() {
        assert_eq!(cookie_header("abc"), "__Host-console_session=abc");
        assert_eq!(cookie_header("auth=abc"), "auth=abc");
    }
}
