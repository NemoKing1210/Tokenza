//! Keys the user types into the provider settings, kept in the OS credential store
//! (Windows Credential Manager), never in `settings.json`.

use std::sync::Arc;

use tokenza_core::secrets::SecretSource;

const SERVICE: &str = "Tokenza";

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
mod backend {
    use super::SERVICE;

    fn entry(provider: &str, field: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, &format!("{provider}/{field}")).map_err(|e| e.to_string())
    }

    pub fn get(provider: &str, field: &str) -> Option<String> {
        entry(provider, field).ok()?.get_password().ok()
    }

    pub fn set(provider: &str, field: &str, value: &str) -> Result<(), String> {
        entry(provider, field)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    /// Deleting something that is not there counts as done.
    pub fn delete(provider: &str, field: &str) -> Result<(), String> {
        match entry(provider, field)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod backend {
    pub fn get(_: &str, _: &str) -> Option<String> {
        None
    }
    pub fn set(_: &str, _: &str, _: &str) -> Result<(), String> {
        Err("no credential store on this platform".into())
    }
    pub fn delete(_: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
}

pub use backend::{delete, set};

pub fn exists(provider: &str, field: &str) -> bool {
    backend::get(provider, field).is_some()
}

struct Store;

impl SecretSource for Store {
    fn get(&self, provider: &str, field: &str) -> Option<String> {
        backend::get(provider, field)
    }
}

/// Lets the providers in `tokenza-core` read what the user typed.
pub fn install() {
    tokenza_core::secrets::set_source(Arc::new(Store));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real credential store, so it only runs on request:
    /// `cargo test -p tokenza-app -- --ignored`.
    #[test]
    #[ignore]
    fn round_trips_a_maximum_length_value() {
        let value = "x".repeat(tokenza_core::secrets::MAX_LEN);
        set("test-provider", "token", &value).unwrap();
        assert_eq!(backend::get("test-provider", "token"), Some(value));
        delete("test-provider", "token").unwrap();
        assert!(!exists("test-provider", "token"));
        delete("test-provider", "token").expect("deleting a missing entry is not an error");
    }
}
