#![allow(dead_code)]

mod model;
mod providers;
mod scanner;

fn main() {
    env_logger::init();
    let snapshot = providers::collect_bar_snapshot();
    log::info!(
        "AgentBar scanned {} agents, {} active sessions",
        snapshot.agents.len(),
        snapshot.active_sessions.len()
    );
}
