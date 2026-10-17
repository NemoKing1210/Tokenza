//! Antigravity: the CLI's OAuth login, model quota from Google's Code Assist backend.
//!
//! Written from community documentation of the private `retrieveUserQuota` method and not yet
//! checked against a signed-in account, so every assumption below degrades to `NotLoggedIn` or
//! `SchemaChanged` instead of wrong numbers.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use super::{body_json, home, http_no_redirect, number, rfc3339, timestamp};
use crate::error::check_response;
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const QUOTA_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";

pub struct Antigravity;

fn token_path() -> Option<PathBuf> {
    Some(
        home()?
            .join(".gemini")
            .join("antigravity-cli")
            .join("antigravity-oauth-token"),
    )
}

/// The access token and, when the file says so, the moment it stops working. The file is either a
/// JSON object or the bare token; a refresh token in it is never read.
fn read_token() -> Option<(String, Option<DateTime<Utc>>)> {
    let text = std::fs::read_to_string(token_path()?).ok()?;
    let (token, expires) = match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(o)) => {
            let token = ["access_token", "accessToken", "token"]
                .iter()
                .find_map(|k| o.get(*k)?.as_str())?
                .to_string();
            let expires = ["expiry_date", "expiry", "expires_at", "expiresAt"]
                .iter()
                .find_map(|k| o.get(*k))
                .and_then(timestamp);
            (token, expires)
        }
        Ok(Value::String(s)) => (s, None),
        _ => (text, None),
    };
    let token = token.trim().trim_matches('"').to_string();
    (!token.is_empty()).then_some((token, expires))
}

#[async_trait]
impl Provider for Antigravity {
    fn id(&self) -> &str {
        "antigravity"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(300)
    }

    fn detect(&self) -> Option<Account> {
        read_token()?;
        Some(Account {
            label: String::new(),
        })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        // Read per request and dropped afterwards; never stored or logged.
        let (token, expires) = read_token().ok_or(ProviderError::NotLoggedIn)?;
        if expires.is_some_and(|e| e <= Utc::now()) {
            return Err(ProviderError::TokenExpired);
        }
        let resp = http_no_redirect()?
            .post(QUOTA_URL)
            .bearer_auth(token)
            .json(&json!({}))
            .send()
            .await?;
        check_response(&resp)?;
        let body = body_json(resp).await?;

        let now = Utc::now();
        let windows = parse(&body, now);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged(
                "no known quota buckets".into(),
            ));
        }
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account: acc.label.clone(),
            windows,
            subscription_ends_at: None,
            fetched_at: now,
        })
    }
}

/// The window length is not in the response, only when it resets, so the time left tells a
/// short session from a daily, weekly or longer quota.
fn kind_for(resets_at: DateTime<Utc>, now: DateTime<Utc>) -> (WindowKind, &'static str) {
    let hours = (resets_at - now).num_minutes() as f64 / 60.0;
    match hours {
        h if h <= 6.0 => (WindowKind::Session5h, "5 h"),
        h if h <= 25.0 => (WindowKind::Daily, "Day"),
        h if h <= 8.0 * 24.0 => (WindowKind::Weekly, "Week"),
        _ => (WindowKind::Monthly, "Month"),
    }
}

/// One window per kind: the model that is closest to its limit, since a card has room for a few
/// meters and not one per model.
fn parse(body: &Value, now: DateTime<Utc>) -> Vec<UsageWindow> {
    let mut worst: Vec<UsageWindow> = Vec::new();
    let buckets = body.get("buckets").and_then(Value::as_array);
    for b in buckets.into_iter().flatten() {
        let Some(resets_at) = b.get("resetTime").and_then(rfc3339) else {
            continue;
        };
        let Some(remaining) = b.get("remainingFraction").and_then(number) else {
            continue;
        };
        let (kind, label) = kind_for(resets_at, now);
        let model = b.get("modelId").and_then(Value::as_str).unwrap_or("");
        let used = ((1.0 - remaining) * 100.0).clamp(0.0, 100.0) as f32;
        let w = UsageWindow {
            kind,
            label: if model.is_empty() {
                label.into()
            } else {
                format!("{label} · {model}")
            },
            used_pct: Some(used),
            resets_at: Some(resets_at),
            detail: None,
        };
        match worst.iter_mut().find(|e| e.kind == kind) {
            Some(e) if e.used_pct >= w.used_pct => {}
            Some(e) => *e = w,
            None => worst.push(w),
        }
    }
    worst.sort_by_key(|w| w.resets_at);
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn keeps_the_most_used_model_per_window_kind() {
        let body = json!({"buckets": [
            {"modelId": "model-a", "remainingFraction": 0.9, "resetTime": "2026-10-16T14:00:00Z", "tokenType": "REQUESTS"},
            {"modelId": "model-b", "remainingFraction": 0.25, "resetTime": "2026-10-16T14:00:00Z", "tokenType": "REQUESTS"},
            {"modelId": "model-a", "remainingFraction": 0.5, "resetTime": "2026-10-21T00:00:00Z", "tokenType": "REQUESTS"}
        ]});
        let w = parse(&body, at("2026-10-16T12:00:00Z"));
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[0].label, "5 h · model-b");
        assert_eq!(w[0].used_pct, Some(75.0));
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_pct, Some(50.0));
    }

    #[test]
    fn window_kind_follows_the_time_left() {
        let now = at("2026-10-16T12:00:00Z");
        assert_eq!(
            kind_for(at("2026-10-16T16:00:00Z"), now).0,
            WindowKind::Session5h
        );
        assert_eq!(
            kind_for(at("2026-10-17T10:00:00Z"), now).0,
            WindowKind::Daily
        );
        assert_eq!(
            kind_for(at("2026-10-22T12:00:00Z"), now).0,
            WindowKind::Weekly
        );
        assert_eq!(
            kind_for(at("2026-11-12T12:00:00Z"), now).0,
            WindowKind::Monthly
        );
    }

    #[test]
    fn skips_buckets_without_numbers_or_reset_time() {
        let body = json!({"buckets": [
            {"modelId": "a", "resetTime": "2026-10-16T14:00:00Z"},
            {"modelId": "b", "remainingFraction": 0.5},
            "junk"
        ]});
        assert!(parse(&body, at("2026-10-16T12:00:00Z")).is_empty());
        assert!(parse(&json!({}), at("2026-10-16T12:00:00Z")).is_empty());
    }
}
