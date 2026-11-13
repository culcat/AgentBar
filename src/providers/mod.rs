pub mod agy;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod opencode;

use chrono::Utc;

use crate::model::{ActiveSession, AgentKind, AgentSnapshot, BarSnapshot};
use crate::scanner::process::discover_active_sessions;

/// Trait implemented by each AI coding agent environment provider.
pub trait AgentProvider {
    fn kind(&self) -> AgentKind;
    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot;
}

/// Collects a full `BarSnapshot` across all supported agents.
#[must_use]
pub fn collect_bar_snapshot() -> BarSnapshot {
    let active_sessions = discover_active_sessions();
    let providers: Vec<Box<dyn AgentProvider>> = vec![
        Box::new(agy::AntigravityProvider),
        Box::new(claude::ClaudeProvider),
        Box::new(cursor::CursorProvider),
        Box::new(codex::CodexProvider),
        Box::new(opencode::OpenCodeProvider),
        Box::new(copilot::CopilotProvider),
    ];

    let agents = providers
        .into_iter()
        .map(|provider| {
            let kind = provider.kind();
            let matching: Vec<ActiveSession> = active_sessions
                .iter()
                .filter(|s| s.agent == kind)
                .cloned()
                .collect();
            provider.collect(&matching)
        })
        .collect();

    BarSnapshot {
        agents,
        active_sessions,
        refreshed_at: Utc::now(),
    }
}
