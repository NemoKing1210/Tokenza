use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokenza_core::{CustomDef, ProviderError, UsageSnapshot};
use tokio::sync::Notify;

use crate::adaptive::{Learned, TuneState, Tuner};
use crate::settings::{write_atomic, Settings, CUSTOM_MAX};

#[derive(Clone, Serialize)]
pub struct ErrorInfo {
    pub kind: String,
    pub message: String,
}

impl From<&ProviderError> for ErrorInfo {
    fn from(e: &ProviderError) -> Self {
        Self {
            kind: e.kind().into(),
            message: e.to_string(),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ProviderState {
    pub id: String,
    pub enabled: bool,
    /// "pending" | "ok" | "error" | "not_found"
    pub status: &'static str,
    /// Last good snapshot, kept while the provider reports an error.
    pub snapshot: Option<UsageSnapshot>,
    pub error: Option<ErrorInfo>,
    pub last_attempt: Option<DateTime<Utc>>,
    /// Polling interval details; filled in when the state is handed to the UI.
    pub poll: Option<PollInfo>,
    /// Credential fields that have a stored value (never the values themselves).
    pub secrets: Vec<String>,
    #[serde(skip)]
    pub tuner: Tuner,
    /// The provider's own minimum interval, learned on its first poll.
    #[serde(skip)]
    pub min_secs: u64,
    #[serde(skip)]
    pub fails: u32,
    #[serde(skip)]
    pub next_due: Option<Instant>,
    /// A poll is running; the scheduler starts no second one.
    #[serde(skip)]
    pub in_flight: bool,
    /// Something changed while a poll ran (a key, the tuning), so poll again once it is back.
    #[serde(skip)]
    pub requeue: bool,
}

impl ProviderState {
    pub fn new(id: &str, enabled: bool) -> Self {
        Self {
            id: id.into(),
            enabled,
            status: "pending",
            snapshot: None,
            error: None,
            last_attempt: None,
            poll: None,
            secrets: Vec::new(),
            tuner: Tuner::default(),
            min_secs: 60,
            fails: 0,
            next_due: None,
            in_flight: false,
            requeue: false,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct PollInfo {
    /// Interval in effect right now.
    pub interval_secs: u64,
    /// What the user asked for (global or per-provider).
    pub base_secs: u64,
    /// Lowest interval the provider tolerates, from its own API.
    pub min_secs: u64,
    /// Lowest interval learned to be safe after a rate limit, if any.
    pub floor_secs: Option<u64>,
    pub state: TuneState,
    pub rate_limit_hits: u32,
    pub next_refresh_at: Option<DateTime<Utc>>,
}

/// Highest threshold level already notified for one window, with its cycle end.
pub struct Notified {
    pub level: u8,
    pub resets_at: Option<DateTime<Utc>>,
}

/// Where the flyout opens, in physical pixels.
#[derive(Clone, Copy, Debug)]
pub enum FlyoutAnchor {
    /// Next to the tray icon rect `(x, y, width, height)`.
    Tray(i32, i32, u32, u32),
    /// In the middle of the work area of the monitor containing this point (global hotkey).
    Centered(i32, i32),
}

pub struct Inner {
    pub providers: Vec<ProviderState>,
    pub settings: Settings,
    pub notified: HashMap<String, Notified>,
    pub flyout_anchor: Option<FlyoutAnchor>,
    pub last_hidden: Option<Instant>,
    /// Last tuning file content, to write only on change.
    pub tuning_saved: String,
    /// Same for the snapshot cache.
    pub snapshots_saved: String,
    /// `provider/field` of every stored credential, so the UI state needs no credential-store reads.
    pub secrets: HashSet<String>,
}

pub struct AppState {
    inner: Mutex<Inner>,
    pub settings_path: PathBuf,
    /// Wakes the scheduler for an immediate round.
    pub wake: Arc<Notify>,
}

#[derive(Clone, Serialize)]
pub struct StateDto {
    pub providers: Vec<ProviderState>,
    pub settings: Settings,
    /// What "auto" resolves to on this system.
    pub resolved_lang: &'static str,
}

fn tuning_path(settings_path: &std::path::Path) -> PathBuf {
    settings_path.with_file_name("tuning.json")
}

fn snapshots_path(settings_path: &std::path::Path) -> PathBuf {
    settings_path.with_file_name("snapshots.json")
}

/// A cached snapshot is only worth showing for windows whose cycle has not ended yet: after a
/// reset the old percentage is wrong, not just old. Windows without a reset time are kept.
fn still_valid(mut snap: UsageSnapshot, now: DateTime<Utc>) -> Option<UsageSnapshot> {
    snap.windows.retain(|w| w.resets_at.is_none_or(|t| t > now));
    (!snap.windows.is_empty()).then_some(snap)
}

impl AppState {
    pub fn new(settings_path: PathBuf) -> Self {
        let settings = Settings::load(&settings_path);
        let learned: HashMap<String, Learned> =
            std::fs::read_to_string(tuning_path(&settings_path))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
        // The last good values from the previous run, so a failed first poll (rate limit,
        // no network) still leaves something to show. A damaged file is simply ignored.
        let mut cached: HashMap<String, UsageSnapshot> =
            std::fs::read_to_string(snapshots_path(&settings_path))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
        let now = Utc::now();
        let providers = settings
            .providers
            .iter()
            .map(|p| {
                let mut st = ProviderState::new(&p.id, p.enabled);
                if let Some(l) = learned.get(&p.id) {
                    st.tuner = Tuner::from_learned(*l);
                }
                st.snapshot = cached.remove(&p.id).and_then(|s| still_valid(s, now));
                st
            })
            .collect();
        Self {
            inner: Mutex::new(Inner {
                providers,
                settings,
                notified: HashMap::new(),
                flyout_anchor: None,
                last_hidden: None,
                tuning_saved: String::new(),
                snapshots_saved: String::new(),
                secrets: HashSet::new(),
            }),
            settings_path,
            wake: Arc::new(Notify::new()),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        // A poisoned lock only means another thread panicked mid-update; the data is still usable.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn dto(&self) -> StateDto {
        let g = self.lock();
        let now = Instant::now();
        let providers = g
            .providers
            .iter()
            .map(|st| {
                let mut st = st.clone();
                st.secrets = secret_fields(&st.id)
                    .into_iter()
                    .filter(|f| g.secrets.contains(&format!("{}/{f}", st.id)))
                    .map(String::from)
                    .collect();
                let p = g.settings.params(&st.id, Duration::from_secs(st.min_secs));
                st.poll = Some(PollInfo {
                    interval_secs: st.tuner.current(&p).as_secs(),
                    base_secs: p.base.as_secs(),
                    min_secs: st.min_secs,
                    floor_secs: st.tuner.floor().map(|f| f.as_secs()),
                    state: st.tuner.state(&p),
                    rate_limit_hits: st.tuner.total_429,
                    next_refresh_at: st.next_due.filter(|_| st.enabled).map(|t| {
                        Utc::now()
                            + chrono::Duration::from_std(t.saturating_duration_since(now))
                                .unwrap_or_default()
                    }),
                });
                st
            })
            .collect();
        StateDto {
            providers,
            settings: g.settings.clone(),
            resolved_lang: crate::i18n::system_lang(),
        }
    }

    /// Forgets what was learned about one provider and polls it again soon.
    pub fn reset_tuning(&self, id: &str) {
        let mut g = self.lock();
        if let Some(st) = g.providers.iter_mut().find(|s| s.id == id) {
            st.tuner.reset();
            st.fails = 0;
            st.next_due = None;
            st.requeue = st.in_flight;
        }
    }

    /// Writes the learned floors next to the settings; skipped when nothing changed.
    pub fn save_tuning(&self) {
        let mut g = self.lock();
        let map: HashMap<String, Learned> = g
            .providers
            .iter()
            .filter_map(|p| p.tuner.learned().map(|l| (p.id.clone(), l)))
            .collect();
        let text = serde_json::to_string(&map).unwrap_or_default();
        if text != g.tuning_saved && write_atomic(&tuning_path(&self.settings_path), &text).is_ok()
        {
            g.tuning_saved = text;
        }
    }

    /// Writes the last good snapshots next to the settings; skipped when nothing changed.
    /// They hold limits and an account label, never credentials.
    pub fn save_snapshots(&self) {
        let mut g = self.lock();
        let map: HashMap<&str, &UsageSnapshot> = g
            .providers
            .iter()
            .filter_map(|p| p.snapshot.as_ref().map(|s| (p.id.as_str(), s)))
            .collect();
        let text = serde_json::to_string(&map).unwrap_or_default();
        if text != g.snapshots_saved
            && write_atomic(&snapshots_path(&self.settings_path), &text).is_ok()
        {
            g.snapshots_saved = text;
        }
    }

    /// Settings from the UI. Custom provider definitions only change through `save_custom` and
    /// `remove_custom`, so a stale copy of the settings cannot bring back a removed provider.
    pub fn apply_settings(&self, mut s: Settings) -> Settings {
        let mut g = self.lock();
        s.custom_providers = g.settings.custom_providers.clone();
        self.apply_locked(&mut g, s)
    }

    /// Adds a custom provider (empty `id`) or replaces an existing one.
    pub fn save_custom(&self, mut def: CustomDef) -> Result<(CustomDef, Settings), String> {
        let mut g = self.lock();
        let mut s = g.settings.clone();
        if def.id.is_empty() {
            if s.custom_providers.len() >= CUSTOM_MAX {
                return Err(format!("at most {CUSTOM_MAX} custom providers"));
            }
            def.id = unique_id(&def.name, &s.custom_providers);
        } else if !s.custom_providers.iter().any(|d| d.id == def.id) {
            return Err("unknown provider".into());
        }
        def.normalize();
        def.validate()?;
        match s.custom_providers.iter_mut().find(|d| d.id == def.id) {
            Some(slot) => *slot = def.clone(),
            None => s.custom_providers.push(def.clone()),
        }
        let applied = self.apply_locked(&mut g, s);
        Ok((def, applied))
    }

    pub fn remove_custom(&self, id: &str) -> Result<Settings, String> {
        let mut g = self.lock();
        let mut s = g.settings.clone();
        let before = s.custom_providers.len();
        s.custom_providers.retain(|d| d.id != id);
        if s.custom_providers.len() == before {
            return Err("unknown provider".into());
        }
        let applied = self.apply_locked(&mut g, s);
        g.secrets.retain(|k| !k.starts_with(&format!("{id}/")));
        Ok(applied)
    }

    pub fn mark_secret(&self, provider: &str, field: &str, present: bool) {
        let key = format!("{provider}/{field}");
        let mut g = self.lock();
        if present {
            g.secrets.insert(key);
        } else {
            g.secrets.remove(&key);
        }
    }

    /// Credential fields the provider accepts; empty for unknown ids.
    pub fn accepted_fields(&self, id: &str) -> Vec<&'static str> {
        let g = self.lock();
        if g.settings.custom_providers.iter().any(|d| d.id == id) || !secret_fields(id).is_empty() {
            secret_fields(id)
        } else {
            Vec::new()
        }
    }

    /// Fills the stored-credential cache from the credential store; called once at startup.
    pub fn load_secret_cache(&self) {
        let ids: Vec<String> = self.lock().providers.iter().map(|p| p.id.clone()).collect();
        let present: HashSet<String> = ids
            .iter()
            .flat_map(|id| secret_fields(id).into_iter().map(move |f| (id.clone(), f)))
            .filter(|(id, f)| crate::secrets::exists(id, f))
            .map(|(id, f)| format!("{id}/{f}"))
            .collect();
        self.lock().secrets = present;
    }

    /// Makes the scheduler poll this provider at its next tick, e.g. after its key changed.
    pub fn poll_soon(&self, id: &str) {
        if let Some(st) = self.lock().providers.iter_mut().find(|s| s.id == id) {
            st.fails = 0;
            st.next_due = None;
            st.requeue = st.in_flight;
        }
        self.wake.notify_one();
    }

    fn apply_locked(&self, g: &mut MutexGuard<'_, Inner>, mut s: Settings) -> Settings {
        s.normalize();
        let mut old: Vec<ProviderState> = std::mem::take(&mut g.providers);
        g.providers = s
            .providers
            .iter()
            .map(|p| {
                let mut st = old
                    .iter()
                    .position(|o| o.id == p.id)
                    .map(|i| old.swap_remove(i))
                    .unwrap_or_else(|| ProviderState::new(&p.id, p.enabled));
                let newly = p.enabled && !st.enabled;
                st.enabled = p.enabled;
                if newly {
                    st.next_due = None; // poll a freshly enabled provider at the next tick
                }
                st
            })
            .collect();
        g.settings = s.clone();
        // New intervals take effect without a refetch: only a due time that is too far out is pulled in.
        let now = Instant::now();
        for st in g.providers.iter_mut() {
            let p = s.params(&st.id, Duration::from_secs(st.min_secs));
            st.tuner.retarget(&p);
            let latest = now + st.tuner.current(&p);
            st.next_due = st.next_due.map(|d| d.min(latest));
        }
        s
    }
}

/// Credential fields a provider id takes: fixed for built-ins, one token for custom ones.
pub fn secret_fields(id: &str) -> Vec<&'static str> {
    if id.starts_with(tokenza_core::providers::custom::ID_PREFIX) {
        vec![tokenza_core::secrets::CUSTOM_FIELD]
    } else {
        tokenza_core::secrets::builtin_fields(id).to_vec()
    }
}

/// `custom-<slug of the name>`, with a number appended when the slug is taken.
fn unique_id(name: &str, taken: &[CustomDef]) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(16).collect();
    let slug = slug.trim_end_matches('-');
    let base = format!(
        "{}{}",
        tokenza_core::providers::custom::ID_PREFIX,
        if slug.is_empty() { "provider" } else { slug }
    );
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|d| d.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(ids: &[&str]) -> Vec<CustomDef> {
        ids.iter()
            .map(|id| CustomDef {
                id: (*id).into(),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn ids_are_slugged_and_unique() {
        assert_eq!(unique_id("My  Service!", &[]), "custom-my-service");
        assert_eq!(unique_id("Яндекс", &[]), "custom-provider");
        assert_eq!(
            unique_id("Acme", &taken(&["custom-acme", "custom-acme-2"])),
            "custom-acme-3"
        );
    }

    #[test]
    fn cached_snapshot_drops_windows_that_already_reset() {
        use tokenza_core::{UsageWindow, WindowKind};
        let now = Utc::now();
        let window = |label: &str, resets_at| UsageWindow {
            kind: WindowKind::Weekly,
            label: label.into(),
            used_pct: Some(40.0),
            resets_at,
            detail: None,
        };
        let snap = |windows| UsageSnapshot {
            provider: "codex".into(),
            account: "plus".into(),
            windows,
            subscription_ends_at: None,
            fetched_at: now,
        };
        let h = chrono::Duration::hours(1);
        let kept = still_valid(
            snap(vec![
                window("old", Some(now - h)),
                window("live", Some(now + h)),
                window("open", None),
            ]),
            now,
        )
        .unwrap();
        let labels: Vec<_> = kept.windows.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, ["live", "open"]);
        assert!(still_valid(snap(vec![window("old", Some(now - h))]), now).is_none());
    }

    #[test]
    fn secret_fields_follow_the_provider_kind() {
        assert_eq!(secret_fields("commandcode"), ["api_key"]);
        assert_eq!(secret_fields("custom-acme"), ["token"]);
        assert!(secret_fields("claude").is_empty());
    }
}
