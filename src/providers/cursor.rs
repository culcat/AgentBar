use std::fs;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;

use crate::model::{
    AccountInfo, ActiveSession, AgentKind, AgentSnapshot, InstallStatus, QuotaMetric, RecentSession,
};
use crate::providers::AgentProvider;
use crate::scanner::{find_app_bundle, find_binary, home_dir, open_sqlite_readonly, query_version};

pub struct CursorProvider;

impl AgentProvider for CursorProvider {
    fn kind(&self) -> AgentKind {
        AgentKind::Cursor
    }

    fn collect(&self, active_sessions: &[ActiveSession]) -> AgentSnapshot {
        let app = find_app_bundle("Cursor");
        let bin = find_binary(
            &[
                "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
                "/usr/local/bin/cursor",
                "/opt/homebrew/bin/cursor",
            ],
            "cursor",
        );

        let cursor_dir = home_dir().map(|h| h.join(".cursor"));
        let has_dir = cursor_dir.as_ref().is_some_and(|d| d.is_dir());

        let install_status = if let Some(ref app_path) = app {
            InstallStatus::Installed {
                version: bin.as_deref().and_then(|p| query_version(p, &["--version"])),
                location: app_path.display().to_string(),
            }
        } else if let Some(ref path) = bin {
            InstallStatus::Installed {
                version: query_version(path, &["--version"]),
                location: path.display().to_string(),
            }
        } else if has_dir {
            InstallStatus::Installed {
                version: None,
                location: cursor_dir
                    .as_ref()
                    .map_or_else(String::new, |p| p.display().to_string()),
            }
        } else {
            InstallStatus::NotInstalled
        };

        let mut accounts = Vec::new();
        let mut quotas = Vec::new();
        let mut recent_sessions = Vec::new();

        if let Some(home) = home_dir() {
            let state_db = home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
            if let Some(conn) = open_sqlite_readonly(&state_db) {
                let email = query_item_table(&conn, "cursorAuth/cachedEmail");
                let plan = query_item_table(&conn, "cursorAuth/stripeMembershipType");
                if let Some(email_val) = email {
                    accounts.push(AccountInfo {
                        label: "Cursor".to_string(),
                        identity: email_val,
                        plan,
                        auth_method: Some("Cursor Auth".to_string()),
                        is_active: true,
                    });
                }
            }

            let projects_dir = home.join(".cursor/projects");
            if let Ok(entries) = fs::read_dir(&projects_dir) {
                let mut items: Vec<(DateTime<Utc>, RecentSession)> = Vec::new();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let Some(name) = path.file_name().and_then(|n| n.to_str()).map(ToString::to_string) else {
                        continue;
                    };
                    let modified = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .map(DateTime::<Utc>::from);
                    let sort_key = modified.unwrap_or(DateTime::<Utc>::MIN_UTC);
                    items.push((
                        sort_key,
                        RecentSession {
                            id: name.clone(),
                            agent: AgentKind::Cursor,
                            title: name.clone(),
                            project_dir: Some(PathBuf::from(format!("/{name}"))),
                            updated_at: modified,
                        },
                    ));
                }
                items.sort_by_key(|item| std::cmp::Reverse(item.0));
                let count = items.len();
                recent_sessions = items.into_iter().take(5).map(|(_, s)| s).collect();
                if count > 0 {
                    quotas.push(QuotaMetric {
                        name: "Projects".to_string(),
                        used_percent: None,
                        remaining_percent: None,
                        summary: format!("{count} tracked workspaces"),
                        resets_at: None,
                    });
                }
            }
        }

        AgentSnapshot {
            kind: AgentKind::Cursor,
            install_status,
            accounts,
            quotas,
            active_sessions: active_sessions.to_vec(),
            recent_sessions,
            note: None,
        }
    }
}

fn query_item_table(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM ItemTable WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|v| v.trim_matches('"').to_string())
    .filter(|v| !v.is_empty())
}
