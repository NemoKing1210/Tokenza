//! Command Code: API key from `~/.commandcode/auth.json`, live limits from the billing API.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{body_json, home, http, number, read_json, timestamp};
use crate::error::check_response;
use crate::{secrets, Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const BASE_URL: &str = "https://api.commandcode.ai/alpha";

pub struct CommandCode;

/// A key typed into Tokenza wins; then `COMMAND_CODE_API_KEY` (as in the CLI), then the CLI's
/// `apiKey` in `auth.json`.
///
/// Keys expire. The CLI's `login` only checks that `auth.json` exists, so a stale key still
/// reports "Already logged in"; `logout` followed by `login` is what actually refreshes it.
fn api_key() -> Option<String> {
    secrets::get("commandcode", "api_key")
        .or_else(|| std::env::var("COMMAND_CODE_API_KEY").ok())
        .or_else(|| {
            let auth = read_json(&home()?.join(".commandcode").join("auth.json"))?;
            Some(auth.get("apiKey")?.as_str()?.to_string())
        })
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// Plan id prefix → (display name, included monthly credits), mirroring the CLI's plan table.
/// The credit balance only says what is left, so the plan's allowance is the denominator.
/// New plan revisions keep the old prefix (`individual-goat-v2`), so match the longest prefix
/// the way the CLI does instead of pinning exact ids.
const PLANS: &[(&str, &str, f64)] = &[
    ("individual-go", "Go", 10.0),
    ("individual-go-v1", "Go", 10.0),
    ("individual-goat", "GOAT", 70.0),
    ("individual-pro", "Pro", 30.0),
    ("individual-pro-v1", "Pro", 80.0),
    ("individual-provider", "Provider", 15.0),
    ("individual-max", "Max", 150.0),
    ("individual-ultra", "Ultra", 300.0),
    ("teams-pro", "Teams Pro", 40.0),
];

fn plan(id: &str) -> Option<(&'static str, f64)> {
    let id = id.trim().to_ascii_lowercase().replace('_', "-");
    let mut found: Option<&(&str, &str, f64)> = None;
    for candidate in PLANS {
        if id.starts_with(candidate.0) && found.is_none_or(|p| candidate.0.len() > p.0.len()) {
            found = Some(candidate);
        }
    }
    found.map(|(_, name, credits)| (*name, *credits))
}

struct Subscription {
    plan_id: Option<String>,
    period_end: Option<DateTime<Utc>>,
}

/// The plan id lives in the subscription for older accounts and in the credit ledger for newer
/// ones; billing works for both, so accept either.
fn credit_plan_id(body: &Value) -> Option<&str> {
    ["/credits/planId", "/planId"]
        .iter()
        .find_map(|path| body.pointer(path)?.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

/// The org a key belongs to, for team billing scope. The CLI reads `org.id`; older captures wrap
/// the same object in `data`.
fn org_id(body: &Value) -> Option<String> {
    ["/org/id", "/data/org/id"]
        .iter()
        .find_map(|path| body.pointer(path)?.as_str())
        .map(str::to_string)
}

#[async_trait]
impl Provider for CommandCode {
    fn id(&self) -> &str {
        "commandcode"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(300)
    }

    fn detect(&self) -> Option<Account> {
        api_key()?;
        // The plan is learned from the subscription request in `fetch`.
        Some(Account {
            label: String::new(),
        })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let key = api_key().ok_or(ProviderError::NotLoggedIn)?;
        let client = http()?;
        let get = |path: &str, org: Option<&str>| {
            let req = client.get(format!("{BASE_URL}{path}")).bearer_auth(&key);
            match org {
                Some(org) => req.query(&[("orgId", org)]),
                None => req,
            }
        };

        // Team accounts need their organisation id on the billing calls. Both this lookup and the
        // subscription are optional: without them only the monthly window is lost.
        let org = match get("/whoami?limits=1", None).send().await {
            Ok(r) if r.status().is_success() => body_json(r).await.ok().and_then(|b| org_id(&b)),
            _ => None,
        };
        let sub = match get("/billing/subscriptions", org.as_deref()).send().await {
            Ok(r) if r.status().is_success() => {
                body_json(r).await.ok().and_then(|b| parse_subscription(&b))
            }
            _ => None,
        };

        let resp = get("/billing/credits", org.as_deref()).send().await?;
        check_response(&resp)?;
        let body = body_json(resp).await?;

        let windows = parse(&body, sub.as_ref());
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged("no known usage fields".into()));
        }
        let plan_id = sub
            .as_ref()
            .and_then(|s| s.plan_id.as_deref())
            .or_else(|| credit_plan_id(&body));
        let account = plan_id
            .and_then(plan)
            .map_or_else(|| acc.label.clone(), |(name, _)| name.to_string());
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account,
            windows,
            subscription_ends_at: sub.as_ref().and_then(|s| s.period_end),
            fetched_at: Utc::now(),
        })
    }
}

/// Checks a key obtained by the browser sign-in and returns the account name it belongs to.
/// Called before the key is stored, so an expired or mistyped key never replaces a working one.
pub async fn validate_key(key: &str) -> Result<String, ProviderError> {
    let client = http()?;
    let resp = client
        .get(format!("{BASE_URL}/whoami?limits=1"))
        .bearer_auth(key)
        .send()
        .await?;
    check_response(&resp)?;
    let body = body_json(resp).await?;
    Ok(["/user/userName", "/user/name"]
        .iter()
        .find_map(|path| body.pointer(path)?.as_str())
        .unwrap_or_default()
        .to_string())
}

fn parse_subscription(body: &Value) -> Option<Subscription> {
    let data = body.get("data")?;
    let plan_id = data
        .get("planId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let period_end = data.get("currentPeriodEnd").and_then(timestamp);
    // The plan and the period end arrive together, but either is useful on its own.
    (plan_id.is_some() || period_end.is_some()).then_some(Subscription {
        plan_id,
        period_end,
    })
}

/// Rolling limits have been observed at the top level and beside the ledger; accept both.
fn window_limit<'a>(body: &'a Value, key: &str) -> Option<&'a Value> {
    body.pointer(&format!("/credits/windowLimits/{key}"))
        .or_else(|| body.pointer(&format!("/windowLimits/{key}")))
}

fn parse(body: &Value, sub: Option<&Subscription>) -> Vec<UsageWindow> {
    let mut out = Vec::new();
    for (key, kind, label) in [
        ("fiveHour", WindowKind::Session5h, "5 h"),
        ("weekly", WindowKind::Weekly, "Week"),
    ] {
        let Some(w) = window_limit(body, key) else {
            continue;
        };
        let (Some(used), Some(cap)) = (
            w.get("used").and_then(number),
            w.get("cap").and_then(number).filter(|c| *c > 0.0),
        ) else {
            continue;
        };
        out.push(UsageWindow {
            kind,
            label: label.into(),
            // A window can be over its cap (extra credits keep working); the meter stops at 100%.
            used_pct: Some((used / cap * 100.0).clamp(0.0, 100.0) as f32),
            resets_at: w
                .get("resetAt")
                .or_else(|| w.get("reset_at"))
                .and_then(timestamp),
            detail: None,
        });
    }

    // The included monthly credits that are already spent. Purchased and free credits are extra
    // and do not count against the plan; an unknown plan has no known allowance, so no window.
    let plan_id = sub
        .and_then(|s| s.plan_id.as_deref())
        .or_else(|| credit_plan_id(body));
    let allowance = plan_id.and_then(plan).map(|(_, total)| total);
    let left = body.pointer("/credits/monthlyCredits").and_then(number);
    if let (Some(total), Some(left)) = (allowance, left) {
        out.push(UsageWindow {
            kind: WindowKind::Monthly,
            label: "Month".into(),
            used_pct: Some(((total - left.max(0.0)) / total * 100.0).clamp(0.0, 100.0) as f32),
            resets_at: sub.and_then(|s| s.period_end),
            detail: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_both_windows() {
        let body = serde_json::json!({
            "windowLimits": {
                "fiveHour": {"used": 2.5, "cap": 10, "resetAt": 1790000000000i64},
                "weekly": {"used": "30", "cap": "120", "resetAt": "2026-11-01T00:00:00Z"}
            }
        });
        let w = parse(&body, None);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[0].used_pct, Some(25.0));
        assert_eq!(w[0].resets_at.unwrap().timestamp(), 1_790_000_000);
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_pct, Some(25.0));
        assert!(w[1].resets_at.is_some());
    }

    #[test]
    fn seconds_resets_are_accepted() {
        assert_eq!(
            timestamp(&serde_json::json!(1790000000))
                .unwrap()
                .timestamp(),
            1_790_000_000
        );
    }

    #[test]
    fn skips_windows_without_a_cap() {
        let body = serde_json::json!({
            "windowLimits": {"fiveHour": {"used": 1, "cap": 0}, "weekly": {"used": 1}}
        });
        assert!(parse(&body, None).is_empty());
    }

    #[test]
    fn unknown_shape_yields_no_windows() {
        assert!(parse(&serde_json::json!({"balance": 5}), None).is_empty());
    }

    #[test]
    fn monthly_window_comes_from_the_plan_allowance() {
        let credits = serde_json::json!({
            "credits": {"monthlyCredits": 59.4, "purchasedCredits": 5, "freeCredits": 0},
            "windowLimits": {"fiveHour": {"used": 1, "cap": 14, "resetAt": 1790000000000i64}}
        });
        let subscription = serde_json::json!({
            "success": true,
            "data": {"planId": "individual-goat", "currentPeriodEnd": "2026-11-01T01:14:01.000Z"}
        });
        let sub = parse_subscription(&subscription).unwrap();
        let w = parse(&credits, Some(&sub));
        assert_eq!(w.len(), 2);
        let month = &w[1];
        assert_eq!(month.kind, WindowKind::Monthly);
        // GOAT includes $70; $59.40 is left, so $10.60 is used.
        assert!((month.used_pct.unwrap() - 15.142857).abs() < 0.001);
        assert_eq!(
            month.resets_at.unwrap().to_rfc3339(),
            "2026-11-01T01:14:01+00:00"
        );
    }

    #[test]
    fn no_monthly_window_for_an_unknown_plan_or_missing_balance() {
        let credits = serde_json::json!({"credits": {"monthlyCredits": 5}});
        let unknown = Subscription {
            plan_id: Some("enterprise-custom".into()),
            period_end: None,
        };
        assert!(parse(&credits, Some(&unknown)).is_empty());
        let goat = Subscription {
            plan_id: Some("individual-goat".into()),
            period_end: None,
        };
        assert!(parse(&serde_json::json!({"credits": {}}), Some(&goat)).is_empty());
        // A balance above the allowance stays within 0..=100.
        let w = parse(
            &serde_json::json!({"credits": {"monthlyCredits": 999}}),
            Some(&goat),
        );
        assert_eq!(w[0].used_pct, Some(0.0));
    }

    #[test]
    fn plan_ids_match_by_longest_prefix() {
        assert_eq!(plan("individual-goat"), Some(("GOAT", 70.0)));
        assert_eq!(plan("individual-goat-v2"), Some(("GOAT", 70.0)));
        assert_eq!(plan("Individual_GOAT_Annual"), Some(("GOAT", 70.0)));
        assert_eq!(plan("individual-pro-v1"), Some(("Pro", 80.0)));
        assert_eq!(plan("individual-pro"), Some(("Pro", 30.0)));
        assert_eq!(plan("individual-go"), Some(("Go", 10.0)));
        assert_eq!(plan("enterprise-custom"), None);
    }

    #[test]
    fn nested_window_limits_are_accepted() {
        // The live ledger also carries the windows beside `credits`, and the plan id there.
        let body = serde_json::json!({
            "credits": {
                "monthlyCredits": 42,
                "planId": "individual-goat",
                "windowLimits": {
                    "fiveHour": {"used": 7, "cap": 14, "resetAt": 1787860800000i64},
                    "weekly": {"used": 10.5, "cap": 35, "reset_at": "2026-08-31T00:00:00Z"}
                }
            }
        });
        let w = parse(&body, None);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[0].used_pct, Some(50.0));
        assert_eq!(
            w[0].resets_at.unwrap().timestamp_millis(),
            1_787_860_800_000
        );
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_pct, Some(30.0));
        assert!(w[1].resets_at.is_some());
        assert_eq!(w[2].kind, WindowKind::Monthly);
        assert_eq!(w[2].used_pct, Some(40.0));
        assert!(w[2].resets_at.is_none());
    }

    #[test]
    fn monthly_window_does_not_need_a_subscription() {
        let sub = parse_subscription(&serde_json::json!({
            "data": {"currentPeriodEnd": "2026-11-01T01:14:01.000Z"}
        }))
        .unwrap();
        assert!(sub.plan_id.is_none());
        let body = serde_json::json!({
            "planId": "individual-goat",
            "credits": {"monthlyCredits": 35}
        });
        let w = parse(&body, Some(&sub));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].kind, WindowKind::Monthly);
        assert_eq!(w[0].used_pct, Some(50.0));
        assert_eq!(
            w[0].resets_at.unwrap().to_rfc3339(),
            "2026-11-01T01:14:01+00:00"
        );
    }

    #[test]
    fn running_over_a_cap_is_clamped() {
        let body = serde_json::json!({
            "windowLimits": {"fiveHour": {"used": 20, "cap": 14}}
        });
        assert_eq!(parse(&body, None)[0].used_pct, Some(100.0));
    }

    #[test]
    fn org_id_accepts_both_whoami_shapes() {
        assert_eq!(
            org_id(&serde_json::json!({"org": {"id": "o1"}})).as_deref(),
            Some("o1")
        );
        assert_eq!(
            org_id(&serde_json::json!({"data": {"org": {"id": "o1"}}})).as_deref(),
            Some("o1")
        );
        assert!(org_id(&serde_json::json!({"user": {"userName": "x"}})).is_none());
    }
}
