#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adaptive;
mod autostart;
mod hotkey;
mod i18n;
mod logos;
mod notify;
mod scheduler;
mod secrets;
mod settings;
mod state;
mod tray;
mod tray_icon;

use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use settings::Settings;
use state::{AppState, StateDto};
use tokenza_core::CustomDef;

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
    let old_theme = state.lock().settings.theme.clone();
    let applied = state.apply_settings(new);
    applied
        .save(&state.settings_path)
        .map_err(|e| e.to_string())?;
    autostart::apply(&app, applied.autostart);
    // Re-applying the acrylic flickers, so only a theme change touches it.
    if applied.theme != old_theme {
        apply_backdrops(&app, &applied.theme);
    }
    // A combination another app owns leaves the hotkey unbound; `set_hotkey` is where the UI hears of it.
    let _ = hotkey::sync(&app, applied.hotkey_enabled, &applied.hotkey);
    tray::apply_language(&app);
    // A freshly enabled provider is polled by the scheduler's next tick; the rest only needs a redraw.
    scheduler::publish(&app);
    let _ = app.emit("settings-updated", &applied);
    Ok(applied)
}

/// Changes the global shortcut. Errors are codes the UI translates: `invalid`, `in_use`.
#[tauri::command]
fn set_hotkey(app: AppHandle, state: State<AppState>, accel: String) -> Result<Settings, String> {
    if hotkey::parse(&accel).is_none() {
        return Err("invalid".into());
    }
    let (enabled, old) = {
        let g = state.lock();
        (g.settings.hotkey_enabled, g.settings.hotkey.clone())
    };
    // Registration is the only way to learn that another app owns the combination.
    if hotkey::sync(&app, enabled, &accel).is_err() {
        let _ = hotkey::sync(&app, enabled, &old);
        return Err("in_use".into());
    }
    let mut next = state.lock().settings.clone();
    next.hotkey = accel;
    let applied = state.apply_settings(next);
    applied
        .save(&state.settings_path)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("settings-updated", &applied);
    Ok(applied)
}

/// What the UI shows for a rejected credential; the codes are mapped to translated text there.
fn check_secret(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("empty".into());
    }
    if value.chars().count() > tokenza_core::secrets::MAX_LEN {
        return Err("too_long".into());
    }
    Ok(value)
}

#[tauri::command]
fn set_provider_secret(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    field: String,
    value: String,
) -> Result<(), String> {
    if !state.accepted_fields(&id).contains(&field.as_str()) {
        return Err("unknown_field".into());
    }
    secrets::set(&id, &field, check_secret(&value)?).map_err(|e| format!("store_failed: {e}"))?;
    state.mark_secret(&id, &field, true);
    state.poll_soon(&id);
    scheduler::publish(&app);
    Ok(())
}

#[tauri::command]
fn clear_provider_secret(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    field: String,
) -> Result<(), String> {
    if !state.accepted_fields(&id).contains(&field.as_str()) {
        return Err("unknown_field".into());
    }
    secrets::delete(&id, &field).map_err(|e| format!("store_failed: {e}"))?;
    state.mark_secret(&id, &field, false);
    state.poll_soon(&id);
    scheduler::publish(&app);
    Ok(())
}

/// Creates (empty `def.id`) or updates a user-defined provider. A new one needs its token;
/// for an existing one `token` replaces the stored key only when given.
#[tauri::command]
fn save_custom_provider(
    app: AppHandle,
    state: State<AppState>,
    def: CustomDef,
    token: Option<String>,
) -> Result<(), String> {
    let is_new = def.id.is_empty();
    let token = token.filter(|t| !t.trim().is_empty());
    if is_new && token.is_none() {
        return Err("empty".into());
    }
    if let Some(t) = &token {
        check_secret(t)?;
    }
    let (saved, applied) = state.save_custom(def)?;
    let field = tokenza_core::secrets::CUSTOM_FIELD;
    if let Some(t) = &token {
        if let Err(e) = secrets::set(&saved.id, field, t.trim()) {
            // Without its key a new provider is only a broken card, so undo it.
            if is_new {
                if let Ok(back) = state.remove_custom(&saved.id) {
                    let _ = back.save(&state.settings_path);
                }
            }
            return Err(format!("store_failed: {e}"));
        }
        state.mark_secret(&saved.id, field, true);
    }
    applied
        .save(&state.settings_path)
        .map_err(|e| e.to_string())?;
    state.poll_soon(&saved.id);
    scheduler::publish(&app);
    Ok(())
}

/// One line of a form test: what the entered paths read from the live response.
#[derive(serde::Serialize)]
struct TestWindow {
    label: String,
    used_pct: Option<f32>,
}

/// Tries a custom provider definition against its URL without saving anything, so wrong paths
/// show up while the form is still open. A blank `token` falls back to the stored one when editing.
#[tauri::command]
async fn test_custom_provider(
    mut def: CustomDef,
    token: Option<String>,
) -> Result<Vec<TestWindow>, String> {
    if def.id.is_empty() {
        def.id = format!("{}test", tokenza_core::providers::custom::ID_PREFIX);
    }
    def.normalize();
    def.validate()?;
    let token = token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| tokenza_core::secrets::get(&def.id, tokenza_core::secrets::CUSTOM_FIELD))
        .ok_or("empty")?;
    check_secret(&token)?;
    let windows = tokenza_core::providers::custom::fetch_windows(&def, token)
        .await
        .map_err(|e| e.to_string())?;
    Ok(windows
        .into_iter()
        .map(|w| TestWindow {
            label: w.label,
            used_pct: w.used_pct,
        })
        .collect())
}

#[tauri::command]
fn remove_custom_provider(
    app: AppHandle,
    state: State<AppState>,
    id: String,
) -> Result<(), String> {
    let applied = state.remove_custom(&id)?;
    // The definition is gone either way; a key that cannot be deleted stays harmlessly orphaned.
    let _ = secrets::delete(&id, tokenza_core::secrets::CUSTOM_FIELD);
    applied
        .save(&state.settings_path)
        .map_err(|e| e.to_string())?;
    scheduler::publish(&app);
    Ok(())
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

/// Links the About page may open. Anything else is refused, so the web view cannot be used to
/// launch arbitrary addresses.
const ALLOWED_LINK_PREFIX: &str = "https://github.com/NemoKing1210/";

#[derive(serde::Serialize)]
struct AboutInfo {
    version: &'static str,
    license: &'static str,
}

#[tauri::command]
fn about_info() -> AboutInfo {
    AboutInfo {
        version: env!("CARGO_PKG_VERSION"),
        license: env!("CARGO_PKG_LICENSE"),
    }
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with(ALLOWED_LINK_PREFIX) || url.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err("link not allowed".into());
    }
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Windows accent colour as the two shades WinUI uses: light on dark surfaces, dark on light ones.
#[derive(serde::Serialize)]
struct Accent {
    light: String,
    dark: String,
}

#[tauri::command]
fn get_accent() -> Option<Accent> {
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let palette = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent")
            .and_then(|k| k.get_raw_value("AccentPalette"))
            .ok()?
            .bytes;
        // Eight RGBA entries: light 3..1, accent, dark 1..3.
        let hex = |i: usize| {
            palette
                .get(i * 4..i * 4 + 3)
                .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
        };
        Some(Accent {
            light: hex(1)?,
            dark: hex(4)?,
        })
    }
    #[cfg(not(windows))]
    None
}

/// Whether the app's own theme is dark: "system" follows Windows, the other two override it.
fn is_dark(app: &AppHandle, theme: &str) -> bool {
    match theme {
        "light" => false,
        "dark" => true,
        _ => app
            .get_webview_window("flyout")
            .and_then(|w| w.theme().ok())
            .is_some_and(|t| t == tauri::Theme::Dark),
    }
}

/// Acrylic behind the flyout in the app's own theme, like the taskbar flyouts of Windows 11: there it is
/// the DWM system backdrop, so "Transparency effects" off turns it into a solid fill by itself. With
/// "light" chosen on a dark system (or the reverse) a backdrop that follows the system would put light
/// cards on a dark panel, hence the explicit dark flag. The settings window gets the matching opaque
/// fill so it does not flash white before the page has painted.
fn apply_backdrops(app: &AppHandle, theme: &str) {
    let dark = is_dark(app, theme);
    if let Some(w) = app.get_webview_window("settings") {
        let fill = if dark {
            tauri::window::Color(44, 44, 46, 255)
        } else {
            tauri::window::Color(243, 243, 245, 255)
        };
        let _ = w.set_background_color(Some(fill));
    }
    #[cfg(windows)]
    if let Some(w) = app.get_webview_window("flyout") {
        let _ = window_vibrancy::clear_mica(&w);
        let _ = window_vibrancy::clear_acrylic(&w);
        // Only here for its side effect of setting the window's dark-mode flag, which acrylic does not set.
        let _ = window_vibrancy::apply_mica(&w, Some(dark));
        let tint = if dark {
            (32, 32, 32, 200)
        } else {
            (243, 243, 243, 200)
        };
        // Failure (old Windows, disabled transparency) keeps the tinted CSS background.
        let _ = window_vibrancy::apply_acrylic(&w, Some(tint));
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_flyout(app, None, None);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh,
            set_settings,
            set_hotkey,
            reset_tuning,
            set_provider_secret,
            clear_provider_secret,
            save_custom_provider,
            test_custom_provider,
            remove_custom_provider,
            send_test_notification,
            set_flyout_height,
            open_settings,
            hide_flyout,
            quit_app,
            about_info,
            get_accent,
            open_url
        ])
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            secrets::install();
            app.manage(AppState::new(path));
            let handle = app.handle().clone();
            handle.state::<AppState>().load_secret_cache();
            let (want_autostart, theme) = {
                let state = handle.state::<AppState>();
                let g = state.lock();
                (g.settings.autostart, g.settings.theme.clone())
            };
            autostart::sync_on_startup(&handle, want_autostart);
            apply_backdrops(&handle, &theme);
            tray::setup(&handle)?;
            tray::apply_language(&handle);
            let state = handle.state::<AppState>();
            let (want, accel) = {
                let g = state.lock();
                (g.settings.hotkey_enabled, g.settings.hotkey.clone())
            };
            if let Err(e) = hotkey::sync(&handle, want, &accel) {
                eprintln!("could not register the {accel} hotkey: {e}");
            }
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
            // The marks in the per-provider tray icons follow the light or dark taskbar, and the
            // backdrop and accent colour follow Windows when the theme is "system".
            ("flyout", WindowEvent::ThemeChanged(_)) => {
                let app = window.app_handle();
                let theme = app.state::<AppState>().lock().settings.theme.clone();
                if theme == "system" {
                    apply_backdrops(app, &theme);
                }
                let _ = app.emit("system-theme-changed", ());
                tray::refresh(app);
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
