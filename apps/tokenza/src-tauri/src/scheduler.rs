//! Background polling: each provider on its own schedule, with backoff on failures.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};
use tokenza_core::{all_providers, CustomDef, Provider, ProviderError, UsageSnapshot};

use crate::adaptive::MAX_BACKOFF;
use crate::state::{AppState, ErrorInfo};
use crate::{notify, tray};

const TICK: Duration = Duration::from_secs(5);
const NOT_FOUND_RECHECK: Duration = Duration::from_secs(60);
const OFFLINE_RETRY: Duration = Duration::from_secs(30);
/// Manual refresh never hits a provider more often than this.
const MANUAL_COOLDOWN: Duration = Duration::from_secs(30);
/// Longest one poll may take, login check included. Requests time out on their own well before
/// this; it only guarantees that a provider stuck for any other reason is polled again later.
const POLL_DEADLINE: Duration = Duration::from_secs(90);

enum Outcome {
    NotFound,
    Fetched(Result<UsageSnapshot, ProviderError>),
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let builtin: Vec<Arc<dyn Provider>> = all_providers().into_iter().map(Arc::from).collect();
        // Providers the user defined, rebuilt only when their definition changes.
        let mut custom: Vec<(CustomDef, Arc<dyn Provider>)> = Vec::new();
        let wake = app.state::<AppState>().wake.clone();
        {
            // Each provider's own minimum interval bounds what the tuner may choose.
            let state = app.state::<AppState>();
            let mut g = state.lock();
            for p in &builtin {
                if let Some(st) = g.providers.iter_mut().find(|s| s.id == p.id()) {
                    st.min_secs = p.min_interval().as_secs();
                }
            }
        }
        let mut manual = false;
        loop {
            sync_custom(&app, &mut custom);
            let providers: Vec<Arc<dyn Provider>> = builtin
                .iter()
                .cloned()
                .chain(custom.iter().map(|(_, p)| p.clone()))
                .collect();
            round(&app, &providers, manual);
            manual = false;
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = wake.notified() => { manual = true; }
            }
        }
    });
}

/// Brings the user-defined providers in line with the current settings.
fn sync_custom(app: &AppHandle, custom: &mut Vec<(CustomDef, Arc<dyn Provider>)>) {
    let defs = app
        .state::<AppState>()
        .lock()
        .settings
        .custom_providers
        .clone();
    custom.retain(|(d, _)| defs.contains(d));
    for d in defs {
        if !custom.iter().any(|(c, _)| *c == d) {
            let p: Arc<dyn Provider> = Arc::from(d.clone().into_provider());
            custom.push((d, p));
        }
    }
}

/// Starts a poll for every provider that is due. Each poll finishes on its own, so a slow or
/// unreachable provider holds up neither the others nor the next tick.
fn round(app: &AppHandle, providers: &[Arc<dyn Provider>], manual: bool) {
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
            if !enabled || st.in_flight {
                continue;
            }
            let scheduled = st.next_due.is_none_or(|t| t <= now);
            let cooled = st.last_attempt.is_none_or(|t| {
                (Utc::now() - t)
                    .to_std()
                    .map_or(true, |d| d >= MANUAL_COOLDOWN)
            });
            if scheduled || (manual && cooled) {
                st.in_flight = true;
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

    for p in due {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let id = p.id().to_string();
            let min = p.min_interval();
            let outcome = poll(p).await;
            finish(&app, &id, min, outcome);
        });
    }
}

async fn poll(p: Arc<dyn Provider>) -> Outcome {
    let mut work = tauri::async_runtime::spawn(async move {
        // Login files (and Cursor's SQLite database) are read with blocking calls.
        let probe = p.clone();
        let acc = tauri::async_runtime::spawn_blocking(move || probe.detect())
            .await
            .ok()
            .flatten();
        match acc {
            None => Outcome::NotFound,
            Some(acc) => Outcome::Fetched(p.fetch(&acc).await),
        }
    });
    match tokio::time::timeout(POLL_DEADLINE, &mut work).await {
        Ok(Ok(outcome)) => outcome,
        // The poll panicked; reported like a broken answer so the provider is retried later.
        Ok(Err(_)) => Outcome::Fetched(Err(ProviderError::SchemaChanged("poll failed".into()))),
        Err(_) => {
            work.abort();
            Outcome::Fetched(Err(ProviderError::Offline("timed out".into())))
        }
    }
}

/// Records one poll's outcome, then saves, notifies and redraws.
fn finish(app: &AppHandle, id: &str, min: Duration, outcome: Outcome) {
    let state = app.state::<AppState>();
    {
        let mut g = state.lock();
        let params = g.settings.params(id, min);
        let Some(st) = g.providers.iter_mut().find(|s| s.id == id) else {
            return;
        };
        st.in_flight = false;
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
        // A key or tuning change that came in while this poll ran asks for a fresh one.
        if std::mem::take(&mut st.requeue) {
            st.next_due = None;
        }
    }

    state.save_tuning();
    state.save_snapshots();
    let toasts = notify::evaluate(&mut state.lock());
    notify::show(app, toasts);
    publish(app);
}

/// Pushes the current state to the tray and to every open window.
pub fn publish(app: &AppHandle) {
    tray::refresh(app);
    let _ = app.emit("usage-updated", app.state::<AppState>().dto());
}
