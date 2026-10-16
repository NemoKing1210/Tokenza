//! Tray icon, its menu, and the flyout window that opens from it.

use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    image::Image, AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow,
};

use crate::i18n;
use crate::state::AppState;
use crate::tray_icon::{self, Levels};

const TRAY_ID: &str = "main";
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

/// Re-applies the language to everything the backend owns: tray menu and the settings window title.
pub fn apply_language(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
    let lang = app.state::<AppState>().lock().settings.resolved_lang();
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.set_title(i18n::strings(lang).settings_title);
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(render(app))
        .tooltip("Tokenza")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => show_flyout(app, None),
            "refresh" => app.state::<AppState>().wake.notify_one(),
            "settings" => open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
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
                toggle_flyout(app, (pos.x, pos.y, size.width, size.height));
            }
        })
        .build(app)?;
    Ok(())
}

fn levels(app: &AppHandle) -> Levels {
    let state = app.state::<AppState>();
    let g = state.lock();
    // Only providers that count toward the tray, and only their visible limit windows.
    let settings = &g.settings;
    Levels::from_windows(
        g.providers
            .iter()
            .filter(|p| p.enabled && settings.counts_in_tray(&p.id))
            .filter_map(|p| p.snapshot.as_ref().map(|s| (p.id.as_str(), s)))
            .flat_map(|(id, s)| {
                s.windows
                    .iter()
                    .filter(move |w| settings.window_visible(id, &w.label))
            }),
    )
}

fn render(app: &AppHandle) -> Image<'static> {
    let style = app.state::<AppState>().lock().settings.tray_style.clone();
    Image::new_owned(
        tray_icon::render(&style, levels(app)),
        tray_icon::SIZE,
        tray_icon::SIZE,
    )
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

pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(render(app)));
        let _ = tray.set_tooltip(Some(tooltip(app)));
    }
}

fn toggle_flyout(app: &AppHandle, anchor: (i32, i32, u32, u32)) {
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
        show_flyout(app, Some(anchor));
    }
}

pub fn show_flyout(app: &AppHandle, anchor: Option<(i32, i32, u32, u32)>) {
    let Some(w) = app.get_webview_window("flyout") else {
        return;
    };
    if anchor.is_some() {
        app.state::<AppState>().lock().flyout_anchor = anchor;
    }
    place(&w);
    // Tell the UI first so it can start its entrance animation as the window appears.
    let _ = app.emit("flyout-shown", ());
    let _ = w.show();
    let _ = w.set_focus();
}

/// Positions the flyout next to the tray icon, kept inside the monitor's work area.
pub fn place(w: &WebviewWindow) {
    let app = w.app_handle();
    let anchor = app.state::<AppState>().lock().flyout_anchor;
    let (Ok(size), Ok(scale)) = (w.outer_size(), w.scale_factor()) else {
        return;
    };
    let (ww, wh) = (size.width as i32, size.height as i32);

    let monitor = anchor
        .and_then(|(x, y, _, _)| app.monitor_from_point(x as f64, y as f64).ok().flatten())
        .or_else(|| w.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return };
    let wa = m.work_area();
    let (left, top) = (wa.position.x, wa.position.y);
    let (right, bottom) = (left + wa.size.width as i32, top + wa.size.height as i32);

    let margin = (GAP as f64 * scale) as i32;
    let (mut x, mut y) = match anchor {
        Some((ax, ay, aw, ah)) => {
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
    let h = height.clamp(120.0, 900.0);
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
