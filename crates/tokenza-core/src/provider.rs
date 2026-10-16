use std::time::Duration;

use async_trait::async_trait;

use crate::{providers, Account, ProviderError, UsageSnapshot};

#[async_trait]
pub trait Provider: Send + Sync {
    /// Built-in providers return a fixed id; user-defined ones carry theirs.
    fn id(&self) -> &str;
    /// Lowest polling interval the provider tolerates.
    fn min_interval(&self) -> Duration;
    /// Whether the agent has a login on this machine.
    fn detect(&self) -> Option<Account>;
    async fn fetch(&self, acc: &Account) -> Result<UsageSnapshot, ProviderError>;
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(providers::claude::Claude),
        Box::new(providers::codex::Codex),
        Box::new(providers::cursor::Cursor),
        Box::new(providers::opencode::OpenCode),
        Box::new(providers::commandcode::CommandCode),
        Box::new(providers::devin::Devin),
        Box::new(providers::antigravity::Antigravity),
    ]
}
