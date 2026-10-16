use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A detected login of an agent. Never holds a token.
#[derive(Debug, Clone)]
pub struct Account {
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSnapshot {
    pub provider: String,
    /// Email or plan, never a token.
    pub account: String,
    pub windows: Vec<UsageWindow>,
    /// When the paid period runs out or renews, for providers that report it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_ends_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowKind {
    Session5h,
    Daily,
    Weekly,
    Monthly,
    Credits,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageWindow {
    pub kind: WindowKind,
    /// Human label, e.g. "Week · Opus".
    pub label: String,
    /// `None` for windows that have no percentage (credit balance).
    pub used_pct: Option<f32>,
    pub resets_at: Option<DateTime<Utc>>,
    /// Extra text such as a credit balance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
