//! Prototype CLI: prints limits of every detected agent as JSON.
//! Usage: `tokenza [claude|codex|cursor|opencode|commandcode|devin|antigravity]`

use serde_json::{json, Value};
use tokenza_core::all_providers;

#[tokio::main]
async fn main() {
    let only = std::env::args().nth(1);
    let handles: Vec<_> = all_providers()
        .into_iter()
        .filter(|p| only.as_deref().is_none_or(|id| id == p.id()))
        .map(|p| {
            tokio::spawn(async move {
                let id = p.id();
                let Some(acc) = p.detect() else {
                    return json!({ "provider": id, "status": "not_found" });
                };
                match p.fetch(&acc).await {
                    Ok(snap) => json!({ "provider": id, "status": "ok", "snapshot": snap }),
                    Err(e) => json!({ "provider": id, "status": "error", "error": e }),
                }
            })
        })
        .collect();

    let mut results: Vec<Value> = Vec::new();
    for h in handles {
        results.push(h.await.unwrap_or_else(|_| json!({ "status": "error" })));
    }
    println!("{}", serde_json::to_string_pretty(&results).unwrap());
}
