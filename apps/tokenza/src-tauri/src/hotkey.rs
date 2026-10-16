//! Global hotkey that opens the flyout in the middle of the active screen.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::tray;

pub const DEFAULT: &str = "Ctrl+Alt+T";

/// Parses an accelerator such as `Ctrl+Alt+T`. A combination without Ctrl, Alt or Win is refused:
/// as a global hotkey it would swallow ordinary typing.
pub fn parse(accel: &str) -> Option<Shortcut> {
    let shortcut: Shortcut = accel.trim().parse().ok()?;
    shortcut
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
        .then_some(shortcut)
}

/// The accelerator to keep in settings: the given one if usable, the default otherwise.
pub fn normalize(accel: &str) -> String {
    match parse(accel) {
        Some(_) => accel.trim().to_string(),
        None => DEFAULT.to_string(),
    }
}

/// Registers or releases the shortcut to match the settings; safe to call on every settings save.
/// Fails when the combination is already owned by another app.
pub fn sync(app: &AppHandle, want: bool, accel: &str) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    if !want {
        let _ = shortcuts.unregister_all();
        return Ok(());
    }
    let shortcut = parse(accel)
        .or_else(|| parse(DEFAULT))
        .ok_or("invalid shortcut")?;
    if shortcuts.is_registered(shortcut) {
        return Ok(());
    }
    // This is the only shortcut Tokenza owns, so replacing the old one means dropping everything.
    let _ = shortcuts.unregister_all();
    shortcuts
        .on_shortcut(shortcut, |app, _, event| {
            if event.state() == ShortcutState::Pressed {
                tray::toggle_flyout_centered(app);
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_usable() {
        assert!(parse(DEFAULT).is_some());
    }

    #[test]
    fn accepts_combinations_with_a_real_modifier() {
        for ok in [
            "Ctrl+Alt+T",
            "Alt+F2",
            "Ctrl+Shift+Space",
            "Super+U",
            " ctrl+alt+t ",
        ] {
            assert!(parse(ok).is_some(), "{ok}");
        }
    }

    #[test]
    fn refuses_keys_that_would_swallow_typing() {
        for bad in ["T", "Shift+T", "F5", "", "Ctrl+Alt", "Ctrl+Alt+Nope"] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn normalize_falls_back_to_the_default() {
        assert_eq!(normalize("Alt+F2"), "Alt+F2");
        assert_eq!(normalize("garbage"), DEFAULT);
    }
}
