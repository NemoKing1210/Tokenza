//! Devin (CLI and desktop): the key from `credentials.toml`, daily and weekly quota from the
//! same seat-management service the app itself calls.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};

use super::{body_json, http_no_redirect, number, timestamp};
use crate::error::check_response;
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

/// The only origin the key is ever sent to; `api_server_url` in the credentials file is ignored
/// unless it names this one, so an edited file cannot redirect the key elsewhere.
const ORIGIN: &str = "https://server.codeium.com";
const STATUS_PATH: &str = "/exa.seat_management_pb.SeatManagementService/GetUserStatus";

pub struct Devin;

fn credentials_path() -> Option<PathBuf> {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(
            PathBuf::from(appdata)
                .join("Devin")
                .join("credentials.toml"),
        );
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(super::home()?.join(".local").join("share")))?;
    Some(data.join("devin").join("credentials.toml"))
}

/// A top-level `key = "value"` line of a TOML file. The file holds a handful of flat strings, so a
/// line scan avoids a TOML parser dependency.
fn toml_string(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            return None;
        }
        let v = v.trim();
        let quote = v.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let inner = v[1..].split_once(quote)?.0;
        Some(inner.to_string())
    })
}

fn api_key() -> Option<String> {
    let text = std::fs::read_to_string(credentials_path()?).ok()?;
    let key = toml_string(&text, "windsurf_api_key")?;
    let key = key.trim();
    (!key.is_empty()).then(|| key.to_string())
}

#[async_trait]
impl Provider for Devin {
    fn id(&self) -> &str {
        "devin"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(300)
    }

    fn detect(&self) -> Option<Account> {
        api_key()?;
        // The plan name comes with the status response.
        Some(Account {
            label: String::new(),
        })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        // Read per request and dropped afterwards; never stored or logged.
        let key = api_key().ok_or(ProviderError::NotLoggedIn)?;
        let resp = http_no_redirect()?
            .post(format!("{ORIGIN}{STATUS_PATH}"))
            .header("Connect-Protocol-Version", "1")
            .json(&json!({
                "metadata": {
                    "apiKey": key,
                    "ideName": "devin",
                    "ideVersion": "0.0.0",
                    "extensionVersion": "0.0.0",
                    "locale": "en",
                }
            }))
            .send()
            .await?;
        check_response(&resp)?;
        let body = body_json(resp).await?;

        let plan = body
            .pointer("/userStatus/planStatus")
            .ok_or_else(|| ProviderError::SchemaChanged("no plan status".into()))?;
        let windows = parse(plan);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged("no known usage fields".into()));
        }
        let account = plan
            .pointer("/planInfo/planName")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
            .unwrap_or(&acc.label)
            .to_string();
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account,
            windows,
            subscription_ends_at: plan.get("planEnd").and_then(timestamp),
            fetched_at: Utc::now(),
        })
    }
}

/// One quota window. The service sends what is left, and drops zero values from the JSON, so a
/// reset time without a percentage means nothing is left; neither field means no such window.
fn quota(plan: &Value, kind: WindowKind, label: &str, name: &str) -> Option<UsageWindow> {
    let resets_at = plan
        .get(format!("{name}QuotaResetAtUnix"))
        .and_then(timestamp);
    let left = plan
        .get(format!("{name}QuotaRemainingPercent"))
        .and_then(number)
        .or_else(|| resets_at.map(|_| 0.0))?;
    Some(UsageWindow {
        kind,
        label: label.into(),
        used_pct: Some((100.0 - left).clamp(0.0, 100.0) as f32),
        resets_at,
        detail: None,
    })
}

fn parse(plan: &Value) -> Vec<UsageWindow> {
    let mut out: Vec<_> = [
        quota(plan, WindowKind::Daily, "Day", "daily"),
        quota(plan, WindowKind::Weekly, "Week", "weekly"),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Pay-as-you-go balance beyond the included quota, in millionths of a dollar.
    let overage = plan
        .get("overageBalanceMicros")
        .and_then(number)
        .filter(|m| *m > 0.0);
    if let (Some(micros), false) = (overage, out.is_empty()) {
        out.push(UsageWindow {
            kind: WindowKind::Credits,
            label: "Credits".into(),
            used_pct: None,
            resets_at: None,
            detail: Some(format!("${:.2}", micros / 1_000_000.0)),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flat_toml_strings() {
        let text = "windsurf_api_key = \"fake-key\"\napi_server_url = 'https://server.codeium.com'\n# note = \"x\"\n";
        assert_eq!(
            toml_string(text, "windsurf_api_key").as_deref(),
            Some("fake-key")
        );
        assert_eq!(
            toml_string(text, "api_server_url").as_deref(),
            Some("https://server.codeium.com")
        );
        assert_eq!(toml_string(text, "missing"), None);
        assert_eq!(
            toml_string("windsurf_api_key = 5", "windsurf_api_key"),
            None
        );
    }

    #[test]
    fn remaining_percent_becomes_used_percent() {
        let plan = json!({
            "dailyQuotaRemainingPercent": 100,
            "weeklyQuotaRemainingPercent": 50,
            "dailyQuotaResetAtUnix": "1792224000",
            "weeklyQuotaResetAtUnix": "1792310400"
        });
        let w = parse(&plan);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kind, WindowKind::Daily);
        assert_eq!(w[0].used_pct, Some(0.0));
        assert_eq!(w[0].resets_at.unwrap().timestamp(), 1_792_224_000);
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_pct, Some(50.0));
    }

    #[test]
    fn omitted_percent_with_a_reset_time_means_used_up() {
        let plan = json!({
            "weeklyQuotaRemainingPercent": 30,
            "dailyQuotaResetAtUnix": "1792224000",
            "weeklyQuotaResetAtUnix": "1792310400"
        });
        let w = parse(&plan);
        assert_eq!(w[0].kind, WindowKind::Daily);
        assert_eq!(w[0].used_pct, Some(100.0));
    }

    #[test]
    fn missing_windows_are_skipped_and_overage_is_a_balance() {
        assert!(parse(&json!({"planInfo": {}})).is_empty());
        let plan = json!({
            "weeklyQuotaRemainingPercent": 80,
            "overageBalanceMicros": "12500000"
        });
        let w = parse(&plan);
        assert_eq!(w.len(), 2);
        assert_eq!(w[1].kind, WindowKind::Credits);
        assert_eq!(w[1].detail.as_deref(), Some("$12.50"));
    }
}
