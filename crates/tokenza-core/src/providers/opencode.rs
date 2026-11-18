//! OpenCode Go: subscription meters of the OpenCode Go plan.
//!
//! Reads Go usage using an explicit API key or the CLI's local credentials. Existing
//! workspace-and-cookie configurations remain available as a fallback.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use super::{body_json, home, http_no_redirect, number, read_json, timestamp};
use crate::error::check_response;
use crate::{secrets, Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const STATUS_URL: &str = "https://opencode.ai/console/api/go/status";
const API_KEY_VAR: &str = "OPENCODE_GO_API_KEY";
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
/// Pasting from DevTools often brings quotes or a `Cookie:` prefix along.
fn cookie_header(raw: &str) -> String {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix("Cookie:")
        .or_else(|| raw.strip_prefix("cookie:"))
        .unwrap_or(raw)
        .trim()
        .trim_matches(|c| c == '"' || c == '\'');
    if raw.contains('=') {
        raw.to_string()
    } else {
        format!("{COOKIE_NAME}={raw}")
    }
}

/// The `wrk_…` id, even when the whole console address was pasted.
fn workspace_id(raw: &str) -> String {
    let raw = raw.trim().trim_matches(|c| c == '"' || c == '\'');
    match raw.find("wrk_") {
        Some(i) => raw[i..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect(),
        None => raw.to_string(),
    }
}

fn credentials() -> Option<(String, String)> {
    Some((
        workspace_id(&setting("workspace_id", WORKSPACE_VAR)?),
        setting("cookie", COOKIE_VAR)?,
    ))
}

/// What the `opencode` CLI has stored locally.
struct CliLogin {
    token: String,
    workspace: Option<String>,
    /// The CLI refreshes its own token when it runs; we never write to its files, so a lapsed
    /// one is reported rather than renewed.
    expired: bool,
}

fn data_dir() -> Option<PathBuf> {
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|path| path.join(".local").join("share")))?;
    Some(root.join("opencode"))
}

fn database_path(dir: &Path) -> PathBuf {
    let name = std::env::var_os("OPENCODE_DB")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "opencode.db".into());
    dir.join(name)
}

/// The console login from `opencode.db`, opened read-only because the CLI keeps it open.
fn db_login(path: &Path) -> Option<CliLogin> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    conn.busy_timeout(Duration::from_secs(2)).ok()?;
    let raw: String = conn
        .query_row(
            "SELECT value FROM credential WHERE integration_id IN ('opencode-go', 'opencode') \
             AND (active = 1 OR active IS NULL) \
             ORDER BY integration_id = 'opencode-go' DESC, active DESC, time_updated DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()?;
    parse_db_login(&serde_json::from_str(&raw).ok()?)
}

fn parse_db_login(v: &Value) -> Option<CliLogin> {
    // A private Console's token must stay on the server it was issued for.
    let server = v.pointer("/metadata/server").and_then(Value::as_str);
    if server.is_some_and(|server| {
        !matches!(
            server.trim_end_matches('/'),
            "https://opencode.ai/console" | "https://opencode.ai" | "https://console.opencode.ai"
        )
    }) {
        return None;
    }
    let (field, expiry) = match v.get("type")?.as_str()? {
        "oauth" => ("access", v.get("expires").and_then(timestamp)),
        "key" => ("key", None),
        _ => return None,
    };
    let token = v.get(field)?.as_str()?.trim().to_string();
    if token.is_empty() {
        return None;
    }
    let workspace = v
        .pointer("/metadata/orgID")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(CliLogin {
        token,
        workspace,
        expired: expiry.is_some_and(|t| t <= Utc::now()),
    })
}

/// The Go API key an older CLI left in `auth.json`.
fn auth_json_login(dir: &Path) -> Option<CliLogin> {
    let v = read_json(&dir.join("auth.json"))?;
    let key = v
        .get("opencode-go")?
        .get("key")?
        .as_str()?
        .trim()
        .to_string();
    (!key.is_empty()).then_some(CliLogin {
        token: key,
        workspace: None,
        expired: false,
    })
}

fn cli_login() -> Option<CliLogin> {
    let dir = data_dir()?;
    match db_login(&database_path(&dir)) {
        Some(login) if login.expired => auth_json_login(&dir).or(Some(login)),
        login => login.or_else(|| auth_json_login(&dir)),
    }
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
        if setting("api_key", API_KEY_VAR).is_none()
            && cli_login().is_none()
            && credentials().is_none()
        {
            return None;
        }
        Some(Account { label: "go".into() })
    }

    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let windows = if let Some(key) = setting("api_key", API_KEY_VAR) {
            // An explicit key selects an account. Report its result even when a CLI login exists.
            fetch_usage(&key, None).await?
        } else {
            let login = tokio::task::spawn_blocking(cli_login).await.ok().flatten();
            let cli = match login {
                Some(login) if !login.expired => {
                    fetch_usage(&login.token, login.workspace.as_deref()).await
                }
                Some(_) => Err(ProviderError::TokenExpired),
                None => Err(ProviderError::NotLoggedIn),
            };
            match cli {
                Err(
                    ProviderError::TokenExpired
                    | ProviderError::NotLoggedIn
                    | ProviderError::NoSubscription,
                ) => match credentials() {
                    Some((workspace, cookie)) => fetch_console(&workspace, &cookie).await?,
                    None => cli?,
                },
                _ => cli?,
            }
        };
        Ok(UsageSnapshot {
            provider: self.id().into(),
            account: acc.label.clone(),
            windows,
            subscription_ends_at: None,
            fetched_at: Utc::now(),
        })
    }
}

/// `/zen/go/v1/usage`, authenticated with a Go API key or Console access token.
async fn fetch_usage(
    token: &str,
    workspace: Option<&str>,
) -> Result<Vec<UsageWindow>, ProviderError> {
    let mut request = http_no_redirect()?.get(USAGE_URL).bearer_auth(token);
    if let Some(workspace) = workspace {
        request = request.header("x-org-id", workspace);
    }
    let resp = request.send().await?;
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(forbidden_usage(&body_json(resp).await?));
    }
    check_response(&resp)?;
    let windows = parse_usage(&body_json(resp).await?);
    if windows.is_empty() {
        return Err(ProviderError::SchemaChanged("no known usage meters".into()));
    }
    Ok(windows)
}

fn forbidden_usage(body: &Value) -> ProviderError {
    match body.pointer("/error/type").and_then(Value::as_str) {
        Some("EntitlementError") => ProviderError::NoSubscription,
        Some("AuthError") => ProviderError::TokenExpired,
        _ => ProviderError::SchemaChanged("unexpected HTTP 403".into()),
    }
}

/// The console status endpoint, authenticated with a pasted session cookie.
async fn fetch_console(workspace: &str, cookie: &str) -> Result<Vec<UsageWindow>, ProviderError> {
    let resp = http_no_redirect()?
        .get(STATUS_URL)
        .header("x-org-id", workspace)
        .header("Cookie", cookie_header(cookie))
        .send()
        .await?;
    // The console answers 400 when the workspace id is missing or not one of the account's.
    if resp.status().as_u16() == 400 {
        return Err(ProviderError::NotLoggedIn);
    }
    check_response(&resp)?;
    let body = body_json(resp).await?;

    let windows = parse(&body);
    if windows.is_empty() {
        // `access: null` is the console's way of saying there is no Go subscription.
        if body.is_null() || body.get("access").is_some_and(Value::is_null) {
            return Err(ProviderError::NoSubscription);
        }
        return Err(ProviderError::SchemaChanged("no known usage meters".into()));
    }
    Ok(windows)
}

/// `{"usage": {"rolling"|"weekly"|"monthly": {status, percent, resetsAt}}}`; the percent is a
/// whole number, and a window that is `rate-limited` can still read 99.
fn parse_usage(body: &Value) -> Vec<UsageWindow> {
    let mut out = Vec::new();
    for (key, kind, label) in [
        ("rolling", WindowKind::Session5h, "5 h"),
        ("weekly", WindowKind::Weekly, "Week"),
        ("monthly", WindowKind::Monthly, "Month"),
    ] {
        let Some(m) = body.pointer(&format!("/usage/{key}")) else {
            continue;
        };
        let Some(pct) = m.get("percent").and_then(number).filter(|n| n.is_finite()) else {
            continue;
        };
        let limited = m.get("status").and_then(Value::as_str) == Some("rate-limited");
        out.push(UsageWindow {
            kind,
            label: label.into(),
            used_pct: Some(if limited { 100.0 } else { pct as f32 }),
            resets_at: m.get("resetsAt").and_then(timestamp),
            detail: None,
        });
    }
    out
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
        // Both amounts share one unit (micro-cents), so only their ratio matters. The console
        // sends them as strings.
        let (Some(used), Some(limit)) = (
            m.get("usedMicroCents").and_then(number),
            m.get("limitMicroCents")
                .and_then(number)
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
    fn console_meters_may_be_strings() {
        let body = serde_json::json!({"access": {"meters": {
            "week": {"resetsAt": "2026-11-02T00:00:00.000Z", "limitMicroCents": "3000000000", "usedMicroCents": "1500000000"}
        }}});
        let w = parse(&body);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].used_pct, Some(50.0));
    }

    #[test]
    fn maps_usage_endpoint() {
        let body = serde_json::json!({"usage": {
            "rolling": {"status": "ok", "percent": 0, "resetsAt": "2026-10-29T20:18:00.266Z"},
            "weekly": {"status": "ok", "percent": 55, "resetsAt": "2026-11-02T00:00:00.000Z"},
            "monthly": {"status": "rate-limited", "percent": 99, "resetsAt": "2026-11-18T06:55:06.000Z"}
        }});
        let w = parse_usage(&body);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].kind, WindowKind::Session5h);
        assert_eq!(w[0].used_pct, Some(0.0));
        assert!(w[0].resets_at.is_some());
        assert_eq!(w[1].used_pct, Some(55.0));
        assert_eq!(w[2].used_pct, Some(100.0));
        assert!(parse_usage(&serde_json::json!({"error": "x"})).is_empty());
    }

    #[test]
    fn distinguishes_forbidden_authentication_from_missing_plan() {
        let body = |kind| serde_json::json!({"type": "error", "error": {"type": kind}});
        assert_eq!(forbidden_usage(&body("AuthError")).kind(), "TokenExpired");
        assert_eq!(
            forbidden_usage(&body("EntitlementError")).kind(),
            "NoSubscription"
        );
        assert_eq!(
            forbidden_usage(&body("UnexpectedError")).kind(),
            "SchemaChanged"
        );
    }

    #[test]
    fn cli_login_is_scoped_to_the_public_console_and_workspace() {
        let value = |server| {
            serde_json::json!({
                "type": "oauth",
                "access": "fake-token",
                "metadata": {"server": server, "orgID": "wrk_TEST"}
            })
        };
        let login = parse_db_login(&value("https://opencode.ai/console")).unwrap();
        assert_eq!(login.workspace.as_deref(), Some("wrk_TEST"));
        assert!(parse_db_login(&value("https://private-console.example")).is_none());
    }

    #[test]
    fn reads_login_from_cli_database() {
        let dir = std::env::temp_dir().join(format!("tokenza-oc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("opencode.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE credential (id text, integration_id text, value text, active integer, time_updated integer);",
        )
        .unwrap();
        let value =
            |exp: i64| format!(r#"{{"type":"oauth","access":"fake-token","expires":{exp}}}"#);
        let future = Utc::now().timestamp_millis() + 3_600_000;
        conn.execute(
            "INSERT INTO credential VALUES ('1', 'opencode', ?1, 1, 1)",
            [value(future)],
        )
        .unwrap();
        drop(conn);
        let login = db_login(&dir.join("opencode.db")).unwrap();
        assert_eq!(login.token, "fake-token");
        assert!(!login.expired);

        let conn = Connection::open(dir.join("opencode.db")).unwrap();
        conn.execute("UPDATE credential SET value = ?1", [value(1_000)])
            .unwrap();
        drop(conn);
        assert!(db_login(&dir.join("opencode.db")).unwrap().expired);

        let conn = Connection::open(dir.join("opencode.db")).unwrap();
        conn.execute(
            "INSERT INTO credential VALUES ('2', 'opencode-go', ?1, NULL, 2)",
            [r#"{"type":"key","key":"fake-go-key"}"#],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO credential VALUES ('3', 'opencode-go', ?1, 0, 3)",
            [r#"{"type":"key","key":"fake-inactive-key"}"#],
        )
        .unwrap();
        drop(conn);
        let login = db_login(&dir.join("opencode.db")).unwrap();
        assert_eq!(login.token, "fake-go-key");
        assert!(!login.expired);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unrecognised_response_yields_no_windows() {
        assert!(parse(&serde_json::json!({"error": "no subscription"})).is_empty());
    }

    #[test]
    fn bare_cookie_gets_its_name() {
        assert_eq!(cookie_header("abc"), "__Host-console_session=abc");
        assert_eq!(cookie_header("auth=abc"), "auth=abc");
        assert_eq!(cookie_header(" \"abc\" "), "__Host-console_session=abc");
        assert_eq!(cookie_header("Cookie: auth=abc"), "auth=abc");
    }

    #[test]
    fn workspace_id_is_cut_out_of_pasted_text() {
        assert_eq!(workspace_id("wrk_ABC123"), "wrk_ABC123");
        assert_eq!(
            workspace_id("https://opencode.ai/workspace/wrk_ABC123/go"),
            "wrk_ABC123"
        );
        assert_eq!(workspace_id(" \"wrk_ABC123\" "), "wrk_ABC123");
    }
}
