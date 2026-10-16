//! Background polling: each provider on its own schedule, with backoff on failures.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};
use tokenza_core::{all_providers, Provider, ProviderError, UsageSnapshot};

use crate::adaptive::MAX_BACKOFF;
use crate::state::{AppState, ErrorInfo};
use crate::{notify, tray};

const TICK: Duration = Duration::from_secs(5);
const NOT_FOUND_RECHECK: Duration = Duration::from_secs(60);
const OFFLINE_RETRY: Duration = Duration::from_secs(30);
/// Manual refresh never hits a provider more often than this.
const MANUAL_COOLDOWN: Duration = Duration::from_secs(30);

enum Outcome {
    NotFound,
    Fetched(Result<UsageSnapshot, ProviderError>),
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let providers: Vec<Arc<dyn Provider>> =
            all_providers().into_iter().map(Arc::from).collect();
        let wake = app.state::<AppState>().wake.clone();
        {
            // Each provider's own minimum interval bounds what the tuner may choose.
            let state = app.state::<AppState>();
            let mut g = state.lock();
            for p in &providers {
                if let Some(st) = g.providers.iter_mut().find(|s| s.id == p.id()) {
                    st.min_secs = p.min_interval().as_secs();
                }
            }
        }
        let mut manual = false;
        loop {
            round(&app, &providers, manual).await;
            manual = false;
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = wake.notified() => { manual = true; }
            }
        }
    });
}

async fn round(app: &AppHandle, providers: &[Arc<dyn Provider>], manual: bool) {
    let state = app.state::<AppState>();
    let now = Instant::now();

    let due: Vec<Arc<dyn Provider>> = {
        let mut g = state.lock();
        let mut due = Vec::new();
        for p in providers {
            let enabled = g.settings.is_enabled(p.id());
            let Some(st) = g.providers.iter_mut().find(|s| s.id == p.id()) else {
                continue;
            };
            st.enabled = enabled;
            if !enabled {
                continue;
            }
            let scheduled = st.next_due.is_none_or(|t| t <= now);
            let cooled = st.last_attempt.is_none_or(|t| {
                (Utc::now() - t)
                    .to_std()
                    .map_or(true, |d| d >= MANUAL_COOLDOWN)
            });
            if scheduled || (manual && cooled) {
                due.push(p.clone());
            }
        }
        due
    };
    if due.is_empty() {
        // A manual refresh that found nothing due still answers, so the UI can stop its spinner.
        if manual {
            publish(app);
        }
        return;
    }

    let handles: Vec<_> = due
        .into_iter()
        .map(|p| {
            tauri::async_runtime::spawn(async move {
                let id = p.id();
                let min = p.min_interval();
                let outcome = match p.detect() {
                    None => Outcome::NotFound,
                    Some(acc) => Outcome::Fetched(p.fetch(&acc).await),
                };
                (id, min, outcome)
            })
        })
        .collect();

    let mut toasts = Vec::new();
    for h in handles {
        let Ok((id, min, outcome)) = h.await else {
            continue;
        };
        let mut g = state.lock();
        let params = g.settings.params(id, min);
        let Some(st) = g.providers.iter_mut().find(|s| s.id == id) else {
            continue;
        };
        st.last_attempt = Some(Utc::now());
        st.min_secs = min.as_secs();
        match outcome {
            Outcome::NotFound => {
                st.status = "not_found";
                st.snapshot = None;
                st.error = None;
                st.next_due = Some(Instant::now() + NOT_FOUND_RECHECK);
            }
            Outcome::Fetched(Ok(snap)) => {
                let worst = snap
                    .windows
                    .iter()
                    .filter_map(|w| w.used_pct)
                    .reduce(f32::max);
                st.status = "ok";
                st.snapshot = Some(snap);
                st.error = None;
                st.fails = 0;
                st.next_due = Some(Instant::now() + st.tuner.on_success(&params, worst));
            }
            Outcome::Fetched(Err(e)) => {
                st.status = "error";
                st.error = Some(ErrorInfo::from(&e));
                // Rate limits, outages and schema errors back off; auth errors just wait for a re-login.
                #[cfg(debug_assertions)]
                eprintln!("[{id}] fetch failed: {e}");
                let delay = match e {
                    ProviderError::RateLimited(retry_after) => {
                        st.tuner.on_rate_limited(&params, retry_after)
                    }
                    // A dropped connection is usually brief: retry soon, then back off.
                    ProviderError::Offline(_) => {
                        st.fails = st.fails.saturating_add(1);
                        (OFFLINE_RETRY * (1u32 << (st.fails - 1).min(6))).min(MAX_BACKOFF)
                    }
                    ProviderError::SchemaChanged(_) => {
                        st.fails = st.fails.saturating_add(1);
                        params
                            .base
                            .saturating_mul(1u32 << st.fails.min(5))
                            .min(MAX_BACKOFF)
                    }
                    _ => st.tuner.current(&params),
                };
                st.next_due = Some(Instant::now() + delay);
            }
        }
        drop(g);
    }

    state.save_tuning();
    toasts.extend(notify::evaluate(&mut state.lock()));
    notify::show(app, toasts);
    publish(app);
}

/// Pushes the current state to the tray and to every open window.
pub fn publish(app: &AppHandle) {
    tray::refresh(app);
    let _ = app.emit("usage-updated", app.state::<AppState>().dto());
}
