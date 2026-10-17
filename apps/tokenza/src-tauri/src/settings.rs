use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokenza_core::{CustomDef, WindowKind};

use crate::adaptive::Params;
use crate::i18n;

/// Bumped when a saved setting needs migrating.
/// 2: `lang` used to be stored as "ru" by default; it is now "auto" (follow the system).
/// 3: the tray icon got its own source setting; providers switched off for it mean "selected".
const SCHEMA: u32 = 3;

fn legacy_schema() -> u32 {
    1
}

/// Provider ids in default card order.
pub const KNOWN_PROVIDERS: [&str; 7] = [
    "claude",
    "codex",
    "cursor",
    "opencode",
    "commandcode",
    "devin",
    "antigravity",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderPref {
    pub id: String,
    pub enabled: bool,
    /// Own refresh interval for this provider; `None` follows the global one.
    pub interval_secs: Option<u64>,
    /// Whether this provider's limits count toward the tray icon when `tray_source` is "selected".
    pub show_in_tray: bool,
    /// Whether this provider also gets a tray icon of its own, next to the main one.
    pub own_tray_icon: bool,
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
            own_tray_icon: false,
            notify: true,
            hidden_windows: Vec::new(),
            alias: None,
        }
    }
}

/// Limit kinds the tray icon can be restricted to, as stored in `Settings::tray_kinds`.
pub const TRAY_KINDS: [&str; 4] = ["session", "daily", "weekly", "monthly"];

/// More than this is a cluttered flyout and a lot of polling.
pub const CUSTOM_MAX: usize = 8;
const ALIAS_MAX_CHARS: usize = 24;
const HIDDEN_WINDOWS_MAX: usize = 16;

/// Replaces a file in one step: a crash or power cut mid-write would otherwise leave a truncated
/// settings file, which loads as the defaults and loses every choice and custom provider.
/// Creates the folder first, which does not exist until something is saved on a fresh install.
pub fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

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
    /// Global Ctrl+Alt+T shortcut that toggles the flyout in the middle of the screen.
    pub hotkey_enabled: bool,
    /// Accelerator of that shortcut, e.g. "Ctrl+Alt+T"; always usable after `normalize`.
    pub hotkey: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "auto" (follow the system language) or one of `i18n::SUPPORTED`.
    pub lang: String,
    /// "ring" | "bars" | "pie" | "number"
    pub tray_style: String,
    /// Whose limits the tray icon shows: "all" enabled providers, the "selected" ones
    /// (`ProviderPref::show_in_tray`), or only the "single" one in `tray_provider`.
    pub tray_source: String,
    /// The provider shown alone when `tray_source` is "single".
    pub tray_provider: String,
    /// Kinds of limit windows the tray icon looks at, from `TRAY_KINDS`; never empty.
    pub tray_kinds: Vec<String>,
    /// Flyout density: "standard" | "compact"
    pub view_mode: String,
    /// Order of the list is the order of the cards.
    pub providers: Vec<ProviderPref>,
    /// Providers the user defined; their keys are in the credential store, not here.
    pub custom_providers: Vec<CustomDef>,
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
            hotkey_enabled: true,
            hotkey: crate::hotkey::DEFAULT.into(),
            theme: "system".into(),
            lang: "auto".into(),
            tray_style: "ring".into(),
            tray_source: "all".into(),
            tray_provider: KNOWN_PROVIDERS[0].into(),
            tray_kinds: TRAY_KINDS.iter().map(|k| (*k).into()).collect(),
            view_mode: "standard".into(),
            providers: KNOWN_PROVIDERS
                .iter()
                .map(|id| ProviderPref {
                    id: (*id).into(),
                    enabled: true,
                    ..Default::default()
                })
                .collect(),
            custom_providers: Vec::new(),
        }
    }
}

impl Settings {
    /// Makes sure every known provider has an entry and no unknown or duplicate ones stay.
    pub fn normalize(&mut self) {
        self.hotkey = crate::hotkey::normalize(&self.hotkey);
        // A hand-edited or damaged definition is dropped rather than polled with a broken request.
        let mut ids: Vec<String> = Vec::new();
        self.custom_providers.retain_mut(|d| {
            d.normalize();
            let keep = d.validate().is_ok()
                && ids.len() < CUSTOM_MAX
                && !KNOWN_PROVIDERS.contains(&d.id.as_str())
                && !ids.contains(&d.id);
            if keep {
                ids.push(d.id.clone());
            }
            keep
        });
        let known = |id: &str| KNOWN_PROVIDERS.contains(&id) || ids.iter().any(|i| i == id);

        let mut seen = Vec::new();
        self.providers.retain(|p| {
            let keep = known(&p.id) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id.clone());
            }
            keep
        });
        let missing: Vec<String> = KNOWN_PROVIDERS
            .iter()
            .map(|id| id.to_string())
            .chain(ids.iter().cloned())
            .filter(|id| !seen.contains(id))
            .collect();
        for id in missing {
            self.providers.push(ProviderPref {
                id,
                enabled: true,
                ..Default::default()
            });
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
        if !matches!(self.tray_style.as_str(), "ring" | "bars" | "pie" | "number") {
            self.tray_style = "ring".into();
        }
        if !matches!(self.tray_source.as_str(), "all" | "selected" | "single") {
            self.tray_source = "all".into();
        }
        if !self.providers.iter().any(|p| p.id == self.tray_provider) {
            self.tray_provider = self
                .providers
                .first()
                .map_or_else(|| KNOWN_PROVIDERS[0].into(), |p| p.id.clone());
        }
        let mut seen: Vec<String> = Vec::new();
        self.tray_kinds.retain(|k| {
            let keep = TRAY_KINDS.contains(&k.as_str()) && !seen.contains(k);
            if keep {
                seen.push(k.clone());
            }
            keep
        });
        // An icon that watches nothing would always look empty, so no choice means every kind.
        if self.tray_kinds.is_empty() {
            self.tray_kinds = TRAY_KINDS.iter().map(|k| (*k).into()).collect();
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
        if self.schema < 3 && self.providers.iter().any(|p| !p.show_in_tray) {
            // Until now switching a provider off for the tray was the only way to narrow it down.
            self.tray_source = "selected".into();
        }
        self.schema = SCHEMA;
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        write_atomic(
            path,
            &serde_json::to_string_pretty(self).unwrap_or_default(),
        )
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

    /// Whether the tray icon looks at this provider's limits.
    pub fn counts_in_tray(&self, id: &str) -> bool {
        match self.tray_source.as_str() {
            "single" => self.tray_provider == id,
            "selected" => self.pref(id).is_none_or(|p| p.show_in_tray),
            _ => true,
        }
    }

    /// Whether the tray icon looks at limit windows of this kind.
    pub fn counts_kind_in_tray(&self, kind: WindowKind) -> bool {
        let key = match kind {
            WindowKind::Session5h => "session",
            WindowKind::Daily => "daily",
            WindowKind::Weekly => "weekly",
            WindowKind::Monthly => "monthly",
            // A balance has no percentage, so there is nothing for the icon to draw.
            WindowKind::Credits => return true,
        };
        self.tray_kinds.iter().any(|k| k == key)
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
            "opencode" => "OpenCode",
            "commandcode" => "Command Code",
            "devin" => "Devin",
            "antigravity" => "Antigravity",
            other => self
                .custom_providers
                .iter()
                .find(|d| d.id == other)
                .map_or(other, |d| d.name.as_str()),
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
    fn save_replaces_the_file_and_leaves_no_temporary_one() {
        let dir = std::env::temp_dir().join(format!("tokenza-save-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.save(&path).unwrap();
        s.interval_secs = 600;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).interval_secs, 600);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
        assert_eq!(
            ids,
            [
                "cursor",
                "claude",
                "codex",
                "opencode",
                "commandcode",
                "devin",
                "antigravity"
            ]
        );
        assert!(!s.is_enabled("cursor"));
        assert_eq!(s.interval_secs, 60);
        assert!(s.crit_pct >= s.warn_pct);
    }

    #[test]
    fn custom_providers_are_validated_and_get_a_card() {
        use tokenza_core::{CustomDef, CustomWindow};
        let good = CustomDef {
            id: "custom-acme".into(),
            name: "Acme".into(),
            url: "https://api.acme.example/usage".into(),
            windows: vec![CustomWindow {
                label: "Month".into(),
                used_path: "used".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let insecure = CustomDef {
            id: "custom-bad".into(),
            url: "http://api.acme.example/usage".into(),
            ..good.clone()
        };
        let clash = CustomDef {
            id: "claude".into(),
            ..good.clone()
        };
        let mut s = Settings::default();
        s.custom_providers = vec![good.clone(), good, insecure, clash];
        s.normalize();
        assert_eq!(
            s.custom_providers.len(),
            1,
            "invalid and duplicate definitions are dropped"
        );
        let ids: Vec<_> = s.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.last(), Some(&"custom-acme"));
        assert_eq!(s.display_name("custom-acme"), "Acme");

        s.custom_providers.clear();
        s.normalize();
        assert!(
            !s.providers.iter().any(|p| p.id == "custom-acme"),
            "a removed provider loses its card"
        );
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
        s.tray_source = "selected".into();
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
    fn tray_source_decides_which_providers_count() {
        let mut s = Settings::default();
        s.providers
            .iter_mut()
            .find(|p| p.id == "codex")
            .unwrap()
            .show_in_tray = false;
        assert!(
            s.counts_in_tray("codex"),
            "\"all\" ignores the per-provider flag"
        );

        s.tray_source = "selected".into();
        assert!(!s.counts_in_tray("codex") && s.counts_in_tray("claude"));

        s.tray_source = "single".into();
        s.tray_provider = "cursor".into();
        assert!(s.counts_in_tray("cursor"));
        assert!(!s.counts_in_tray("claude") && !s.counts_in_tray("codex"));
    }

    #[test]
    fn tray_kinds_filter_window_kinds() {
        let mut s = Settings::default();
        assert!(s.counts_kind_in_tray(WindowKind::Session5h));
        s.tray_kinds = vec!["weekly".into()];
        assert!(s.counts_kind_in_tray(WindowKind::Weekly));
        assert!(!s.counts_kind_in_tray(WindowKind::Session5h));
        assert!(!s.counts_kind_in_tray(WindowKind::Monthly));
    }

    #[test]
    fn normalize_repairs_tray_choices() {
        let mut s = Settings::default();
        s.tray_style = "hologram".into();
        s.tray_source = "everything".into();
        s.tray_provider = "bogus".into();
        s.tray_kinds = vec!["weekly".into(), "weekly".into(), "yearly".into()];
        s.normalize();
        assert_eq!(s.tray_style, "ring");
        assert_eq!(s.tray_source, "all");
        assert_eq!(
            s.tray_provider, "claude",
            "falls back to the first provider"
        );
        assert_eq!(
            s.tray_kinds,
            vec!["weekly".to_string()],
            "unknown kinds and duplicates are dropped"
        );

        s.tray_kinds = vec!["yearly".into()];
        s.normalize();
        assert_eq!(
            s.tray_kinds.len(),
            TRAY_KINDS.len(),
            "empty means every kind"
        );

        s.tray_style = "pie".into();
        s.tray_provider = "devin".into();
        s.normalize();
        assert_eq!(
            (s.tray_style.as_str(), s.tray_provider.as_str()),
            ("pie", "devin")
        );
    }

    #[test]
    fn providers_switched_off_for_the_tray_become_a_selection() {
        let dir = std::env::temp_dir().join(format!("tokenza-tray-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");

        // A schema-2 file: the only way to narrow the tray was the per-provider switch.
        std::fs::write(
            &path,
            r#"{"schema":2,"providers":[{"id":"claude","show_in_tray":false}]}"#,
        )
        .unwrap();
        let s = Settings::load(&path);
        assert_eq!(s.tray_source, "selected");
        assert!(!s.counts_in_tray("claude") && s.counts_in_tray("codex"));

        std::fs::write(&path, r#"{"schema":2,"providers":[{"id":"claude"}]}"#).unwrap();
        assert_eq!(
            Settings::load(&path).tray_source,
            "all",
            "nothing was switched off, nothing to migrate"
        );
        let _ = std::fs::remove_dir_all(&dir);
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
