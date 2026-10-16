//! Credentials the user typed into Tokenza itself.
//!
//! The core crate knows nothing about where they live: the host application installs a
//! [`SecretSource`] (the desktop app backs it with the OS credential store) and providers
//! ask for a value per request. Nothing is cached here, so a value exists in memory only
//! while a request is being built.

use std::sync::{Arc, RwLock};

pub trait SecretSource: Send + Sync {
    fn get(&self, provider: &str, field: &str) -> Option<String>;
}

static SOURCE: RwLock<Option<Arc<dyn SecretSource>>> = RwLock::new(None);

/// Installs the process-wide source; the last call wins.
pub fn set_source(source: Arc<dyn SecretSource>) {
    *SOURCE.write().unwrap_or_else(|e| e.into_inner()) = Some(source);
}

/// A stored value, trimmed; blank counts as absent.
pub fn get(provider: &str, field: &str) -> Option<String> {
    let source = SOURCE.read().unwrap_or_else(|e| e.into_inner()).clone()?;
    let v = source.get(provider, field)?.trim().to_string();
    (!v.is_empty()).then_some(v)
}

/// Fields the user may fill in for a built-in provider, in display order.
pub fn builtin_fields(provider: &str) -> &'static [&'static str] {
    match provider {
        "opencode" => &["workspace_id", "cookie"],
        "commandcode" => &["api_key"],
        _ => &[],
    }
}

/// Custom providers keep a single secret.
pub const CUSTOM_FIELD: &str = "token";

/// Upper bound for one value; the Windows credential store holds about 1280 characters per entry.
pub const MAX_LEN: usize = 1200;
