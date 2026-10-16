//! User-defined provider: one HTTPS GET with a token the user supplied, plus JSON paths that
//! map the response onto limit windows. Meant for services Tokenza has no built-in support for.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use reqwest::header::{HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, COOKIE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{http_no_redirect, number, timestamp};
use crate::error::check_response;
use crate::secrets::{self, CUSTOM_FIELD};
use crate::{Account, Provider, ProviderError, UsageSnapshot, UsageWindow, WindowKind};

const MAX_BODY: usize = 1 << 20;
const MAX_WINDOWS: usize = 6;
const NAME_MAX: usize = 24;
const LABEL_MAX: usize = 24;
const PATH_MAX: usize = 120;
pub const ID_PREFIX: &str = "custom-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    /// `Authorization: Bearer <token>`
    Bearer,
    /// `<name>: <token>`
    Header,
    /// `Cookie: <name>=<token>`
    Cookie,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomAuth {
    pub scheme: AuthScheme,
    /// Header or cookie name; unused for bearer auth.
    pub name: String,
}

impl Default for CustomAuth {
    fn default() -> Self {
        Self {
            scheme: AuthScheme::Bearer,
            name: String::new(),
        }
    }
}

/// How to read one limit window out of the response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomWindow {
    pub label: String,
    pub kind: WindowKind,
    /// Path to a number: the used amount, or the used percentage when `limit_path` is empty.
    pub used_path: String,
    /// Path to the total; makes `used_path` an amount instead of a percentage.
    pub limit_path: Option<String>,
    /// The value at `used_path` is what is left, not what is used.
    pub remaining: bool,
    /// Path to the reset time: RFC 3339 text or epoch seconds / milliseconds.
    pub resets_at_path: Option<String>,
}

impl Default for CustomWindow {
    fn default() -> Self {
        Self {
            label: String::new(),
            kind: WindowKind::Monthly,
            used_path: String::new(),
            limit_path: None,
            remaining: false,
            resets_at_path: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomDef {
    /// `custom-` plus a slug; assigned by the host application.
    pub id: String,
    pub name: String,
    pub url: String,
    pub auth: CustomAuth,
    pub windows: Vec<CustomWindow>,
}

fn blank_to_none(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
}

impl CustomDef {
    /// Trims text fields and turns blank optional paths into `None`.
    pub fn normalize(&mut self) {
        self.name = self.name.trim().to_string();
        self.url = self.url.trim().to_string();
        self.auth.name = self.auth.name.trim().to_string();
        for w in &mut self.windows {
            w.label = w.label.trim().to_string();
            w.used_path = w.used_path.trim().to_string();
            w.limit_path = blank_to_none(w.limit_path.take());
            w.resets_at_path = blank_to_none(w.resets_at_path.take());
        }
    }

    /// Checks everything the user typed; the message is shown to them, so it is plain English.
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = self.id.len() <= 40
            && self.id.starts_with(ID_PREFIX)
            && self.id.len() > ID_PREFIX.len()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err("invalid provider id".into());
        }
        if self.name.is_empty() || self.name.chars().count() > NAME_MAX {
            return Err(format!("name must be 1 to {NAME_MAX} characters"));
        }
        let url = reqwest::Url::parse(&self.url).map_err(|_| "the URL is not valid".to_string())?;
        if url.scheme() != "https" || url.host_str().is_none() {
            return Err("the URL must start with https://".into());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("the URL must not contain a user name or password".into());
        }
        if self.auth.scheme != AuthScheme::Bearer {
            let n = &self.auth.name;
            if n.is_empty() || n.len() > 64 || !n.chars().all(is_token_char) {
                return Err("the header or cookie name may only use letters, digits, - _ .".into());
            }
        }
        if self.windows.is_empty() || self.windows.len() > MAX_WINDOWS {
            return Err(format!("add 1 to {MAX_WINDOWS} limit windows"));
        }
        for (i, w) in self.windows.iter().enumerate() {
            if w.label.is_empty() || w.label.chars().count() > LABEL_MAX {
                return Err(format!("window label must be 1 to {LABEL_MAX} characters"));
            }
            if self.windows[..i].iter().any(|o| o.label == w.label) {
                return Err("window labels must be unique".into());
            }
            if w.kind == WindowKind::Credits {
                return Err("credits windows are not supported".into());
            }
            let paths = [
                Some(&w.used_path),
                w.limit_path.as_ref(),
                w.resets_at_path.as_ref(),
            ];
            if w.used_path.is_empty() || paths.iter().flatten().any(|p| p.len() > PATH_MAX) {
                return Err("every window needs a value path of at most 120 characters".into());
            }
        }
        Ok(())
    }

    pub fn into_provider(self) -> Box<dyn Provider> {
        Box::new(CustomProvider { def: self })
    }
}

struct CustomProvider {
    def: CustomDef,
}

/// A secret as a header value that never shows up in debug output.
fn sensitive(value: String) -> Result<HeaderValue, ProviderError> {
    let mut v = HeaderValue::from_str(&value).map_err(|_| ProviderError::NotLoggedIn)?;
    v.set_sensitive(true);
    Ok(v)
}

#[async_trait]
impl Provider for CustomProvider {
    fn id(&self) -> &str {
        &self.def.id
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(180)
    }

    fn detect(&self) -> Option<Account> {
        secrets::get(&self.def.id, CUSTOM_FIELD)?;
        Some(Account {
            label: self.def.name.clone(),
        })
    }

    async fn fetch(&self, _acc: &Account) -> Result<UsageSnapshot, ProviderError> {
        let token = secrets::get(&self.def.id, CUSTOM_FIELD).ok_or(ProviderError::NotLoggedIn)?;
        let auth = &self.def.auth;
        let req = http_no_redirect()?
            .get(&self.def.url)
            .header(ACCEPT, "application/json");
        let req = match auth.scheme {
            AuthScheme::Bearer => req.header(AUTHORIZATION, sensitive(format!("Bearer {token}"))?),
            AuthScheme::Header => {
                let name = HeaderName::from_bytes(auth.name.as_bytes())
                    .map_err(|_| ProviderError::SchemaChanged("invalid header name".into()))?;
                req.header(name, sensitive(token)?)
            }
            AuthScheme::Cookie => req.header(COOKIE, sensitive(format!("{}={token}", auth.name))?),
        };
        let resp = req.send().await?;
        check_response(&resp)?;
        let bytes = resp.bytes().await?;
        if bytes.len() > MAX_BODY {
            return Err(ProviderError::SchemaChanged("response is too large".into()));
        }
        let body: Value = serde_json::from_slice(&bytes)
            .map_err(|_| ProviderError::SchemaChanged("response is not JSON".into()))?;

        let windows = parse(&self.def, &body);
        if windows.is_empty() {
            return Err(ProviderError::SchemaChanged(
                "no configured value found".into(),
            ));
        }
        Ok(UsageSnapshot {
            provider: self.def.id.clone(),
            // Plan names are not configurable; an empty account shows no plan badge.
            account: String::new(),
            windows,
            fetched_at: Utc::now(),
        })
    }
}

/// `/a/b/0` is a JSON pointer; `a.b.0` is the same path written with dots.
fn lookup<'a>(body: &'a Value, path: &str) -> Option<&'a Value> {
    let path = path.trim();
    if path.starts_with('/') {
        body.pointer(path)
    } else {
        body.pointer(&format!("/{}", path.replace('.', "/")))
    }
}

fn parse(def: &CustomDef, body: &Value) -> Vec<UsageWindow> {
    let mut out = Vec::new();
    for w in &def.windows {
        let Some(value) = lookup(body, &w.used_path).and_then(number) else {
            continue;
        };
        let pct = match w.limit_path.as_deref() {
            Some(lp) => {
                let Some(limit) = lookup(body, lp).and_then(number).filter(|l| *l > 0.0) else {
                    continue;
                };
                let used = if w.remaining { limit - value } else { value };
                used / limit * 100.0
            }
            None if w.remaining => 100.0 - value,
            None => value,
        };
        out.push(UsageWindow {
            kind: w.kind,
            label: w.label.clone(),
            used_pct: Some(pct.max(0.0) as f32),
            resets_at: w
                .resets_at_path
                .as_deref()
                .and_then(|p| lookup(body, p))
                .and_then(timestamp),
            detail: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(label: &str, used: &str) -> CustomWindow {
        CustomWindow {
            label: label.into(),
            used_path: used.into(),
            ..Default::default()
        }
    }

    fn def(windows: Vec<CustomWindow>) -> CustomDef {
        CustomDef {
            id: "custom-demo".into(),
            name: "Demo".into(),
            url: "https://example.com/usage".into(),
            windows,
            ..Default::default()
        }
    }

    #[test]
    fn reads_percent_amounts_and_remaining() {
        let body = serde_json::json!({
            "quota": {"pct": 42.5, "used": "30", "limit": 120, "left": 90, "reset": 1790000000},
            "items": [{"pct_left": 10}]
        });
        let mut by_amount = window("Month", "quota.used");
        by_amount.limit_path = Some("/quota/limit".into());
        by_amount.resets_at_path = Some("quota.reset".into());
        let mut left = window("Left", "/quota/left");
        left.limit_path = Some("quota.limit".into());
        left.remaining = true;
        let mut pct_left = window("Items", "items.0.pct_left");
        pct_left.remaining = true;

        let w = parse(
            &def(vec![window("Pct", "quota.pct"), by_amount, left, pct_left]),
            &body,
        );
        let pcts: Vec<_> = w.iter().map(|w| w.used_pct.unwrap()).collect();
        assert_eq!(pcts, [42.5, 25.0, 25.0, 90.0]);
        assert_eq!(w[1].resets_at.unwrap().timestamp(), 1_790_000_000);
        assert!(w[0].resets_at.is_none());
    }

    #[test]
    fn skips_windows_whose_paths_do_not_resolve() {
        let body = serde_json::json!({"a": 5, "limit": 0});
        let mut zero_limit = window("Zero", "a");
        zero_limit.limit_path = Some("limit".into());
        let w = parse(
            &def(vec![
                window("Missing", "nope"),
                zero_limit,
                window("Ok", "a"),
            ]),
            &body,
        );
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].label, "Ok");
    }

    #[test]
    fn validation_rejects_unsafe_or_incomplete_definitions() {
        let good = def(vec![window("Month", "used")]);
        assert!(good.validate().is_ok());

        let mut d = good.clone();
        d.url = "http://example.com/usage".into();
        assert!(d.validate().is_err(), "plain http would leak the token");

        let mut d = good.clone();
        d.url = "https://user:pw@example.com/usage".into();
        assert!(d.validate().is_err());

        let mut d = good.clone();
        d.auth = CustomAuth {
            scheme: AuthScheme::Header,
            name: "X Key\r\nEvil".into(),
        };
        assert!(d.validate().is_err());

        let mut d = good.clone();
        d.windows.push(window("Month", "other"));
        assert!(d.validate().is_err(), "duplicate labels");

        let mut d = good.clone();
        d.windows[0].kind = WindowKind::Credits;
        assert!(d.validate().is_err());

        let mut d = good.clone();
        d.id = "claude".into();
        assert!(d.validate().is_err(), "ids must not collide with built-ins");

        let mut d = good;
        d.windows.clear();
        assert!(d.validate().is_err());
    }

    #[test]
    fn normalize_drops_blank_optional_paths() {
        let mut d = def(vec![window(" Month ", " used ")]);
        d.windows[0].limit_path = Some("  ".into());
        d.normalize();
        assert_eq!(d.windows[0].label, "Month");
        assert_eq!(d.windows[0].used_path, "used");
        assert_eq!(d.windows[0].limit_path, None);
    }
}
