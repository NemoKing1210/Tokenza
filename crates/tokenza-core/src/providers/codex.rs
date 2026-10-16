//! Codex: ChatGPT backend usage endpoint, with the login from `~/.codex/auth.json`.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{home, http, read_json};
use crate::error::check_response;
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

pub struct Codex;

fn tokens() -> Option<Value> {
    let auth = read_json(&home()?.join(".codex").join("auth.json"))?;
    auth.get("tokens").cloned()
}

#[async_trait]
impl Provider for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(180)
    }

    fn detect(&self) -> Option<Account> {
        tokens()?.get("access_token")?.as_str()?;
        Some(Account {
            label: "chatgpt".into(),
        })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let t = tokens().ok_or(ProviderError::NotLoggedIn)?;
        let token = t
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or(ProviderError::NotLoggedIn)?;

        let mut req = http()?.get(USAGE_URL).bearer_auth(token);
        if let Some(id) = t.get("account_id").and_then(Value::as_str) {
            req = req.header("ChatGPT-Account-Id", id);
        }
        let resp = req.send().await?;
        check_response(&resp)?;
        let body: Value = resp.json().await?;

        let windows = parse(&body);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged(
                "no known usage windows".into(),
            ));
        }
        let account = body
            .get("plan_type")
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
    let mut out = Vec::new();
    for key in ["primary_window", "secondary_window"] {
        let Some(w) = body.pointer(&format!("/rate_limit/{key}")) else {
            continue;
        };
        let Some(pct) = w.get("used_percent").and_then(Value::as_f64) else {
            continue;
        };
        // Window length tells the 5-hour session from the weekly one.
        let secs = w
            .get("limit_window_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let (kind, label) = if secs > 0 && secs <= 6 * 3600 {
            (WindowKind::Session5h, "5 h")
        } else {
            (WindowKind::Weekly, "Week")
        };
        let resets_at = w
            .get("reset_at")
            .and_then(Value::as_i64)
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .or_else(|| {
                let after = w.get("reset_after_seconds").and_then(Value::as_i64)?;
                Some(Utc::now() + chrono::Duration::seconds(after))
            });
        out.push(UsageWindow {
            kind,
            label: label.into(),
            used_pct: Some(pct as f32),
            resets_at,
            detail: None,
        });
    }

    if let Some(c) = body.get("credits") {
        let has = c
            .get("has_credits")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let unlimited = c.get("unlimited").and_then(Value::as_bool).unwrap_or(false);
        if has || unlimited {
            let balance = match c.get("balance") {
                Some(Value::String(s)) => s.clone(),
                Some(v) if v.is_number() => v.to_string(),
                _ => String::new(),
            };
            out.push(UsageWindow {
                kind: WindowKind::Credits,
                label: "Credits".into(),
                used_pct: None,
                resets_at: None,
                detail: Some(if unlimited {
                    "unlimited".into()
                } else {
                    balance
                }),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_windows_by_length() {
        let body = serde_json::json!({
            "plan_type": "plus",
            "rate_limit": {
                "primary_window": {"used_percent": 20, "limit_window_seconds": 18000, "reset_at": 1790000000},
                "secondary_window": {"used_percent": 55.5, "limit_window_seconds": 604800, "reset_after_seconds": 3600}
            },
            "credits": {"has_credits": true, "unlimited": false, "balance": "4.20"}
        });
        let w = parse(&body);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert!(w[1].resets_at.is_some());
        assert_eq!(w[2].detail.as_deref(), Some("4.20"));
    }
}
