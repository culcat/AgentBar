use std::fs;

use crate::model::{
    AccountInfo, ActiveSession, AgentKind, AgentSnapshot, InstallStatus,
};
use crate::providers::AgentProvider;
use crate::scanner::{find_binary, home_dir, query_version};

pub struct CopilotProvider;

impl AgentProvider for CopilotProvider {
    fn kind(&self) -> AgentKind {
        AgentKind::Copilot
    }

    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot {
        let bin = find_binary(
            &[
                "/opt/homebrew/bin/copilot",
                "/usr/local/bin/copilot",
                "~/.local/bin/copilot",
            ],
            "copilot",
        );
        let gh_bin = find_binary(&["/opt/homebrew/bin/gh", "/usr/local/bin/gh"], "gh");

        let copilot_dir = home_dir().map(|h| h.join(".copilot"));
        let has_dir = copilot_dir.as_ref().is_some_and(|d| d.is_dir());

        let install_status = if let Some(ref path) = bin {
            InstallStatus::Installed {
                version: query_version(path, &["--version"]),
                location: path.display().to_string(),
            }
        } else if let Some(ref gh) = gh_bin
            && has_dir
        {
            InstallStatus::Installed {
                version: query_version(gh, &["--version"]),
                location: gh.display().to_string(),
            }
        } else {
            InstallStatus::NotInstalled
        };

        let mut accounts = Vec::new();
        if let Some(home) = home_dir() {
            let gh_hosts = home.join(".config/gh/hosts.yml");
            if let Ok(content) = fs::read_to_string(&gh_hosts)
                && let Some(user) = extract_yaml_user(&content)
            {
                accounts.push(AccountInfo {
                    label: "GitHub".to_string(),
                    identity: user,
                    plan: Some("Copilot".to_string()),
                    auth_method: Some("gh CLI".to_string()),
                    is_active: true,
                });
            }
        }

        AgentSnapshot {
            kind: AgentKind::Copilot,
            install_status,
            accounts,
            quotas: Vec::new(),
            active_sessions: active_sessions.to_vec(),
            recent_sessions: Vec::new(),
            note: None,
        }
    }
}

fn extract_yaml_user(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("user:") {
            let user = rest.trim().trim_matches('"').trim_matches('\'');
            if !user.is_empty() {
                return Some(user.to_string());
            }
        }
    }
    None
}
