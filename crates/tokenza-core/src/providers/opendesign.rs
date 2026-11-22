//! Open Design: Coding Plan credit windows of the account's personal workspace.
//!
//! The plan limits are served by Open Design's hosted gateway (vela/AMR), not by a local
//! daemon. The login the app stores in `~/.amr/config.json` is replayed to the gateway's
//! own endpoint and dropped afterwards.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::{body_json, home, http_no_redirect, number, read_json, timestamp};
use crate::error::check_response;
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const API_URL: &str = "https://amr-api.open-design.ai";
const WINDOW_5H: f64 = 18_000.0;
const WINDOW_7D: f64 = 604_800.0;

pub struct OpenDesign;

/// What the AMR config stores for the signed-in account. Never leaves this module.
struct Login {
    control_key: String,
    account: String,
}

/// The AMR profile may point a test build elsewhere; Tokenza only ever talks to production.
fn config_path() -> Option<PathBuf> {
    let dir = match std::env::var_os("AMR_HOME") {
        Some(raw) => expand_home(&raw)?,
        None => home()?.join(".amr"),
    };
    Some(dir.join("config.json"))
}

/// `AMR_HOME` may be absolute, `~/...`, or relative to the home directory.
fn expand_home(raw: &OsStr) -> Option<PathBuf> {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return Some(path);
    }
    let text = path.to_string_lossy();
    let home = home()?;
    if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
        return Some(home.join(rest));
    }
    if text == "~" {
        return Some(home);
    }
    Some(home.join(path))
}

fn read_login(path: &Path) -> Option<Login> {
    let root = read_json(path)?;
    let profiles = root.get("profiles")?.as_object()?;
    let profile = match profiles.get("prod") {
        Some(profile) => profile,
        None if profiles.len() == 1 => profiles.values().next()?,
        None => return None,
    };
    // A half-finished browser sign-in must not look like a working login.
    if profile
        .get("pendingLogin")
        .is_some_and(|value| !value.is_null())
    {
        return None;
    }
    let control_key = profile.get("controlKey")?.as_str()?.trim().to_string();
    if control_key.is_empty() {
        return None;
    }
    if profile
        .get("apiUrl")
        .and_then(Value::as_str)
        .is_some_and(|api| api.trim_end_matches('/') != API_URL)
    {
        return None;
    }
    let account = profile
        .pointer("/user/email")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "Open Design".to_owned());
    Some(Login {
        control_key,
        account,
    })
}

/// The personal active workspace, else the first workspace the account can use.
fn select_workspace(body: &Value) -> Option<String> {
    let items = body.get("items")?.as_array()?;
    let mut fallback = None;
    for item in items {
        let Some(id) = item
            .get("workspaceId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .filter(|_| item.get("memberStatus").and_then(Value::as_str) == Some("active"))
            .filter(|_| item.get("lifecycleState").and_then(Value::as_str) != Some("deleted"))
        else {
            continue;
        };
        let personal = item.get("workspaceType").and_then(Value::as_str) == Some("personal")
            && item.get("lifecycleState").and_then(Value::as_str) == Some("active");
        if personal {
            return Some(id.to_owned());
        }
        fallback.get_or_insert_with(|| id.to_owned());
    }
    fallback
}

fn parse_windows(body: &Value) -> Result<Vec<UsageWindow>, ProviderError> {
    // A wallet-funded account has no plan windows; that is not a schema change.
    let Some(plan) = body.get("codingPlan").filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for window in plan
        .get("windows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(duration) = window.get("durationSeconds").and_then(number) else {
            continue;
        };
        let (kind, label) = if (duration - WINDOW_5H).abs() < 1.0 {
            (WindowKind::Session5h, "5 h")
        } else if (duration - WINDOW_7D).abs() < 1.0 {
            (WindowKind::Weekly, "Week")
        } else {
            continue;
        };
        let (Some(used), Some(limit)) = (
            window.get("usedCredits").and_then(number),
            window
                .get("limitCredits")
                .and_then(number)
                .filter(|limit| *limit > 0.0),
        ) else {
            continue;
        };
        // The gateway sends credits as digit strings; keep them as text so no precision is lost.
        let detail = match (
            window.get("remainingCredits").and_then(Value::as_str),
            window.get("limitCredits").and_then(Value::as_str),
        ) {
            (Some(remaining), Some(limit_text)) => {
                Some(format!("{remaining} / {limit_text} credits"))
            }
            _ => None,
        };
        out.push(UsageWindow {
            kind,
            label: label.into(),
            used_pct: Some((used / limit * 100.0) as f32),
            resets_at: window.get("resetsAt").and_then(timestamp),
            detail,
        });
    }
    if out.is_empty() {
        return Err(ProviderError::SchemaChanged(
            "no recognized coding plan windows".into(),
        ));
    }
    Ok(out)
}

fn account_label(body: &Value, fallback: &str) -> String {
    body.pointer("/codingPlan/tier")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| fallback.to_owned())
}

#[async_trait]
impl Provider for OpenDesign {
    fn id(&self) -> &str {
        "opendesign"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(300)
    }

    fn detect(&self) -> Option<Account> {
        let login = config_path().and_then(|path| read_login(&path))?;
        Some(Account {
            label: login.account,
        })
    }

    async fn fetch(&self, _acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let login =
            tokio::task::spawn_blocking(|| config_path().and_then(|path| read_login(&path)))
                .await
                .ok()
                .flatten()
                .ok_or(ProviderError::NotLoggedIn)?;
        let client = http_no_redirect()?;
        let resp = client
            .get(format!("{API_URL}/api/v1/workspaces"))
            .bearer_auth(&login.control_key)
            .send()
            .await?;
        check_response(&resp)?;
        let directory = body_json(resp).await?;
        let workspace = select_workspace(&directory).ok_or(ProviderError::NoSubscription)?;
        let resp = client
            .get(format!(
                "{API_URL}/api/v1/workspaces/{workspace}/billing/funding-check"
            ))
            .bearer_auth(&login.control_key)
            .send()
            .await?;
        check_response(&resp)?;
        let body = body_json(resp).await?;
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account: account_label(&body, &login.account),
            windows: parse_windows(&body)?,
            subscription_ends_at: None,
            fetched_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preflight(windows: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "workspaceId": "ws_1",
            "workspaceMemberId": "wm_1",
            "generatedAt": "2026-11-22T10:00:00Z",
            "balanceUsd": "0.00",
            "funding": "coding_plan",
            "codingPlan": {"tier": "go", "eligible": true, "windows": windows}
        })
    }

    #[test]
    fn parses_coding_plan_windows() {
        let body = preflight(serde_json::json!([
            {
                "policyId": "go-5h", "durationSeconds": 18000, "resetMode": "activity_triggered",
                "usedCredits": "450", "remainingCredits": "100", "limitCredits": "550",
                "windowStart": "2026-11-22T08:00:00Z", "resetsAt": "2026-11-22T13:00:00Z"
            },
            {
                "policyId": "go-7d", "durationSeconds": 604800, "resetMode": "anchored_recurring",
                "usedCredits": "550", "remainingCredits": "1650", "limitCredits": "2200",
                "windowStart": "2026-11-18T00:00:00Z", "resetsAt": "2026-11-25T00:00:00Z"
            }
        ]));
        let w = parse_windows(&body).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[0].label, "5 h");
        assert!((w[0].used_pct.unwrap() - 81.81818).abs() < 0.001);
        assert_eq!(w[0].detail.as_deref(), Some("100 / 550 credits"));
        assert!(w[0].resets_at.is_some());
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_pct, Some(25.0));
        assert_eq!(account_label(&body, "dev@example.com"), "go");
    }

    #[test]
    fn accepts_string_credit_values() {
        let body = preflight(serde_json::json!([
            {"durationSeconds": 18000, "usedCredits": "0", "remainingCredits": "18", "limitCredits": "18"}
        ]));
        assert_eq!(parse_windows(&body).unwrap()[0].used_pct, Some(0.0));
    }

    #[test]
    fn wallet_account_has_no_windows() {
        let wallet = serde_json::json!({"funding": "wallet", "codingPlan": null});
        assert!(parse_windows(&wallet).unwrap().is_empty());
        assert!(parse_windows(&serde_json::json!({})).unwrap().is_empty());
        assert_eq!(account_label(&wallet, "dev@example.com"), "dev@example.com");
    }

    #[test]
    fn unknown_duration_only_is_schema_change() {
        let body = preflight(serde_json::json!([
            {"durationSeconds": 86400, "usedCredits": "1", "limitCredits": "2"}
        ]));
        assert!(matches!(
            parse_windows(&body),
            Err(ProviderError::SchemaChanged(_))
        ));
    }

    #[test]
    fn prefers_personal_active_workspace() {
        let body = serde_json::json!({"items": [
            {"workspaceId": "ws_team", "workspaceType": "team", "memberStatus": "active", "lifecycleState": "active"},
            {"workspaceId": "ws_personal", "workspaceType": "personal", "memberStatus": "active", "lifecycleState": "active"},
            {"workspaceId": "ws_gone", "workspaceType": "personal", "memberStatus": "active", "lifecycleState": "deleted"}
        ]});
        assert_eq!(select_workspace(&body).as_deref(), Some("ws_personal"));

        let team_only = serde_json::json!({"items": [
            {"workspaceId": "ws_team", "workspaceType": "team", "memberStatus": "active", "lifecycleState": "active"}
        ]});
        assert_eq!(select_workspace(&team_only).as_deref(), Some("ws_team"));

        let removed = serde_json::json!({"items": [
            {"workspaceId": "ws_x", "workspaceType": "personal", "memberStatus": "removed", "lifecycleState": "active"}
        ]});
        assert_eq!(select_workspace(&removed), None);
        assert_eq!(select_workspace(&serde_json::json!({"items": []})), None);
    }

    fn config_file(dir: &std::path::Path, body: serde_json::Value) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, body.to_string()).unwrap();
        path
    }

    fn prod_profile(extra: serde_json::Value) -> serde_json::Value {
        let mut profile = serde_json::json!({
            "controlKey": "fake-control-key",
            "runtimeKey": "fake-runtime-key",
            "apiUrl": "https://amr-api.open-design.ai",
            "linkUrl": "https://amr-link.open-design.ai",
            "user": {"email": "dev@example.com"}
        });
        profile
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::json!({"profiles": {"prod": profile}})
    }

    #[test]
    fn reads_prod_profile() {
        let dir = std::env::temp_dir().join(format!("tokenza-od-{}", std::process::id()));
        let path = config_file(&dir, prod_profile(serde_json::json!({})));
        let login = read_login(&path).unwrap();
        assert_eq!(login.control_key, "fake-control-key");
        assert_eq!(login.account, "dev@example.com");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_pending_login_and_foreign_endpoint() {
        let dir = std::env::temp_dir().join(format!("tokenza-od-bad-{}", std::process::id()));
        let pending = config_file(
            &dir,
            prod_profile(serde_json::json!({"pendingLogin": {"state": "waiting"}})),
        );
        assert!(read_login(&pending).is_none());
        let foreign = config_file(
            &dir,
            serde_json::json!({"profiles": {"prod": {
                "controlKey": "fake-control-key", "apiUrl": "https://evil.example"
            }}}),
        );
        assert!(read_login(&foreign).is_none());
        let blank = config_file(
            &dir,
            serde_json::json!({"profiles": {"prod": {"controlKey": "", "apiUrl": "https://amr-api.open-design.ai"}}}),
        );
        assert!(read_login(&blank).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sole_non_prod_profile_is_used() {
        let dir = std::env::temp_dir().join(format!("tokenza-od-sole-{}", std::process::id()));
        let path = config_file(
            &dir,
            serde_json::json!({"profiles": {"test": {"controlKey": "fake-control-key"}}}),
        );
        let login = read_login(&path).unwrap();
        assert_eq!(login.account, "Open Design");
        std::fs::remove_dir_all(&dir).ok();
    }
}
