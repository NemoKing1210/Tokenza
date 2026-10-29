//! Start with the OS. On Windows this is the per-user `Run` registry key (no admin rights), written
//! directly so a stale command can be detected and repaired; elsewhere the autostart plugin does it.

use tauri::AppHandle;

#[cfg(windows)]
mod imp {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// The product name, which is also the value name the autostart plugin used, so existing entries carry over.
    const VALUE: &str = "Tokenza";

    fn registered() -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(RUN_KEY)
            .and_then(|k| k.get_value::<String, _>(VALUE))
            .ok()
    }

    fn command() -> std::io::Result<String> {
        Ok(format!("\"{}\"", std::env::current_exe()?.display()))
    }

    pub fn is_enabled() -> bool {
        registered().is_some()
    }

    /// True when the registered command points at the running executable. A stale path (the app was
    /// moved, reinstalled, or a dev build registered itself) counts as out of sync.
    pub fn is_current() -> bool {
        registered().is_some() && registered() == command().ok()
    }

    pub fn set(_: &super::AppHandle, enabled: bool) -> Result<(), String> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(RUN_KEY)
            .map_err(|e| e.to_string())?
            .0;
        if enabled {
            key.set_value(VALUE, &command().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())
        } else {
            match key.delete_value(VALUE) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other.map_err(|e| e.to_string()),
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri_plugin_autostart::ManagerExt;

    pub fn set(app: &super::AppHandle, enabled: bool) -> Result<(), String> {
        let manager = app.autolaunch();
        let have = manager.is_enabled().unwrap_or(false);
        match (enabled, have) {
            (true, false) => manager.enable().map_err(|e| e.to_string()),
            (false, true) => manager.disable().map_err(|e| e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Applies the setting. Failures (policy, sandbox) are logged and must not break saving the rest.
pub fn apply(app: &AppHandle, want: bool) {
    if let Err(e) = imp::set(app, want) {
        eprintln!("autostart sync failed: {e}");
    }
}

/// Keeps the OS entry in line with the setting at launch, repairing a stale path and not just a
/// missing entry. Debug builds leave it alone, so `tauri dev` never registers `target\debug\tokenza-app.exe`.
pub fn sync_on_startup(app: &AppHandle, want: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    #[cfg(windows)]
    let in_sync = if want {
        imp::is_current()
    } else {
        !imp::is_enabled()
    };
    #[cfg(not(windows))]
    let in_sync = false;
    if !in_sync {
        apply(app, want);
    }
}
