//! Tray icon, its menu, and the flyout window that opens from it.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{
    image::Image, AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Theme, WebviewWindow,
};
use tokenza_core::UsageWindow;

use crate::i18n;
use crate::logos;
use crate::settings::Settings;
use crate::state::{AppState, FlyoutAnchor, ProviderState};
use crate::tray_icon::{self, Levels};

const TRAY_ID: &str = "main";
/// Providers that currently have an icon of their own, so one that was switched off can be removed.
static PROVIDER_ICONS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Pixels and tooltip each tray icon shows now, so an unchanged one is not handed to Windows again.
static SHOWN: Mutex<Vec<(String, Vec<u8>, String)>> = Mutex::new(Vec::new());
/// Language the tray menus were last built in; rebuilding them is only needed when it changes.
static MENU_LANG: Mutex<Option<&'static str>> = Mutex::new(None);
pub const FLYOUT_WIDTH: f64 = 360.0;
/// A tray click right after the flyout lost focus is the click that closed it, not a new open.
const REOPEN_GUARD: Duration = Duration::from_millis(350);
const GAP: i32 = 8;

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let lang = app.state::<AppState>().lock().settings.resolved_lang();
    let tr = i18n::strings(lang);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", tr.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "refresh", tr.refresh, true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", tr.settings, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", tr.quit, true, None::<&str>)?,
        ],
    )
}

const PROVIDER_ICON_PREFIX: &str = "provider-";

fn provider_icon_id(provider: &str) -> String {
    format!("{PROVIDER_ICON_PREFIX}{provider}")
}

/// Re-applies the language to everything the backend owns: tray menus and the settings window title.
pub fn apply_language(app: &AppHandle) {
    let lang = app.state::<AppState>().lock().settings.resolved_lang();
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.set_title(i18n::strings(lang).settings_title);
    }
    // Called on every settings save; the menus only need rebuilding when the language changed.
    if MENU_LANG
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .replace(lang)
        == Some(lang)
    {
        return;
    }
    let mut ids = vec![TRAY_ID.to_string()];
    ids.extend(
        PROVIDER_ICONS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|p| provider_icon_id(p)),
    );
    for id in ids {
        if let (Some(tray), Ok(menu)) = (app.tray_by_id(&id), build_menu(app)) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn on_menu_event(app: &AppHandle, ev: MenuEvent) {
    match ev.id().as_ref() {
        "open" => show_flyout(app, None, None),
        "refresh" => app.state::<AppState>().wake.notify_one(),
        "settings" => open_settings(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// A left click opens the flyout next to whichever icon was clicked.
fn on_tray_event(tray: &TrayIcon, ev: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        rect,
        ..
    } = ev
    {
        let app = tray.app_handle();
        let scale = app
            .get_webview_window("flyout")
            .and_then(|w| w.scale_factor().ok())
            .unwrap_or(1.0);
        let pos = rect.position.to_physical::<i32>(scale);
        let size = rect.size.to_physical::<u32>(scale);
        // A provider's own icon opens the flyout with that provider's card highlighted.
        let focus = tray.id().as_ref().strip_prefix(PROVIDER_ICON_PREFIX);
        toggle_flyout(app, (pos.x, pos.y, size.width, size.height), focus);
    }
}

fn build_icon(
    app: &AppHandle,
    id: &str,
    icon: Image<'static>,
    tooltip: &str,
) -> tauri::Result<TrayIcon> {
    TrayIconBuilder::with_id(id)
        .icon(icon)
        .tooltip(tooltip)
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_event)
        .build(app)
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let icon = Image::new_owned(render(app), tray_icon::SIZE, tray_icon::SIZE);
    build_icon(app, TRAY_ID, icon, "Tokenza")?;
    sync_provider_icons(app);
    Ok(())
}

/// The limit windows the icon may draw for one provider: not hidden, and of a followed kind.
fn tray_windows<'a>(
    settings: &'a Settings,
    p: &'a ProviderState,
) -> impl Iterator<Item = &'a UsageWindow> {
    p.snapshot
        .iter()
        .flat_map(|s| s.windows.iter())
        .filter(|w| {
            settings.window_visible(&p.id, &w.label) && settings.counts_kind_in_tray(w.kind)
        })
}

/// What the main icon draws: the chosen providers' limit windows.
fn tray_levels(settings: &Settings, providers: &[ProviderState]) -> Levels {
    Levels::from_windows(
        providers
            .iter()
            .filter(|p| p.enabled && settings.counts_in_tray(&p.id))
            .flat_map(|p| tray_windows(settings, p)),
    )
}

fn levels(app: &AppHandle) -> Levels {
    let state = app.state::<AppState>();
    let g = state.lock();
    tray_levels(&g.settings, &g.providers)
}

fn render(app: &AppHandle) -> Vec<u8> {
    let style = app.state::<AppState>().lock().settings.tray_style.clone();
    tray_icon::render(&style, levels(app))
}

/// Windows caps tooltips at 127 characters, so only short per-provider numbers fit.
fn tooltip(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let g = state.lock();
    let parts: Vec<String> = g
        .providers
        .iter()
        .filter(|p| p.enabled)
        .filter_map(|p| {
            let snap = p.snapshot.as_ref()?;
            // Aliases can be long; the tooltip has to stay under Windows' length limit.
            let name: String = g.settings.display_name(&p.id).chars().take(14).collect();
            let pcts: Vec<String> = snap
                .windows
                .iter()
                .filter(|w| g.settings.window_visible(&p.id, &w.label))
                .filter_map(|w| w.used_pct)
                .map(|v| format!("{}%", v.round()))
                .collect();
            if pcts.is_empty() {
                return None;
            }
            Some(format!("{name} {}", pcts.join("/")))
        })
        .collect();
    let text = if parts.is_empty() {
        "Tokenza".to_string()
    } else {
        format!("Tokenza\n{}", parts.join(" · "))
    };
    text.chars().take(120).collect()
}

/// Redraws every tray icon from the current state.
///
/// A tray call made off the main thread blocks until the main thread has run it, and sync commands
/// run on the main thread and take the same locks. Doing all of it on the main thread means the
/// poller never waits on the main thread while holding a lock the main thread needs.
pub fn refresh(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = handle.tray_by_id(TRAY_ID) {
            update_icon(&tray, TRAY_ID, render(&handle), tooltip(&handle));
        }
        sync_provider_icons(&handle);
    });
}

/// Hands the icon and tooltip to the tray only when they differ from what it already shows.
fn update_icon(tray: &TrayIcon, id: &str, rgba: Vec<u8>, tip: String) {
    let mut shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner);
    let i = match shown.iter().position(|(i, ..)| i == id) {
        Some(i) => i,
        None => {
            shown.push((id.to_string(), Vec::new(), String::new()));
            shown.len() - 1
        }
    };
    let slot = &mut shown[i];
    if slot.1 != rgba {
        let image = Image::new_owned(rgba.clone(), tray_icon::SIZE, tray_icon::SIZE);
        if tray.set_icon(Some(image)).is_ok() {
            slot.1 = rgba;
        }
    }
    if slot.2 != tip && tray.set_tooltip(Some(&tip)).is_ok() {
        slot.2 = tip;
    }
}

/// One provider's tooltip: its name and every visible limit, within Windows' length cap.
fn provider_tooltip(settings: &Settings, p: &ProviderState) -> String {
    let name = settings.display_name(&p.id);
    let limits: Vec<String> = p
        .snapshot
        .iter()
        .flat_map(|s| s.windows.iter())
        .filter(|w| settings.window_visible(&p.id, &w.label))
        .filter_map(|w| Some(format!("{} {}%", w.label, w.used_pct?.round())))
        .collect();
    let text = if limits.is_empty() {
        name
    } else {
        format!("{name}\n{}", limits.join(" · "))
    };
    text.chars().take(120).collect()
}

/// Gives every provider that asked for one an icon of its own next to the main one, and removes
/// the icons of providers that no longer want it.
fn sync_provider_icons(app: &AppHandle) {
    // The flyout follows the system theme, which is a good guess for the taskbar's.
    let dark = app
        .get_webview_window("flyout")
        .and_then(|w| w.theme().ok())
        .is_none_or(|t| t != Theme::Light);
    let wanted: Vec<(String, Vec<u8>, String)> = {
        let state = app.state::<AppState>();
        let g = state.lock();
        let s = &g.settings;
        g.providers
            .iter()
            .filter(|p| p.enabled && s.pref(&p.id).is_some_and(|pref| pref.own_tray_icon))
            .map(|p| {
                let levels = Levels::from_windows(tray_windows(s, p));
                // A provider the user defined has no mark, so it gets the plain gauge.
                let rgba = match logos::logo(&p.id) {
                    Some(logo) => tray_icon::render_provider(&s.tray_style, levels, logo, dark),
                    None => tray_icon::render(&s.tray_style, levels),
                };
                (p.id.clone(), rgba, provider_tooltip(s, p))
            })
            .collect()
    };

    let mut live = PROVIDER_ICONS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let mut shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner);
    live.retain(|id| {
        let keep = wanted.iter().any(|(w, ..)| w == id);
        if !keep {
            let tray_id = provider_icon_id(id);
            app.remove_tray_by_id(&tray_id);
            shown.retain(|(i, ..)| *i != tray_id);
        }
        keep
    });
    drop(shown);
    for (id, rgba, tip) in wanted {
        let tray_id = provider_icon_id(&id);
        if let Some(tray) = app.tray_by_id(&tray_id) {
            update_icon(&tray, &tray_id, rgba, tip);
            continue;
        }
        let image = Image::new_owned(rgba.clone(), tray_icon::SIZE, tray_icon::SIZE);
        if build_icon(app, &tray_id, image, &tip).is_ok() {
            SHOWN
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((tray_id, rgba, tip));
            live.push(id);
        }
    }
}

fn toggle_flyout(app: &AppHandle, anchor: (i32, i32, u32, u32), focus: Option<&str>) {
    let Some(w) = app.get_webview_window("flyout") else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        hide_flyout(app);
        return;
    }
    let recently_hidden = app
        .state::<AppState>()
        .lock()
        .last_hidden
        .is_some_and(|t| t.elapsed() < REOPEN_GUARD);
    if !recently_hidden {
        let (x, y, w, h) = anchor;
        show_flyout(app, Some(FlyoutAnchor::Tray(x, y, w, h)), focus);
    }
}

/// Global hotkey: opens the flyout in the middle of the screen the mouse is on, or closes it.
pub fn toggle_flyout_centered(app: &AppHandle) {
    let Some(w) = app.get_webview_window("flyout") else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        hide_flyout(app);
        return;
    }
    // The cursor's monitor is the "active" screen; fall back to the primary one.
    let point = app
        .cursor_position()
        .ok()
        .map(|p| (p.x as i32, p.y as i32))
        .or_else(|| {
            let m = w.primary_monitor().ok().flatten()?;
            Some((m.position().x, m.position().y))
        });
    if let Some((x, y)) = point {
        show_flyout(app, Some(FlyoutAnchor::Centered(x, y)), None);
    }
}

/// Opens the flyout; `focus` names a provider whose card the UI should highlight.
pub fn show_flyout(app: &AppHandle, anchor: Option<FlyoutAnchor>, focus: Option<&str>) {
    let Some(w) = app.get_webview_window("flyout") else {
        return;
    };
    if anchor.is_some() {
        app.state::<AppState>().lock().flyout_anchor = anchor;
    }
    place(&w);
    // Tell the UI first so it can start its entrance animation as the window appears.
    let _ = app.emit("flyout-shown", focus);
    let _ = w.show();
    let _ = w.set_focus();
}

/// The monitor the flyout belongs on: the one of its anchor, else the primary one.
fn flyout_monitor(w: &WebviewWindow) -> Option<tauri::Monitor> {
    let app = w.app_handle();
    let anchor = app.state::<AppState>().lock().flyout_anchor;
    anchor
        .and_then(|a| {
            let (x, y) = match a {
                FlyoutAnchor::Tray(x, y, _, _) | FlyoutAnchor::Centered(x, y) => (x, y),
            };
            app.monitor_from_point(x as f64, y as f64).ok().flatten()
        })
        .or_else(|| w.primary_monitor().ok().flatten())
}

/// Positions the flyout next to the tray icon, kept inside the monitor's work area.
pub fn place(w: &WebviewWindow) {
    let app = w.app_handle();
    let anchor = app.state::<AppState>().lock().flyout_anchor;
    let (Ok(size), Ok(scale)) = (w.outer_size(), w.scale_factor()) else {
        return;
    };
    let (ww, wh) = (size.width as i32, size.height as i32);

    let Some(m) = flyout_monitor(w) else { return };
    let wa = m.work_area();
    let (left, top) = (wa.position.x, wa.position.y);
    let (right, bottom) = (left + wa.size.width as i32, top + wa.size.height as i32);

    let margin = (GAP as f64 * scale) as i32;
    let (mut x, mut y) = match anchor {
        Some(FlyoutAnchor::Centered(..)) => ((left + right - ww) / 2, (top + bottom - wh) / 2),
        Some(FlyoutAnchor::Tray(ax, ay, aw, ah)) => {
            let cx = ax + aw as i32 / 2;
            let ny = if ay > (top + bottom) / 2 {
                ay - wh - margin
            } else {
                ay + ah as i32 + margin
            };
            (cx - ww / 2, ny)
        }
        None => (right - ww - margin, bottom - wh - margin),
    };
    x = x.clamp(left + margin, (right - ww - margin).max(left + margin));
    y = y.clamp(top + margin, (bottom - wh - margin).max(top + margin));
    #[cfg(debug_assertions)]
    eprintln!("place: anchor={anchor:?} win={ww}x{wh} scale={scale} work=({left},{top})-({right},{bottom}) -> ({x},{y})");
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// Called by the UI once its content height is known.
pub fn resize_flyout(app: &AppHandle, height: f64) {
    let Some(w) = app.get_webview_window("flyout") else {
        return;
    };
    // Taller than the screen's work area is pointless: the page scrolls inside the window instead.
    let room = flyout_monitor(&w)
        .map(|m| m.work_area().size.height as f64 / m.scale_factor() - 2.0 * GAP as f64)
        .unwrap_or(900.0)
        .max(120.0);
    let h = height.clamp(120.0, room);
    let _ = w.set_size(LogicalSize::new(FLYOUT_WIDTH, h));
    if w.is_visible().unwrap_or(false) {
        place(&w);
    }
}

pub fn hide_flyout(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("flyout") {
        // Lets the UI reset to its "before entrance" look so the next show does not flash old content.
        let _ = app.emit("flyout-hidden", ());
        let _ = w.hide();
    }
}

pub fn note_hidden(app: &AppHandle) {
    app.state::<AppState>().lock().last_hidden = Some(Instant::now());
}

pub fn open_settings(app: &AppHandle) {
    hide_flyout(app);
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use chrono::Utc;
    use tokenza_core::{UsageSnapshot, UsageWindow, WindowKind};

    use super::*;

    fn provider(id: &str, windows: &[(WindowKind, &str, f32)]) -> ProviderState {
        let mut st = ProviderState::new(id, true);
        st.snapshot = Some(UsageSnapshot {
            provider: id.into(),
            account: String::new(),
            windows: windows
                .iter()
                .map(|(kind, label, pct)| UsageWindow {
                    kind: *kind,
                    label: (*label).into(),
                    used_pct: Some(*pct),
                    resets_at: None,
                    detail: None,
                })
                .collect(),
            subscription_ends_at: None,
            fetched_at: Utc::now(),
        });
        st
    }

    fn sample() -> Vec<ProviderState> {
        vec![
            provider(
                "claude",
                &[
                    (WindowKind::Session5h, "5 h", 30.0),
                    (WindowKind::Weekly, "Week", 50.0),
                ],
            ),
            provider("codex", &[(WindowKind::Weekly, "Week", 80.0)]),
        ]
    }

    #[test]
    fn all_providers_report_the_worst_limit() {
        let l = tray_levels(&Settings::default(), &sample());
        assert_eq!(l.worst, Some(80.0));
        assert_eq!(l.session, Some(30.0));
        assert_eq!(l.weekly, Some(80.0));
    }

    #[test]
    fn a_single_provider_ignores_the_others() {
        let mut s = Settings::default();
        s.tray_source = "single".into();
        s.tray_provider = "claude".into();
        let l = tray_levels(&s, &sample());
        assert_eq!(
            (l.worst, l.session, l.weekly),
            (Some(50.0), Some(30.0), Some(50.0))
        );
    }

    #[test]
    fn selected_providers_follow_their_flag() {
        let mut s = Settings::default();
        s.tray_source = "selected".into();
        s.providers
            .iter_mut()
            .find(|p| p.id == "codex")
            .unwrap()
            .show_in_tray = false;
        assert_eq!(tray_levels(&s, &sample()).worst, Some(50.0));
    }

    #[test]
    fn only_the_chosen_kinds_count() {
        let mut s = Settings::default();
        s.tray_kinds = vec!["session".into()];
        let l = tray_levels(&s, &sample());
        assert_eq!(
            (l.worst, l.session, l.weekly),
            (Some(30.0), Some(30.0), None)
        );
    }

    #[test]
    fn a_provider_icon_draws_only_that_provider() {
        let providers = sample();
        let s = Settings::default();
        let claude = Levels::from_windows(tray_windows(&s, &providers[0]));
        assert_eq!(
            (claude.worst, claude.session, claude.weekly),
            (Some(50.0), Some(30.0), Some(50.0))
        );
        let codex = Levels::from_windows(tray_windows(&s, &providers[1]));
        assert_eq!(codex.worst, Some(80.0));
    }

    #[test]
    fn provider_tooltip_names_the_provider_and_lists_visible_limits() {
        let mut s = Settings::default();
        let providers = sample();
        assert_eq!(
            provider_tooltip(&s, &providers[0]),
            "Claude Code\n5 h 30% · Week 50%"
        );
        s.providers
            .iter_mut()
            .find(|p| p.id == "claude")
            .unwrap()
            .hidden_windows = vec!["Week".into()];
        assert_eq!(provider_tooltip(&s, &providers[0]), "Claude Code\n5 h 30%");
        assert_eq!(
            provider_tooltip(&s, &ProviderState::new("cursor", true)),
            "Cursor",
            "no data yet: just the name"
        );
    }

    #[test]
    fn hidden_windows_and_disabled_providers_stay_out() {
        let mut s = Settings::default();
        s.providers
            .iter_mut()
            .find(|p| p.id == "codex")
            .unwrap()
            .hidden_windows = vec!["Week".into()];
        assert_eq!(tray_levels(&s, &sample()).worst, Some(50.0));

        let mut providers = sample();
        providers[0].enabled = false;
        assert_eq!(
            tray_levels(&Settings::default(), &providers).worst,
            Some(80.0)
        );
        providers[1].enabled = false;
        assert_eq!(tray_levels(&Settings::default(), &providers).worst, None);
    }
}
