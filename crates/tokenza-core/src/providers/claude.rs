//! Claude Code: OAuth usage endpoint, the same one `/usage` calls.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::{home, http, read_json, rfc3339};
use crate::error::check_response;
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub struct Claude;

fn oauth() -> Option<Value> {
    let creds = read_json(&home()?.join(".claude").join(".credentials.json"))?;
    creds.get("claudeAiOauth").cloned()
}

#[async_trait]
impl Provider for Claude {
    fn id(&self) -> &str {
        "claude"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(180)
    }

    fn detect(&self) -> Option<Account> {
        let o = oauth()?;
        o.get("accessToken")?.as_str()?;
        let label = o
            .get("subscriptionType")
            .and_then(Value::as_str)
            .unwrap_or("claude.ai")
            .to_string();
        Some(Account { label })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        // Token is read per request and dropped afterwards; never stored.
        let o = oauth().ok_or(ProviderError::NotLoggedIn)?;
        let token = o
            .get("accessToken")
            .and_then(Value::as_str)
            .ok_or(ProviderError::NotLoggedIn)?;
        if let Some(exp_ms) = o.get("expiresAt").and_then(Value::as_i64) {
            if exp_ms <= Utc::now().timestamp_millis() {
                return Err(ProviderError::TokenExpired);
            }
        }

        let resp = http()?
            .get(USAGE_URL)
            .bearer_auth(token)
            .header("anthropic-beta", "oauth-2025-04-20")
            .send()
            .await?;
        check_response(&resp)?;
        let body: Value = resp.json().await?;

        let windows = parse(&body);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged(
                "no known usage windows".into(),
            ));
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
    const KEYS: [(&str, WindowKind, &str); 4] = [
        ("five_hour", WindowKind::Session5h, "5 h"),
        ("seven_day", WindowKind::Weekly, "Week"),
        ("seven_day_opus", WindowKind::Weekly, "Week · Opus"),
        ("seven_day_sonnet", WindowKind::Weekly, "Week · Sonnet"),
    ];
    KEYS.iter()
        .filter_map(|(key, kind, label)| {
            let w = body.get(*key)?;
            Some(UsageWindow {
                kind: *kind,
                label: (*label).into(),
                used_pct: Some(w.get("utilization")?.as_f64()? as f32),
                resets_at: w.get("resets_at").and_then(rfc3339),
                detail: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_and_skips_null() {
        let body = serde_json::json!({
            "five_hour": {"utilization": 37.0, "resets_at": "2026-10-16T15:00:00+00:00"},
            "seven_day": {"utilization": 12.5, "resets_at": null},
            "seven_day_opus": null
        });
        let w = parse(&body);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].used_pct, Some(37.0));
        assert!(w[0].resets_at.is_some());
        assert!(w[1].resets_at.is_none());
    }
}
