use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokenza_core::{ProviderError, UsageSnapshot};
use tokio::sync::Notify;

use crate::adaptive::{Learned, TuneState, Tuner};
use crate::settings::Settings;

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
    #[serde(skip)]
    pub tuner: Tuner,
    /// The provider's own minimum interval, learned on its first poll.
    #[serde(skip)]
    pub min_secs: u64,
    #[serde(skip)]
    pub fails: u32,
    #[serde(skip)]
    pub next_due: Option<Instant>,
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
            tuner: Tuner::default(),
            min_secs: 60,
            fails: 0,
            next_due: None,
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

pub struct Inner {
    pub providers: Vec<ProviderState>,
    pub settings: Settings,
    pub notified: HashMap<String, Notified>,
    pub flyout_anchor: Option<(i32, i32, u32, u32)>,
    pub last_hidden: Option<Instant>,
    /// Last tuning file content, to write only on change.
    pub tuning_saved: String,
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

impl AppState {
    pub fn new(settings_path: PathBuf) -> Self {
        let settings = Settings::load(&settings_path);
        let learned: HashMap<String, Learned> =
            std::fs::read_to_string(tuning_path(&settings_path))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
        let providers = settings
            .providers
            .iter()
            .map(|p| {
                let mut st = ProviderState::new(&p.id, p.enabled);
                if let Some(l) = learned.get(&p.id) {
                    st.tuner = Tuner::from_learned(*l);
                }
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
        if text != g.tuning_saved {
            let _ = std::fs::write(tuning_path(&self.settings_path), &text);
            g.tuning_saved = text;
        }
    }

    /// Applies new settings: keeps provider order and per-provider state in sync.
    pub fn apply_settings(&self, mut s: Settings) -> Settings {
        s.normalize();
        let mut g = self.lock();
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
