use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::adaptive::Params;
use crate::i18n;

/// Bumped when a saved setting needs migrating.
/// 2: `lang` used to be stored as "ru" by default; it is now "auto" (follow the system).
const SCHEMA: u32 = 2;

fn legacy_schema() -> u32 {
    1
}

/// Provider ids in default card order.
pub const KNOWN_PROVIDERS: [&str; 3] = ["claude", "codex", "cursor"];

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderPref {
    pub id: String,
    pub enabled: bool,
    /// Own refresh interval for this provider; `None` follows the global one.
    pub interval_secs: Option<u64>,
    /// Whether this provider's limits count toward the tray icon.
    pub show_in_tray: bool,
    /// Whether this provider may send threshold toasts.
    pub notify: bool,
    /// Labels of limit windows to hide everywhere (flyout, tray, toasts), e.g. "Week · Opus".
    pub hidden_windows: Vec<String>,
    /// Own display name, shown instead of the provider's default one.
    pub alias: Option<String>,
}

impl Default for ProviderPref {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            interval_secs: None,
            show_in_tray: true,
            notify: true,
            hidden_windows: Vec::new(),
            alias: None,
        }
    }
}

const ALIAS_MAX_CHARS: usize = 24;
const HIDDEN_WINDOWS_MAX: usize = 16;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Settings file version, for migrations in `load`. A file without it predates versioning.
    #[serde(default = "legacy_schema")]
    pub schema: u32,
    /// "smart" tunes the interval itself; "fixed" keeps it (still obeys rate limits).
    pub update_mode: String,
    /// Base interval; in smart mode the tuner starts from it.
    pub interval_secs: u64,
    /// Smart-mode bounds for the tuned interval.
    pub min_interval_secs: u64,
    pub max_interval_secs: u64,
    /// Poll twice as often while a window is at or above the warning threshold.
    pub boost_near_limit: bool,
    /// Poll less often while usage does not change.
    pub slow_when_idle: bool,
    pub warn_pct: f32,
    pub crit_pct: f32,
    pub notify_reset: bool,
    pub autostart: bool,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "auto" (follow the system language) or one of `i18n::SUPPORTED`.
    pub lang: String,
    /// "ring" | "bars"
    pub tray_style: String,
    /// Flyout density: "standard" | "compact"
    pub view_mode: String,
    /// Order of the list is the order of the cards.
    pub providers: Vec<ProviderPref>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            update_mode: "smart".into(),
            interval_secs: 180,
            min_interval_secs: 60,
            max_interval_secs: 1800,
            boost_near_limit: true,
            slow_when_idle: true,
            warn_pct: 80.0,
            crit_pct: 95.0,
            notify_reset: true,
            autostart: false,
            theme: "system".into(),
            lang: "auto".into(),
            tray_style: "ring".into(),
            view_mode: "standard".into(),
            providers: KNOWN_PROVIDERS
                .iter()
                .map(|id| ProviderPref {
                    id: (*id).into(),
                    enabled: true,
                    ..Default::default()
                })
                .collect(),
        }
    }
}

impl Settings {
    /// Makes sure every known provider has an entry and no unknown or duplicate ones stay.
    pub fn normalize(&mut self) {
        let mut seen = Vec::new();
        self.providers.retain(|p| {
            let keep = KNOWN_PROVIDERS.contains(&p.id.as_str()) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id.clone());
            }
            keep
        });
        for id in KNOWN_PROVIDERS {
            if !seen.iter().any(|s| s == id) {
                self.providers.push(ProviderPref {
                    id: id.into(),
                    enabled: true,
                    ..Default::default()
                });
            }
        }
        self.interval_secs = self.interval_secs.clamp(60, 3600);
        self.min_interval_secs = self.min_interval_secs.clamp(30, 3600);
        self.max_interval_secs = self
            .max_interval_secs
            .clamp(self.min_interval_secs, 6 * 3600);
        for p in &mut self.providers {
            p.interval_secs = p.interval_secs.map(|v| v.clamp(60, 3600));
            p.alias = p
                .alias
                .take()
                .map(|a| a.trim().chars().take(ALIAS_MAX_CHARS).collect::<String>())
                .filter(|a| !a.is_empty());
            let mut seen: Vec<String> = Vec::new();
            for w in p.hidden_windows.drain(..) {
                if w.len() <= 40 && !seen.contains(&w) && seen.len() < HIDDEN_WINDOWS_MAX {
                    seen.push(w);
                }
            }
            p.hidden_windows = seen;
        }
        if !matches!(self.update_mode.as_str(), "smart" | "fixed") {
            self.update_mode = "smart".into();
        }
        self.warn_pct = self.warn_pct.clamp(1.0, 100.0);
        self.crit_pct = self.crit_pct.clamp(self.warn_pct, 100.0);
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            self.theme = "system".into();
        }
        if self.lang != "auto" && !i18n::SUPPORTED.contains(&self.lang.as_str()) {
            self.lang = "auto".into();
        }
        if !matches!(self.tray_style.as_str(), "ring" | "bars") {
            self.tray_style = "ring".into();
        }
        if !matches!(self.view_mode.as_str(), "standard" | "compact") {
            self.view_mode = "standard".into();
        }
    }

    pub fn load(path: &PathBuf) -> Self {
        let mut s: Settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        s.migrate();
        s.normalize();
        s
    }

    /// Brings a file written by an older version up to date.
    fn migrate(&mut self) {
        if self.schema < 2 {
            // Older versions saved the default language as an explicit choice, so it cannot be trusted.
            self.lang = "auto".into();
        }
        self.schema = SCHEMA;
    }

    pub fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap_or_default())
    }

    /// Tuning inputs for one provider; `provider_min` is the interval its API tolerates at least.
    pub fn params(&self, id: &str, provider_min: std::time::Duration) -> Params {
        use std::time::Duration;
        let secs = Duration::from_secs;
        let lo = secs(self.min_interval_secs).max(provider_min);
        let hi = secs(self.max_interval_secs).max(lo);
        let own = self
            .providers
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.interval_secs);
        Params {
            smart: self.update_mode == "smart",
            base: secs(own.unwrap_or(self.interval_secs)).clamp(lo, hi),
            lo,
            hi,
            boost_near_limit: self.boost_near_limit,
            slow_when_idle: self.slow_when_idle,
            warn_pct: self.warn_pct,
        }
    }

    /// The language to actually use: the explicit pick, or the system's for "auto".
    pub fn resolved_lang(&self) -> &'static str {
        match i18n::SUPPORTED.iter().find(|l| **l == self.lang) {
            Some(l) => l,
            None => i18n::system_lang(),
        }
    }

    pub fn pref(&self, id: &str) -> Option<&ProviderPref> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// False when the user hid this limit window for the provider.
    pub fn window_visible(&self, id: &str, label: &str) -> bool {
        !self
            .pref(id)
            .is_some_and(|p| p.hidden_windows.iter().any(|h| h == label))
    }

    pub fn counts_in_tray(&self, id: &str) -> bool {
        self.pref(id).is_none_or(|p| p.show_in_tray)
    }

    pub fn notifies(&self, id: &str) -> bool {
        self.pref(id).is_none_or(|p| p.notify)
    }

    /// The name to show for a provider: the user's alias, else the built-in one.
    pub fn display_name(&self, id: &str) -> String {
        if let Some(a) = self.pref(id).and_then(|p| p.alias.clone()) {
            return a;
        }
        match id {
            "claude" => "Claude Code",
            "codex" => "Codex",
            "cursor" => "Cursor",
            other => other,
        }
        .to_string()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.providers
            .iter()
            .find(|p| p.id == id)
            .is_none_or(|p| p.enabled)
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_missing_and_drops_unknown() {
        let mut s = Settings::default();
        s.providers = vec![
            ProviderPref {
                id: "cursor".into(),
                enabled: false,
                interval_secs: Some(5),
                alias: Some("  ".into()),
                hidden_windows: vec!["a".into(), "a".into()],
                ..Default::default()
            },
            ProviderPref {
                id: "bogus".into(),
                enabled: true,
                ..Default::default()
            },
        ];
        s.crit_pct = 10.0;
        s.interval_secs = 5;
        s.view_mode = "tiny".into();
        s.lang = "klingon".into();
        s.normalize();
        assert_eq!(s.view_mode, "standard");
        assert_eq!(s.lang, "auto", "unknown language falls back to auto");
        assert_eq!(
            s.providers[0].interval_secs,
            Some(60),
            "per-provider interval is clamped"
        );
        assert_eq!(s.providers[0].alias, None, "a blank alias is dropped");
        assert_eq!(
            s.providers[0].hidden_windows,
            vec!["a".to_string()],
            "duplicate hidden windows collapse"
        );
        let ids: Vec<_> = s.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["cursor", "claude", "codex"]);
        assert!(!s.is_enabled("cursor"));
        assert_eq!(s.interval_secs, 60);
        assert!(s.crit_pct >= s.warn_pct);
    }

    #[test]
    fn old_settings_files_get_auto_language() {
        let dir = std::env::temp_dir().join(format!("tokenza-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");

        std::fs::write(&path, r#"{"lang":"ru","theme":"dark"}"#).unwrap();
        let s = Settings::load(&path);
        assert_eq!(
            s.lang, "auto",
            "pre-schema files do not carry a real language choice"
        );
        assert_eq!(s.theme, "dark", "everything else is kept");

        s.save(&path).unwrap();
        let mut again = Settings::load(&path);
        again.lang = "de".into();
        again.save(&path).unwrap();
        assert_eq!(
            Settings::load(&path).lang,
            "de",
            "a choice made after migration sticks"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn provider_prefs_drive_visibility_and_names() {
        let mut s = Settings::default();
        assert!(
            s.window_visible("claude", "Week")
                && s.counts_in_tray("claude")
                && s.notifies("claude")
        );
        assert_eq!(s.display_name("claude"), "Claude Code");
        let p = s.providers.iter_mut().find(|p| p.id == "claude").unwrap();
        p.hidden_windows = vec!["Week · Opus".into()];
        p.show_in_tray = false;
        p.notify = false;
        p.alias = Some("Work Claude".into());
        assert!(!s.window_visible("claude", "Week · Opus"));
        assert!(
            s.window_visible("claude", "Week"),
            "only the listed window is hidden"
        );
        assert!(!s.counts_in_tray("claude") && !s.notifies("claude"));
        assert_eq!(s.display_name("claude"), "Work Claude");
        assert!(
            s.window_visible("codex", "Week · Opus"),
            "other providers are unaffected"
        );
    }

    #[test]
    fn explicit_language_wins_over_system() {
        let mut s = Settings::default();
        s.lang = "de".into();
        assert_eq!(s.resolved_lang(), "de");
        s.lang = "auto".into();
        assert!(i18n::SUPPORTED.contains(&s.resolved_lang()));
    }

    #[test]
    fn params_follow_provider_minimum_and_overrides() {
        use std::time::Duration;
        let mut s = Settings::default();
        s.providers[1].interval_secs = Some(600);
        let claude = s.params("claude", Duration::from_secs(180));
        assert_eq!(claude.base, Duration::from_secs(180));
        let codex = s.params("codex", Duration::from_secs(180));
        assert_eq!(
            codex.base,
            Duration::from_secs(600),
            "provider override wins"
        );
        let cursor = s.params("cursor", Duration::from_secs(300));
        assert_eq!(
            cursor.base,
            Duration::from_secs(300),
            "raised to the provider minimum"
        );
        assert_eq!(cursor.lo, Duration::from_secs(300));
    }
}
