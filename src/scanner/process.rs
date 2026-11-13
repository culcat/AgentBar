use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::model::{ActiveSession, AgentKind};

/// Discovers currently running AI agent processes on macOS.
#[must_use]
pub fn discover_active_sessions() -> Vec<ActiveSession> {
    let Ok(output) = Command::new("ps")
        .args(["-axo", "pid=,pcpu=,rss=,etime=,command="])
        .output()
    else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = parse_ps_output(&stdout);

    if !sessions.is_empty() {
        let pids: Vec<u32> = sessions.iter().map(|s| s.pid).collect();
        let cwds = resolve_cwds(&pids);
        for session in &mut sessions {
            if let Some(dir) = cwds.get(&session.pid) {
                session.working_dir = Some(dir.clone());
            }
        }
    }

    sessions
}

/// Parses `ps -axo pid=,pcpu=,rss=,etime=,command=` output into active sessions.
#[must_use]
pub fn parse_ps_output(stdout: &str) -> Vec<ActiveSession> {
    let mut sessions = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let Some(pid_str) = parts.next() else { continue };
        let Some(cpu_str) = parts.next() else { continue };
        let Some(rss_str) = parts.next() else { continue };
        let Some(etime_str) = parts.next() else { continue };

        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let cpu_percent = cpu_str.parse::<f32>().unwrap_or(0.0);
        let rss_kb = rss_str.parse::<u64>().unwrap_or(0);
        let memory_mb = rss_kb / 1024;

        let command_start = trimmed
            .find(etime_str)
            .map_or(0, |idx| idx + etime_str.len());
        let command = trimmed[command_start..].trim();
        if command.is_empty() {
            continue;
        }

        if let Some((agent, title)) = classify_process(command) {
            sessions.push(ActiveSession {
                pid,
                agent,
                title,
                working_dir: None,
                cpu_percent,
                memory_mb,
                started_at: etime_str.to_string(),
                command: truncate_command(command, 120),
            });
        }
    }

    sessions
}

/// Classifies a process command line into an `AgentKind` and short title.
#[must_use]
pub fn classify_process(command: &str) -> Option<(AgentKind, String)> {
    let lower = command.to_ascii_lowercase();

    // Ignore our own process, grep/ps/lsof wrappers, and macOS system XPC services.
    if lower.contains("agentbar")
        || lower.contains("cursoruiviewservice")
        || lower.contains("helper (renderer)")
        || lower.contains("helper (gpu)")
        || lower.contains("helper (plugin)")
        || lower.contains("helper.app")
        || lower.contains("crashpad")
    {
        return None;
    }

    let first_token = command.split_whitespace().next().unwrap_or("");
    let bin_name = std::path::Path::new(first_token)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(first_token)
        .to_ascii_lowercase();

    if bin_name == "claude" || lower.contains("/.local/bin/claude") {
        return Some((AgentKind::ClaudeCode, "Claude Code CLI".to_string()));
    }

    if bin_name == "codex" || lower.contains("/bin/codex") {
        return Some((AgentKind::Codex, "Codex CLI".to_string()));
    }

    if bin_name == "opencode" || lower.contains("/bin/opencode") {
        return Some((AgentKind::OpenCode, "OpenCode CLI".to_string()));
    }

    if bin_name == "agy" || lower.contains("/bin/agy") {
        let role = if lower.contains("--hub") {
            "Antigravity Hub"
        } else {
            "Antigravity CLI"
        };
        return Some((AgentKind::Antigravity, role.to_string()));
    }

    if command.contains("/Applications/Antigravity IDE.app/Contents/MacOS/")
        || command.contains("/Applications/Antigravity.app/Contents/MacOS/")
    {
        return Some((AgentKind::Antigravity, "Antigravity IDE".to_string()));
    }

    if command.contains("/Applications/Cursor.app/Contents/MacOS/Cursor")
        || bin_name == "cursor"
    {
        return Some((AgentKind::Cursor, "Cursor IDE".to_string()));
    }

    if command.contains("/Applications/Claude.app/Contents/MacOS/Claude") {
        return Some((AgentKind::ClaudeCode, "Claude Desktop".to_string()));
    }

    if bin_name == "copilot" || lower.contains("gh copilot") {
        return Some((AgentKind::Copilot, "Copilot CLI".to_string()));
    }

    None
}

fn resolve_cwds(pids: &[u32]) -> HashMap<u32, PathBuf> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let pid_list = pids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");

    let Ok(output) = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-Fn", "-p", &pid_list])
        .output()
    else {
        return map;
    };

    if !output.status.success() {
        return map;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_pid: Option<u32> = None;

    for line in stdout.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.parse::<u32>().ok();
        } else if let Some(path_str) = line.strip_prefix('n')
            && let Some(pid) = current_pid
        {
            map.insert(pid, PathBuf::from(path_str));
        }
    }

    map
}

fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.chars().count() <= max_len {
        cmd.to_string()
    } else {
        let prefix: String = cmd.chars().take(max_len.saturating_sub(1)).collect();
        format!("{prefix}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_real_agent_processes_and_ignores_system_xpc() {
        assert_eq!(
            classify_process("opencode"),
            Some((AgentKind::OpenCode, "OpenCode CLI".to_string()))
        );
        assert_eq!(
            classify_process("/Users/alex/.local/bin/claude --resume"),
            Some((AgentKind::ClaudeCode, "Claude Code CLI".to_string()))
        );
        assert_eq!(
            classify_process("/Users/alex/.gemini/bin/agy --hub --hub-port=50217"),
            Some((AgentKind::Antigravity, "Antigravity Hub".to_string()))
        );
        assert_eq!(
            classify_process("/Applications/Cursor.app/Contents/MacOS/Cursor"),
            Some((AgentKind::Cursor, "Cursor IDE".to_string()))
        );
        assert_eq!(
            classify_process("/System/Library/PrivateFrameworks/TextInputUIMacHelper.framework/Versions/A/XPCServices/CursorUIViewService.xpc/Contents/MacOS/CursorUIViewService"),
            None
        );
        assert_eq!(
            classify_process("/Applications/Antigravity IDE.app/Contents/Frameworks/Antigravity IDE Helper (Renderer).app/Contents/MacOS/Antigravity IDE Helper (Renderer)"),
            None
        );
    }

    #[test]
    fn parses_ps_lines_cleanly() {
        let sample = "\
  36657   0.1  10560   01:55:12 opencode
  47075   5.6  64352      12:34 /Users/alex/.gemini/bin/agy --hub --hub-port=50217
    982   0.8   6224 14-01:02:03 /System/Library/PrivateFrameworks/CursorUIViewService.xpc/Contents/MacOS/CursorUIViewService
";
        let sessions = parse_ps_output(sample);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].pid, 36657);
        assert_eq!(sessions[0].agent, AgentKind::OpenCode);
        assert_eq!(sessions[0].memory_mb, 10);
        assert_eq!(sessions[1].pid, 47075);
        assert_eq!(sessions[1].agent, AgentKind::Antigravity);
    }
}
