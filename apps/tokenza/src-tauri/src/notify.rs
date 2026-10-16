//! Threshold toasts: one per level per reset cycle, plus "limit available again".

use chrono::{DateTime, Utc};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::i18n::{self, fill, window_label};
use crate::state::{Inner, Notified};

pub struct Toast {
    pub title: String,
    pub body: String,
}

/// Cycle ends reported by providers jitter by microseconds; treat close ones as equal.
fn same_cycle(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a - b).num_minutes().abs() < 10,
        (None, None) => true,
        _ => false,
    }
}

/// Compares fresh snapshots against what was already announced and returns the toasts to show.
pub fn evaluate(g: &mut Inner) -> Vec<Toast> {
    let (warn, crit, notify_reset) = (
        g.settings.warn_pct,
        g.settings.crit_pct,
        g.settings.notify_reset,
    );
    let lang = g.settings.resolved_lang();
    let tr = i18n::strings(lang);
    let mut toasts = Vec::new();
    let mut updates: Vec<(String, Notified)> = Vec::new();

    for p in g
        .providers
        .iter()
        .filter(|p| p.enabled && g.settings.notifies(&p.id))
    {
        let Some(snap) = &p.snapshot else { continue };
        let name = g.settings.display_name(&p.id);
        for w in snap
            .windows
            .iter()
            .filter(|w| g.settings.window_visible(&p.id, &w.label))
        {
            let Some(pct) = w.used_pct else { continue };
            let key = format!("{}:{}", p.id, w.label);
            let level: u8 = if pct >= crit {
                2
            } else if pct >= warn {
                1
            } else {
                0
            };
            let prev = g.notified.get(&key);
            let prev_level = match prev {
                Some(n) if same_cycle(n.resets_at, w.resets_at) => n.level,
                _ => 0,
            };
            if level > prev_level {
                let window = window_label(lang, w.kind, &w.label);
                let title = fill(tr.used_title, &[&name, &pct.round().to_string()]);
                let body = fill(
                    tr.used_body,
                    &[&window, &(100.0 - pct).max(0.0).round().to_string()],
                );
                toasts.push(Toast { title, body });
                updates.push((
                    key,
                    Notified {
                        level,
                        resets_at: w.resets_at,
                    },
                ));
            } else if level == 0 && prev.is_some_and(|n| n.level > 0) {
                if notify_reset {
                    let window = window_label(lang, w.kind, &w.label);
                    toasts.push(Toast {
                        title: fill(tr.reset_title, &[&name]),
                        body: fill(tr.reset_body, &[&window]),
                    });
                }
                updates.push((
                    key,
                    Notified {
                        level: 0,
                        resets_at: w.resets_at,
                    },
                ));
            } else if prev.is_some() && prev_level == level {
                // Same cycle, same level: only follow the cycle end so a later cycle change is detected.
                updates.push((
                    key,
                    Notified {
                        level: prev_level,
                        resets_at: w.resets_at,
                    },
                ));
            }
        }
    }
    for (k, v) in updates {
        g.notified.insert(k, v);
    }
    toasts
}

/// Shows a sample toast in the user's language with their warning threshold; reports a failure to the caller.
pub fn show_test(app: &AppHandle, lang: &str, warn_pct: f32) -> Result<(), String> {
    let tr = i18n::strings(lang);
    app.notification()
        .builder()
        .title(tr.test_title)
        .body(fill(tr.test_body, &[&warn_pct.round().to_string()]))
        .show()
        .map_err(|e| e.to_string())
}

pub fn show(app: &AppHandle, toasts: Vec<Toast>) {
    for t in toasts {
        // A failed toast (notifications disabled in Windows) must not break polling.
        let _ = app
            .notification()
            .builder()
            .title(t.title)
            .body(t.body)
            .show();
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use std::collections::HashMap;

    use tokenza_core::{UsageSnapshot, UsageWindow, WindowKind};

    use super::*;
    use crate::settings::Settings;
    use crate::state::ProviderState;

    fn inner(pct: f32, resets: DateTime<Utc>, notified: HashMap<String, Notified>) -> Inner {
        let mut p = ProviderState::new("codex", true);
        p.snapshot = Some(UsageSnapshot {
            provider: "codex".into(),
            account: "plus".into(),
            windows: vec![UsageWindow {
                kind: WindowKind::Session5h,
                label: "5 h".into(),
                used_pct: Some(pct),
                resets_at: Some(resets),
                detail: None,
            }],
            fetched_at: Utc::now(),
        });
        let mut settings = Settings::default();
        settings.lang = "en".into();
        Inner {
            providers: vec![p],
            settings,
            notified,
            flyout_anchor: None,
            last_hidden: None,
            tuning_saved: String::new(),
            secrets: Default::default(),
        }
    }

    #[test]
    fn notifies_once_per_level_and_cycle() {
        let reset = Utc::now() + chrono::Duration::hours(2);
        let mut g = inner(82.0, reset, HashMap::new());
        assert_eq!(evaluate(&mut g).len(), 1, "first crossing of warn notifies");
        assert!(
            evaluate(&mut g).is_empty(),
            "same level, same cycle: silent"
        );
        g.providers[0].snapshot.as_mut().unwrap().windows[0].used_pct = Some(97.0);
        assert_eq!(evaluate(&mut g).len(), 1, "crit is a new level");
    }

    #[test]
    fn respects_provider_notify_switch_hidden_windows_and_alias() {
        let reset = Utc::now() + chrono::Duration::hours(2);
        // The provider's toasts are switched off.
        let mut g = inner(97.0, reset, HashMap::new());
        g.settings
            .providers
            .iter_mut()
            .find(|p| p.id == "codex")
            .unwrap()
            .notify = false;
        assert!(evaluate(&mut g).is_empty());
        // The only window is hidden.
        let mut g = inner(97.0, reset, HashMap::new());
        g.settings
            .providers
            .iter_mut()
            .find(|p| p.id == "codex")
            .unwrap()
            .hidden_windows = vec!["5 h".into()];
        assert!(evaluate(&mut g).is_empty());
        // The alias replaces the built-in name.
        let mut g = inner(97.0, reset, HashMap::new());
        g.settings
            .providers
            .iter_mut()
            .find(|p| p.id == "codex")
            .unwrap()
            .alias = Some("Work Codex".into());
        let t = evaluate(&mut g);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.starts_with("Work Codex"), "{}", t[0].title);
    }

    #[test]
    fn announces_reset_and_new_cycle() {
        let reset = Utc::now() + chrono::Duration::hours(2);
        let mut g = inner(97.0, reset, HashMap::new());
        evaluate(&mut g);
        // Window resets: usage back to 5% on a new cycle.
        {
            let w = &mut g.providers[0].snapshot.as_mut().unwrap().windows[0];
            w.used_pct = Some(5.0);
            w.resets_at = Some(reset + chrono::Duration::hours(5));
        }
        let t = evaluate(&mut g);
        assert_eq!(t.len(), 1);
        assert!(t[0].title.contains("available again"));
        // Next cycle crosses warn again: must notify again.
        g.providers[0].snapshot.as_mut().unwrap().windows[0].used_pct = Some(85.0);
        assert_eq!(evaluate(&mut g).len(), 1);
    }
}
