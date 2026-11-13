use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Supported AI coding agent environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentKind {
    Antigravity,
    ClaudeCode,
    Cursor,
    Codex,
    OpenCode,
    Copilot,
}

impl AgentKind {
    /// Returns all built-in supported agents in display order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Antigravity,
            Self::ClaudeCode,
            Self::Cursor,
            Self::Codex,
            Self::OpenCode,
            Self::Copilot,
        ]
    }

    /// Human-readable display name for menus.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Antigravity => "Antigravity (agy)",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::Codex => "OpenAI Codex",
            Self::OpenCode => "OpenCode",
            Self::Copilot => "GitHub Copilot",
        }
    }

    /// Short identifier used in compact labels and IDs.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Antigravity => "agy",
            Self::ClaudeCode => "claude",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
        }
    }

    /// Primary CLI binary name.
    #[must_use]
    pub const fn cli_binary(self) -> &'static str {
        match self {
            Self::Antigravity => "agy",
            Self::ClaudeCode => "claude",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
        }
    }

    /// Whether this agent can be launched as an interactive CLI in a terminal.
    #[must_use]
    pub const fn supports_cli_launch(self) -> bool {
        matches!(
            self,
            Self::Antigravity | Self::ClaudeCode | Self::Codex | Self::OpenCode | Self::Copilot
        )
    }

    /// Whether this agent has a macOS `.app` GUI bundle.
    #[must_use]
    pub const fn gui_app_name(self) -> Option<&'static str> {
        match self {
            Self::Antigravity => Some("Antigravity IDE"),
            Self::ClaudeCode => Some("Claude"),
            Self::Cursor => Some("Cursor"),
            Self::Codex | Self::OpenCode | Self::Copilot => None,
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Installation status of an agent environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallStatus {
    Installed {
        version: Option<String>,
        location: String,
    },
    NotInstalled,
}

impl InstallStatus {
    #[must_use]
    pub const fn is_installed(&self) -> bool {
        matches!(self, Self::Installed { .. })
    }
}

/// Account or provider credential metadata for an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub label: String,
    pub identity: String,
    pub plan: Option<String>,
    pub auth_method: Option<String>,
    pub is_active: bool,
}

/// Usage quota or rate-limit metric for an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaMetric {
    pub name: String,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub summary: String,
    pub resets_at: Option<DateTime<Utc>>,
}

impl QuotaMetric {
    /// Formats a compact Unicode progress bar for menu items.
    #[must_use]
    pub fn format_bar(&self, width: usize) -> String {
        let pct = self
            .used_percent
            .or_else(|| self.remaining_percent.map(|r| 100.0 - r))
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let filled = ((pct / 100.0) * width as f64).round() as usize;
        let filled = filled.min(width);
        let empty = width.saturating_sub(filled);

        format!(
            "{}{} {:>3.0}%",
            "█".repeat(filled),
            "░".repeat(empty),
            pct
        )
    }

    /// Formats the remaining time until reset relative to `now`.
    #[must_use]
    pub fn format_reset(&self, now: DateTime<Utc>) -> Option<String> {
        let reset = self.resets_at?;
        let secs = (reset - now).num_seconds();
        if secs <= 0 {
            return Some("resetting now".to_string());
        }
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        if hours > 24 {
            let days = hours / 24;
            let rem_hours = hours % 24;
            Some(format!("resets in {days}d {rem_hours}h"))
        } else if hours > 0 {
            Some(format!("resets in {hours}h {mins}m"))
        } else {
            Some(format!("resets in {mins}m"))
        }
    }
}

/// A currently running agent process on macOS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveSession {
    pub pid: u32,
    pub agent: AgentKind,
    pub title: String,
    pub working_dir: Option<PathBuf>,
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub started_at: String,
    pub command: String,
}

impl ActiveSession {
    /// Short project directory name for display in menu items.
    #[must_use]
    pub fn project_label(&self) -> String {
        self.working_dir
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map_or_else(|| "~".to_string(), ToString::to_string)
    }
}

/// A recent historical session recorded by an agent locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentSession {
    pub id: String,
    pub agent: AgentKind,
    pub title: String,
    pub project_dir: Option<PathBuf>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Aggregated state for a single agent environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub kind: AgentKind,
    pub install_status: InstallStatus,
    pub accounts: Vec<AccountInfo>,
    pub quotas: Vec<QuotaMetric>,
    pub active_sessions: Vec<ActiveSession>,
    pub recent_sessions: Vec<RecentSession>,
    pub note: Option<String>,
}

impl AgentSnapshot {
    /// Returns true if the agent is installed and has at least one configured account or active session.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.install_status.is_installed()
            && (!self.accounts.is_empty() || !self.active_sessions.is_empty())
    }

    /// Status dot character for native menu labels.
    #[must_use]
    pub fn status_dot(&self) -> &'static str {
        if !self.active_sessions.is_empty() {
            "🟢"
        } else if self.is_ready() {
            "🔵"
        } else if self.install_status.is_installed() {
            "🟡"
        } else {
            "⚪"
        }
    }
}

/// Complete snapshot of all monitored agents at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarSnapshot {
    pub agents: Vec<AgentSnapshot>,
    pub active_sessions: Vec<ActiveSession>,
    pub refreshed_at: DateTime<Utc>,
}

impl BarSnapshot {
    /// Count of installed agents.
    #[must_use]
    pub fn installed_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|a| a.install_status.is_installed())
            .count()
    }

    /// Count of agents with active accounts.
    #[must_use]
    pub fn ready_count(&self) -> usize {
        self.agents.iter().filter(|a| a.is_ready()).count()
    }
}

/// Formats a `DateTime<Utc>` as a compact relative time string (e.g. `12m ago`).
#[must_use]
pub fn format_relative_time(dt: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - dt).num_seconds().max(0);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn quota_bar_formats_clamped_percentages() {
        let metric = QuotaMetric {
            name: "5h".to_string(),
            used_percent: Some(50.0),
            remaining_percent: Some(50.0),
            summary: "50 / 100".to_string(),
            resets_at: None,
        };
        assert_eq!(metric.format_bar(10), "█████░░░░░  50%");

        let over = QuotaMetric {
            used_percent: Some(150.0),
            ..metric
        };
        assert_eq!(over.format_bar(8), "████████ 100%");
    }

    #[test]
    fn quota_reset_formats_relative_duration() {
        let now = Utc::now();
        let metric = QuotaMetric {
            name: "Window".to_string(),
            used_percent: Some(25.0),
            remaining_percent: Some(75.0),
            summary: String::new(),
            resets_at: Some(now + Duration::minutes(135)),
        };
        assert_eq!(metric.format_reset(now).as_deref(), Some("resets in 2h 15m"));
    }

    #[test]
    fn relative_time_formats_buckets() {
        let now = Utc::now();
        assert_eq!(format_relative_time(now - Duration::seconds(20), now), "just now");
        assert_eq!(format_relative_time(now - Duration::minutes(15), now), "15m ago");
        assert_eq!(format_relative_time(now - Duration::hours(3), now), "3h ago");
        assert_eq!(format_relative_time(now - Duration::days(2), now), "2d ago");
    }
}
