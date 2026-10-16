#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adaptive;
mod i18n;
mod notify;
mod scheduler;
mod settings;
mod state;
mod tray;
mod tray_icon;

use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use settings::Settings;
use state::{AppState, StateDto};

#[tauri::command]
fn get_state(state: State<AppState>) -> StateDto {
    state.dto()
}

#[tauri::command]
fn refresh(state: State<AppState>) {
    state.wake.notify_one();
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<AppState>, new: Settings) -> Result<Settings, String> {
    let applied = state.apply_settings(new);
    applied
        .save(&state.settings_path)
        .map_err(|e| e.to_string())?;
    sync_autostart(&app, applied.autostart);
    tray::apply_language(&app);
    // A freshly enabled provider is polled by the scheduler's next tick; the rest only needs a redraw.
    scheduler::publish(&app);
    let _ = app.emit("settings-updated", &applied);
    Ok(applied)
}

#[tauri::command]
fn send_test_notification(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let (lang, warn) = {
        let g = state.lock();
        (g.settings.resolved_lang(), g.settings.warn_pct)
    };
    notify::show_test(&app, lang, warn)
}

#[tauri::command]
fn reset_tuning(app: AppHandle, state: State<AppState>, id: String) {
    state.reset_tuning(&id);
    state.save_tuning();
    scheduler::publish(&app);
}

#[tauri::command]
fn set_flyout_height(app: AppHandle, height: f64) {
    tray::resize_flyout(&app, height);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    tray::open_settings(&app);
}

#[tauri::command]
fn hide_flyout(app: AppHandle) {
    tray::hide_flyout(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn sync_autostart(app: &AppHandle, want: bool) {
    let manager = app.autolaunch();
    let have = manager.is_enabled().unwrap_or(false);
    // Autostart failures (policy, sandbox) must not break saving the rest of the settings.
    if want && !have {
        let _ = manager.enable();
    } else if !want && have {
        let _ = manager.disable();
    }
}

fn apply_backdrops(app: &AppHandle) {
    #[cfg(windows)]
    {
        use window_vibrancy::apply_acrylic;
        if let Some(w) = app.get_webview_window("flyout") {
            // Failure (old Windows, disabled transparency) keeps the tinted CSS background.
            let _ = apply_acrylic(&w, Some((243, 243, 245, 140)));
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_flyout(app, None);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh,
            set_settings,
            reset_tuning,
            send_test_notification,
            set_flyout_height,
            open_settings,
            hide_flyout,
            quit_app
        ])
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            app.manage(AppState::new(path));
            let handle = app.handle().clone();
            sync_autostart(
                &handle,
                handle.state::<AppState>().lock().settings.autostart,
            );
            apply_backdrops(&handle);
            tray::setup(&handle)?;
            tray::apply_language(&handle);
            scheduler::spawn(handle);
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            ("flyout", WindowEvent::Focused(false)) => {
                if window.is_visible().unwrap_or(false) {
                    tray::hide_flyout(window.app_handle());
                    tray::note_hidden(window.app_handle());
                }
            }
            // Closing settings only hides it; the app lives in the tray.
            ("settings", WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running Tokenza");
}
