use std::path::PathBuf;

use crate::model::{
    AccountInfo, ActiveSession, AgentKind, AgentSnapshot, InstallStatus, QuotaMetric, RecentSession,
};
use crate::providers::AgentProvider;
use crate::scanner::{
    find_binary, home_dir, open_sqlite_readonly, query_version, read_json, read_jsonc,
    timestamp_to_utc,
};

pub struct OpenCodeProvider;

impl AgentProvider for OpenCodeProvider {
    fn kind(&self) -> AgentKind {
        AgentKind::OpenCode
    }

    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot {
        let bin = find_binary(
            &[
                "/opt/homebrew/bin/opencode",
                "/usr/local/bin/opencode",
                "~/.local/bin/opencode",
            ],
            "opencode",
        );

        let install_status = if let Some(ref path) = bin {
            let version = query_version(path, &["--version"]);
            InstallStatus::Installed {
                version,
                location: path.display().to_string(),
            }
        } else {
            InstallStatus::NotInstalled
        };

        let mut accounts = Vec::new();
        let mut quotas = Vec::new();
        let mut recent_sessions = Vec::new();

        if let Some(home) = home_dir() {
            collect_opencode_local_state(&home, &mut accounts, &mut quotas, &mut recent_sessions);
        }

        AgentSnapshot {
            kind: AgentKind::OpenCode,
            install_status,
            accounts,
            quotas,
            active_sessions: active_sessions.to_vec(),
            recent_sessions,
            note: None,
        }
    }
}

fn collect_opencode_local_state(
    home: &std::path::Path,
    accounts: &mut Vec<AccountInfo>,
    quotas: &mut Vec<QuotaMetric>,
    recent_sessions: &mut Vec<RecentSession>,
) {
    let auth_path = home.join(".local/share/opencode/auth.json");
    if let Some(auth) = read_json(&auth_path)
        && let Some(obj) = auth.as_object()
    {
        for (provider_name, cfg) in obj {
            let auth_type = cfg
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("configured");
            accounts.push(AccountInfo {
                label: provider_name.clone(),
                identity: format!("{provider_name} ({auth_type})"),
                plan: None,
                auth_method: Some(auth_type.to_string()),
                is_active: true,
            });
        }
    }

    let config_path = home.join(".config/opencode/opencode.jsonc");
    if let Some(cfg) = read_jsonc(&config_path)
        && let Some(model) = cfg.get("model").and_then(|v| v.as_str())
    {
        quotas.push(QuotaMetric {
            name: "Default Model".to_string(),
            used_percent: None,
            remaining_percent: None,
            summary: model.to_string(),
            resets_at: None,
        });
    }

    let db_path = home.join(".local/share/opencode/opencode.db");
    if let Some(conn) = open_sqlite_readonly(&db_path) {
        if let Ok((total_cost, total_in, total_out)) = conn.query_row(
            "SELECT COALESCE(SUM(cost), 0.0), \
             COALESCE(SUM(tokens_input), 0), \
             COALESCE(SUM(tokens_output), 0) FROM session",
            [],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        ) {
            let total_tokens = total_in + total_out;
            if total_tokens > 0 || total_cost > 0.0 {
                quotas.push(QuotaMetric {
                    name: "Usage".to_string(),
                    used_percent: None,
                    remaining_percent: None,
                    summary: format!("{} · ${total_cost:.2}", format_tokens(total_tokens)),
                    resets_at: None,
                });
            }
        }

        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, COALESCE(NULLIF(title, ''), slug, 'OpenCode session'), directory, time_updated \
             FROM session WHERE time_archived IS NULL \
             ORDER BY time_updated DESC LIMIT 5",
        ) && let Ok(rows) = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let dir: Option<String> = row.get(2)?;
            let updated: Option<i64> = row.get(3)?;
            Ok(RecentSession {
                id,
                agent: AgentKind::OpenCode,
                title: title.chars().take(48).collect(),
                project_dir: dir.filter(|s| !s.is_empty()).map(PathBuf::from),
                updated_at: updated.and_then(timestamp_to_utc),
            })
        }) {
            *recent_sessions = rows.flatten().collect();
        }
    }
}

fn format_tokens(tokens: i64) -> String {
    if tokens >= 1_000_000 {
        #[allow(clippy::cast_precision_loss)]
        let m = tokens as f64 / 1_000_000.0;
        format!("{m:.2}M tok")
    } else if tokens >= 1_000 {
        #[allow(clippy::cast_precision_loss)]
        let k = tokens as f64 / 1_000.0;
        format!("{k:.1}K tok")
    } else {
        format!("{tokens} tok")
    }
}
