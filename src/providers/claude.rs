use std::fs;
use std::path::PathBuf;

use chrono::Utc;

use crate::model::{
    AccountInfo, ActiveSession, AgentKind, AgentSnapshot, InstallStatus, QuotaMetric, RecentSession,
};
use crate::providers::AgentProvider;
use crate::scanner::{find_app_bundle, find_binary, home_dir, query_version, read_json, timestamp_to_utc};

pub struct ClaudeProvider;

impl AgentProvider for ClaudeProvider {
    fn kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }

    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot {
        let bin = find_binary(
            &["~/.local/bin/claude", "/opt/homebrew/bin/claude", "/usr/local/bin/claude"],
            "claude",
        );
        let app = find_app_bundle("Claude");

        let install_status = if let Some(ref path) = bin {
            let version = query_version(path, &["--version"]);
            InstallStatus::Installed {
                version,
                location: path.display().to_string(),
            }
        } else if let Some(ref app_path) = app {
            InstallStatus::Installed {
                version: None,
                location: app_path.display().to_string(),
            }
        } else {
            InstallStatus::NotInstalled
        };

        let mut accounts = Vec::new();
        let mut quotas = Vec::new();
        let mut recent_sessions = Vec::new();

        if let Some(home) = home_dir() {
            collect_claude_local_state(&home, &mut accounts, &mut quotas, &mut recent_sessions);
        }

        AgentSnapshot {
            kind: AgentKind::ClaudeCode,
            install_status,
            accounts,
            quotas,
            active_sessions: active_sessions.to_vec(),
            recent_sessions,
            note: None,
        }
    }
}

fn collect_claude_local_state(
    home: &std::path::Path,
    accounts: &mut Vec<AccountInfo>,
    quotas: &mut Vec<QuotaMetric>,
    recent_sessions: &mut Vec<RecentSession>,
) {
    let claude_json = home.join(".claude.json");
    if let Some(val) = read_json(&claude_json) {
        if let Some(oauth) = val.get("oauthAccount") {
            let email = oauth
                .get("emailAddress")
                .and_then(|v| v.as_str())
                .unwrap_or("Authenticated");
            let org = oauth
                .get("organizationName")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let billing = oauth
                .get("billingType")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            accounts.push(AccountInfo {
                label: "Claude OAuth".to_string(),
                identity: email.to_string(),
                plan: org.or(billing),
                auth_method: Some("OAuth".to_string()),
                is_active: true,
            });
        } else if val.get("numStartups").is_some() {
            let startups = val
                .get("numStartups")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            accounts.push(AccountInfo {
                label: "Claude Code".to_string(),
                identity: format!("Local CLI ({startups} startups)"),
                plan: None,
                auth_method: Some("Configured".to_string()),
                is_active: true,
            });
        }
    }

    let profiles_dir = home.join(".claude/profiles");
    if let Ok(entries) = fs::read_dir(&profiles_dir) {
        let profile_count = entries.flatten().count();
        if profile_count > 0 {
            quotas.push(QuotaMetric {
                name: "Profiles".to_string(),
                used_percent: None,
                remaining_percent: None,
                summary: format!("{profile_count} saved profiles"),
                resets_at: None,
            });
        }
    }

    let stats_path = home.join(".claude/stats-cache.json");
    if let Some(stats) = read_json(&stats_path) {
        if let Some(today_tokens) = extract_today_tokens(&stats) {
            quotas.push(QuotaMetric {
                name: "Today".to_string(),
                used_percent: None,
                remaining_percent: None,
                summary: format_token_count(today_tokens),
                resets_at: None,
            });
        }
        if let Some(total_sessions) = stats.get("totalSessions").and_then(serde_json::Value::as_u64) {
            quotas.push(QuotaMetric {
                name: "Total Sessions".to_string(),
                used_percent: None,
                remaining_percent: None,
                summary: format!("{total_sessions} sessions"),
                resets_at: None,
            });
        }
    }

    let history_path = home.join(".claude/history.jsonl");
    *recent_sessions = read_claude_history(&history_path, 5);
}

fn extract_today_tokens(stats: &serde_json::Value) -> Option<u64> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let daily = stats.get("dailyModelTokens")?.as_array()?;
    let entry = daily
        .iter()
        .find(|item| item.get("date").and_then(|d| d.as_str()) == Some(today.as_str()))?;
    let by_model = entry.get("tokensByModel")?.as_object()?;
    let sum: u64 = by_model.values().filter_map(serde_json::Value::as_u64).sum();
    Some(sum)
}

fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        #[allow(clippy::cast_precision_loss)]
        let m = tokens as f64 / 1_000_000.0;
        format!("{m:.2}M tokens")
    } else if tokens >= 1_000 {
        #[allow(clippy::cast_precision_loss)]
        let k = tokens as f64 / 1_000.0;
        format!("{k:.1}K tokens")
    } else {
        format!("{tokens} tokens")
    }
}

fn read_claude_history(path: &std::path::Path, limit: usize) -> Vec<RecentSession> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut sessions = Vec::new();
    for line in content.lines().rev() {
        if sessions.len() >= limit {
            break;
        }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let display = val
            .get("display")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if display.is_empty() {
            continue;
        }
        let project = val
            .get("project")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);
        let ts = val
            .get("timestamp")
            .and_then(serde_json::Value::as_i64)
            .and_then(timestamp_to_utc);
        let id = val
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-session")
            .to_string();

        sessions.push(RecentSession {
            id,
            agent: AgentKind::ClaudeCode,
            title: display.chars().take(48).collect(),
            project_dir: project,
            updated_at: ts,
        });
    }

    sessions
}
