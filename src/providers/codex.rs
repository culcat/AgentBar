use std::path::PathBuf;

use crate::model::{
    AccountInfo, ActiveSession, AgentKind, AgentSnapshot, InstallStatus, QuotaMetric, RecentSession,
};
use crate::providers::AgentProvider;
use crate::scanner::{
    find_binary, home_dir, open_sqlite_readonly, query_version, read_json, timestamp_to_utc,
};

pub struct CodexProvider;

impl AgentProvider for CodexProvider {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot {
        let bin = find_binary(
            &[
                "~/.local/bin/codex",
                "/opt/homebrew/bin/codex",
                "/usr/local/bin/codex",
            ],
            "codex",
        );

        let codex_dir = home_dir().map(|h| h.join(".codex"));
        let has_dir = codex_dir.as_ref().is_some_and(|d| d.is_dir());

        let install_status = if let Some(ref path) = bin {
            let version = query_version(path, &["--version"]);
            InstallStatus::Installed {
                version,
                location: path.display().to_string(),
            }
        } else if has_dir {
            InstallStatus::Installed {
                version: None,
                location: codex_dir
                    .as_ref()
                    .map_or_else(String::new, |p| p.display().to_string()),
            }
        } else {
            InstallStatus::NotInstalled
        };

        let mut accounts = Vec::new();
        let mut quotas = Vec::new();
        let mut recent_sessions = Vec::new();

        if let Some(dir) = codex_dir {
            let auth_path = dir.join("auth.json");
            if let Some(auth) = read_json(&auth_path) {
                let auth_mode = auth
                    .get("auth_mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ChatGPT");
                let has_api_key = auth
                    .get("OPENAI_API_KEY")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty());

                accounts.push(AccountInfo {
                    label: "OpenAI Codex".to_string(),
                    identity: if has_api_key {
                        "API Key configured".to_string()
                    } else {
                        auth_mode.to_string()
                    },
                    plan: Some(auth_mode.to_string()),
                    auth_method: Some(if has_api_key { "API Key" } else { "OAuth" }.to_string()),
                    is_active: true,
                });
            }

            let db_path = dir.join("state_5.sqlite");
            if let Some(conn) = open_sqlite_readonly(&db_path) {
                if let Ok(total_tokens) = conn.query_row(
                    "SELECT COALESCE(SUM(tokens_used), 0) FROM threads",
                    [],
                    |row| row.get::<_, i64>(0),
                ) && total_tokens > 0
                {
                    quotas.push(QuotaMetric {
                        name: "Total Tokens".to_string(),
                        used_percent: None,
                        remaining_percent: None,
                        summary: format_tokens(total_tokens),
                        resets_at: None,
                    });
                }

                if let Ok(mut stmt) = conn.prepare(
                    "SELECT id, COALESCE(NULLIF(title, ''), first_user_message, 'Codex thread'), cwd, updated_at \
                     FROM threads WHERE COALESCE(archived, 0) = 0 \
                     ORDER BY updated_at DESC LIMIT 5",
                ) && let Ok(rows) = stmt.query_map([], |row| {
                    let id: String = row.get(0)?;
                    let title: String = row.get(1)?;
                    let cwd: Option<String> = row.get(2)?;
                    let updated_at: Option<i64> = row.get(3)?;
                    Ok(RecentSession {
                        id,
                        agent: AgentKind::Codex,
                        title: title.chars().take(48).collect(),
                        project_dir: cwd.filter(|s| !s.is_empty()).map(PathBuf::from),
                        updated_at: updated_at.and_then(timestamp_to_utc),
                    })
                }) {
                    recent_sessions = rows.flatten().collect();
                }
            }
        }

        AgentSnapshot {
            kind: AgentKind::Codex,
            install_status,
            accounts,
            quotas,
            active_sessions: active_sessions.to_vec(),
            recent_sessions,
            note: None,
        }
    }
}

fn format_tokens(tokens: i64) -> String {
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
