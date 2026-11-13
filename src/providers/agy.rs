use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::model::{
    AccountInfo, ActiveSession, AgentKind, AgentSnapshot, InstallStatus, QuotaMetric, RecentSession,
};
use crate::providers::AgentProvider;
use crate::scanner::{find_app_bundle, find_binary, home_dir, query_version, read_json};

pub struct AntigravityProvider;

impl AgentProvider for AntigravityProvider {
    fn kind(&self) -> AgentKind {
        AgentKind::Antigravity
    }

    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot {
        let bin = find_binary(
            &["~/.local/bin/agy", "~/.gemini/bin/agy"],
            "agy",
        );
        let app = find_app_bundle("Antigravity IDE").or_else(|| find_app_bundle("Antigravity"));

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
            let settings_path = home.join(".gemini/settings.json");
            if let Some(settings) = read_json(&settings_path)
                && let Some(auth_type) = settings
                    .pointer("/security/auth/selectedType")
                    .and_then(|v| v.as_str())
            {
                accounts.push(AccountInfo {
                    label: "Antigravity".to_string(),
                    identity: auth_type.to_string(),
                    plan: None,
                    auth_method: Some(auth_type.to_string()),
                    is_active: true,
                });
            }

            let google_accounts = home.join(".gemini/google_accounts.json");
            if let Some(val) = read_json(&google_accounts)
                && let Some(active) = val.get("active").and_then(|v| v.as_str())
                && !active.is_empty()
            {
                accounts.clear();
                accounts.push(AccountInfo {
                    label: "Google".to_string(),
                    identity: active.to_string(),
                    plan: Some("Antigravity".to_string()),
                    auth_method: Some("OAuth".to_string()),
                    is_active: true,
                });
            }

            let agy_dir = home.join(".gemini/antigravity");
            if accounts.is_empty() && agy_dir.is_dir() {
                accounts.push(AccountInfo {
                    label: "Antigravity".to_string(),
                    identity: "Local session state".to_string(),
                    plan: None,
                    auth_method: Some("Configured".to_string()),
                    is_active: true,
                });
            }

            let brain_dir = agy_dir.join("brain");
            recent_sessions = collect_recent_brain_sessions(&brain_dir, 5);

            let conv_count = count_dir_entries(&agy_dir.join("conversations"));
            if conv_count > 0 || !recent_sessions.is_empty() {
                quotas.push(QuotaMetric {
                    name: "Conversations".to_string(),
                    used_percent: None,
                    remaining_percent: None,
                    summary: format!("{conv_count} stored conversations"),
                    resets_at: None,
                });
            }
        }

        AgentSnapshot {
            kind: AgentKind::Antigravity,
            install_status,
            accounts,
            quotas,
            active_sessions: active_sessions.to_vec(),
            recent_sessions,
            note: None,
        }
    }
}

fn collect_recent_brain_sessions(brain_dir: &PathBuf, limit: usize) -> Vec<RecentSession> {
    let Ok(entries) = fs::read_dir(brain_dir) else {
        return Vec::new();
    };

    let mut items: Vec<(DateTime<Utc>, RecentSession)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|n| n.to_str()).map(ToString::to_string) else {
            continue;
        };
        if id == "tempmediaStorage" {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(DateTime::<Utc>::from);

        let title = read_task_title(&path).unwrap_or_else(|| {
            let short: String = id.chars().take(8).collect();
            format!("Session {short}")
        });

        let sort_key = modified.unwrap_or(DateTime::<Utc>::MIN_UTC);
        items.push((
            sort_key,
            RecentSession {
                id,
                agent: AgentKind::Antigravity,
                title,
                project_dir: None,
                updated_at: modified,
            },
        ));
    }

    items.sort_by_key(|item| std::cmp::Reverse(item.0));
    items.into_iter().take(limit).map(|(_, s)| s).collect()
}

fn read_task_title(session_dir: &std::path::Path) -> Option<String> {
    let task_md = session_dir.join("task.md");
    let content = fs::read_to_string(task_md).ok()?;
    for line in content.lines() {
        let trimmed = line.trim().trim_start_matches('#').trim();
        if !trimmed.is_empty() {
            return Some(trimmed.chars().take(48).collect());
        }
    }
    None
}

fn count_dir_entries(dir: &std::path::Path) -> usize {
    fs::read_dir(dir).map_or(0, |entries| entries.flatten().count())
}
