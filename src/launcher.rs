use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::AgentKind;
use crate::scanner::{find_app_bundle, find_binary, home_dir};

/// Supported macOS terminal emulators for launching interactive CLI agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKind {
    Auto,
    Ghostty,
    ITerm,
    Warp,
    TerminalApp,
}

impl TerminalKind {
    #[must_use]
    pub const fn all_selectable() -> &'static [Self] {
        &[
            Self::Auto,
            Self::Ghostty,
            Self::ITerm,
            Self::Warp,
            Self::TerminalApp,
        ]
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Auto => "Auto-detect",
            Self::Ghostty => "Ghostty",
            Self::ITerm => "iTerm2",
            Self::Warp => "Warp",
            Self::TerminalApp => "Terminal.app",
        }
    }

    /// Resolves `Auto` to the best installed terminal emulator on this Mac.
    #[must_use]
    pub fn resolve(self) -> Self {
        if self != Self::Auto {
            return self;
        }
        if find_app_bundle("Ghostty").is_some() {
            Self::Ghostty
        } else if find_app_bundle("iTerm").is_some() {
            Self::ITerm
        } else if find_app_bundle("Warp").is_some() {
            Self::Warp
        } else {
            Self::TerminalApp
        }
    }
}

/// Launches an interactive CLI agent inside a macOS terminal emulator.
pub fn launch_agent_cli(
    agent: AgentKind,
    cwd: Option<&Path>,
    prompt: Option<&str>,
    terminal: TerminalKind,
) -> Result<(), String> {
    let bin = resolve_agent_binary(agent)
        .ok_or_else(|| format!("{} CLI binary not found", agent.display_name()))?;

    let target_dir = cwd
        .map(PathBuf::from)
        .or_else(home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));

    let cmd = build_agent_shell_command(agent, &bin, &target_dir, prompt);
    run_in_terminal(&cmd, terminal.resolve())
}

/// Launches a GUI agent app (e.g., Cursor, Antigravity IDE, Claude).
pub fn launch_agent_gui(agent: AgentKind, cwd: Option<&Path>) -> Result<(), String> {
    let app_name = agent
        .gui_app_name()
        .ok_or_else(|| format!("{} has no GUI app", agent.display_name()))?;

    let mut command = Command::new("open");
    command.args(["-a", app_name]);
    if let Some(dir) = cwd {
        command.arg(dir);
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open {app_name}: {e}"))
}

/// Opens a folder in macOS Finder.
pub fn open_in_finder(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open Finder: {e}"))
}

/// Opens a terminal window at the given directory.
pub fn open_terminal_at(path: &Path, terminal: TerminalKind) -> Result<(), String> {
    let cmd = format!("cd {} && clear", shell_quote(&path.display().to_string()));
    run_in_terminal(&cmd, terminal.resolve())
}

/// Sends `SIGTERM` to a running agent process.
pub fn terminate_pid(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|e| format!("Failed to invoke kill: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("kill -TERM {pid} exited with {status}"))
    }
}

/// Opens a native macOS folder picker dialog.
#[must_use]
pub fn pick_project_folder() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Select Project Folder for Agent")
        .pick_folder()
}

/// Prompts the user for a short text input using a native macOS dialog.
#[must_use]
pub fn prompt_for_input(title: &str, message: &str) -> Option<String> {
    let script = format!(
        "display dialog {} default answer \"\" with title {} buttons {{\"Cancel\", \"Launch\"}} default button \"Launch\"",
        applescript_quote(message),
        applescript_quote(title)
    );

    let output = Command::new("osascript").args(["-e", &script]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = stdout
        .split("text returned:")
        .nth(1)?
        .trim()
        .to_string();

    if text.is_empty() { None } else { Some(text) }
}

fn resolve_agent_binary(agent: AgentKind) -> Option<PathBuf> {
    match agent {
        AgentKind::Antigravity => find_binary(&["~/.local/bin/agy", "~/.gemini/bin/agy"], "agy"),
        AgentKind::ClaudeCode => find_binary(
            &["~/.local/bin/claude", "/opt/homebrew/bin/claude", "/usr/local/bin/claude"],
            "claude",
        ),
        AgentKind::Cursor => find_binary(
            &[
                "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
                "/usr/local/bin/cursor",
                "/opt/homebrew/bin/cursor",
            ],
            "cursor",
        ),
        AgentKind::Codex => find_binary(
            &["~/.local/bin/codex", "/opt/homebrew/bin/codex", "/usr/local/bin/codex"],
            "codex",
        ),
        AgentKind::OpenCode => find_binary(
            &["/opt/homebrew/bin/opencode", "/usr/local/bin/opencode", "~/.local/bin/opencode"],
            "opencode",
        ),
        AgentKind::Copilot => find_binary(
            &["/opt/homebrew/bin/copilot", "/usr/local/bin/copilot", "~/.local/bin/copilot"],
            "copilot",
        ),
    }
}

#[must_use]
pub fn build_agent_shell_command(
    agent: AgentKind,
    bin: &Path,
    cwd: &Path,
    prompt: Option<&str>,
) -> String {
    let cd_part = format!("cd {}", shell_quote(&cwd.display().to_string()));
    let bin_part = shell_quote(&bin.display().to_string());

    let invoke = match (agent, prompt) {
        (AgentKind::OpenCode, Some(p)) => {
            format!("{bin_part} --prompt {}", shell_quote(p))
        }
        (_, Some(p)) => format!("{bin_part} {}", shell_quote(p)),
        (_, None) => bin_part,
    };

    format!("{cd_part} && {invoke}")
}

fn run_in_terminal(shell_cmd: &str, terminal: TerminalKind) -> Result<(), String> {
    match terminal {
        TerminalKind::Ghostty => {
            let full_cmd = format!("{shell_cmd}; exec $SHELL");
            Command::new("open")
                .args([
                    "-na",
                    "Ghostty.app",
                    "--args",
                    "-e",
                    "/bin/zsh",
                    "-lic",
                    &full_cmd,
                ])
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("Failed to launch Ghostty: {e}"))
        }
        TerminalKind::ITerm => {
            let script = format!(
                "tell application \"iTerm\"\n\
                 activate\n\
                 create window with default profile command {}\n\
                 end tell",
                applescript_quote(&format!("/bin/zsh -lic {}", shell_quote(shell_cmd)))
            );
            run_osascript(&script)
        }
        TerminalKind::Warp | TerminalKind::TerminalApp | TerminalKind::Auto => {
            let script = format!(
                "tell application \"Terminal\"\n\
                 activate\n\
                 do script {}\n\
                 end tell",
                applescript_quote(shell_cmd)
            );
            run_osascript(&script)
        }
    }
}

fn run_osascript(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| format!("Failed to run osascript: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("AppleScript error: {err}"))
    }
}

#[must_use]
pub fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn applescript_quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("hello world"), "'hello world'");
        assert_eq!(shell_quote("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn builds_agent_command_with_cd_and_prompt() {
        let cmd = build_agent_shell_command(
            AgentKind::ClaudeCode,
            Path::new("/Users/test/.local/bin/claude"),
            Path::new("/Users/test/my project"),
            Some("fix tests"),
        );
        assert_eq!(
            cmd,
            "cd '/Users/test/my project' && '/Users/test/.local/bin/claude' 'fix tests'"
        );
    }
}
