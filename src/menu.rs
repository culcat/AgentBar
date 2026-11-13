use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use muda::{
    CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};

use crate::launcher::TerminalKind;
use crate::model::{
    AgentKind, AgentSnapshot, BarSnapshot, InstallStatus, format_relative_time,
};

/// Actions triggered by clicking items in the `AgentBar` menu.
#[derive(Debug, Clone)]
pub enum MenuAction {
    RefreshNow,
    ToggleAutoRefresh,
    SelectTerminal(TerminalKind),
    LaunchCli {
        agent: AgentKind,
        cwd: Option<PathBuf>,
        pick_folder: bool,
        with_prompt: bool,
    },
    LaunchGui {
        agent: AgentKind,
        cwd: Option<PathBuf>,
    },
    OpenInFinder(PathBuf),
    OpenTerminal(PathBuf),
    TerminateProcess(u32),
    Quit,
}

/// Maps `muda::MenuId` values to strongly-typed `MenuAction`s.
pub type ActionMap = HashMap<MenuId, MenuAction>;

/// Builds the native macOS `NSMenu` from the latest `BarSnapshot`.
pub fn build_menu(
    snapshot: &BarSnapshot,
    auto_refresh: bool,
    terminal: TerminalKind,
) -> (Menu, ActionMap) {
    let menu = Menu::new();
    let mut actions = ActionMap::new();
    let now = Utc::now();

    let header = MenuItem::new(
        format!(
            "AgentBar  ·  {} active  ·  {}/{} ready",
            snapshot.active_sessions.len(),
            snapshot.ready_count(),
            snapshot.installed_count()
        ),
        false,
        None,
    );
    let updated = MenuItem::new(
        format!(
            "Updated {} ({})",
            snapshot.refreshed_at.format("%H:%M:%S"),
            format_relative_time(snapshot.refreshed_at, now)
        ),
        false,
        None,
    );
    let _ = menu.append(&header);
    let _ = menu.append(&updated);
    let _ = menu.append(&PredefinedMenuItem::separator());

    append_active_sessions_section(&menu, snapshot, &mut actions);
    let _ = menu.append(&PredefinedMenuItem::separator());

    append_agents_section(&menu, snapshot, now, &mut actions);
    let _ = menu.append(&PredefinedMenuItem::separator());

    append_quick_launch_section(&menu, snapshot, &mut actions);
    let _ = menu.append(&PredefinedMenuItem::separator());

    append_footer_section(&menu, auto_refresh, terminal, &mut actions);

    (menu, actions)
}

fn append_active_sessions_section(
    menu: &Menu,
    snapshot: &BarSnapshot,
    actions: &mut ActionMap,
) {
    let title = format!("Active Sessions ({})", snapshot.active_sessions.len());
    let _ = menu.append(&MenuItem::new(title, false, None));

    if snapshot.active_sessions.is_empty() {
        let _ = menu.append(&MenuItem::new("  No active agent sessions", false, None));
        return;
    }

    for session in &snapshot.active_sessions {
        let label = format!(
            "🟢 {}  ·  {}  (PID {})",
            session.agent.display_name(),
            session.project_label(),
            session.pid
        );
        let sub = Submenu::new(label, true);

        let _ = sub.append(&MenuItem::new(format!("Process: {}", session.title), false, None));
        let _ = sub.append(&MenuItem::new(
            format!("CPU: {:.1}%  ·  RAM: {} MB", session.cpu_percent, session.memory_mb),
            false,
            None,
        ));
        let _ = sub.append(&MenuItem::new(
            format!("Elapsed: {}", session.started_at),
            false,
            None,
        ));

        if let Some(ref cwd) = session.working_dir {
            let _ = sub.append(&MenuItem::new(
                format!("Dir: {}", cwd.display()),
                false,
                None,
            ));
            let _ = sub.append(&PredefinedMenuItem::separator());

            let finder_item = MenuItem::new("Open Folder in Finder", true, None);
            actions.insert(finder_item.id().clone(), MenuAction::OpenInFinder(cwd.clone()));
            let _ = sub.append(&finder_item);

            let term_item = MenuItem::new("Open Terminal Here", true, None);
            actions.insert(term_item.id().clone(), MenuAction::OpenTerminal(cwd.clone()));
            let _ = sub.append(&term_item);
        }

        let _ = sub.append(&PredefinedMenuItem::separator());
        let kill_item = MenuItem::new("Terminate Process (SIGTERM)", true, None);
        actions.insert(kill_item.id().clone(), MenuAction::TerminateProcess(session.pid));
        let _ = sub.append(&kill_item);

        let _ = menu.append(&sub);
    }
}

fn append_agents_section(
    menu: &Menu,
    snapshot: &BarSnapshot,
    now: chrono::DateTime<Utc>,
    actions: &mut ActionMap,
) {
    let _ = menu.append(&MenuItem::new("Agents & Accounts", false, None));

    for agent in &snapshot.agents {
        let badge = agent_summary_badge(agent);
        let label = format!("{} {}  —  {}", agent.status_dot(), agent.kind.display_name(), badge);
        let sub = Submenu::new(label, true);

        match &agent.install_status {
            InstallStatus::Installed { version, location } => {
                let ver_text = version.as_deref().unwrap_or("installed");
                let _ = sub.append(&MenuItem::new(format!("Version: {ver_text}"), false, None));
                let _ = sub.append(&MenuItem::new(format!("Path: {location}"), false, None));
            }
            InstallStatus::NotInstalled => {
                let _ = sub.append(&MenuItem::new("Status: Not installed", false, None));
            }
        }

        let _ = sub.append(&PredefinedMenuItem::separator());
        let _ = sub.append(&MenuItem::new("Accounts", false, None));
        if agent.accounts.is_empty() {
            let _ = sub.append(&MenuItem::new("  ○ No account detected", false, None));
        } else {
            for acc in &agent.accounts {
                let plan_suffix = acc
                    .plan
                    .as_ref()
                    .map_or_else(String::new, |p| format!(" [{p}]"));
                let marker = if acc.is_active { "●" } else { "○" };
                let _ = sub.append(&MenuItem::new(
                    format!("  {marker} {}{plan_suffix}", acc.identity),
                    false,
                    None,
                ));
            }
        }

        if !agent.quotas.is_empty() {
            let _ = sub.append(&PredefinedMenuItem::separator());
            let _ = sub.append(&MenuItem::new("Limits & Usage", false, None));
            for quota in &agent.quotas {
                let detail = if quota.used_percent.is_some() || quota.remaining_percent.is_some() {
                    let reset = quota
                        .format_reset(now)
                        .map_or_else(String::new, |r| format!(" ({r})"));
                    format!("  {}: {}{reset}", quota.name, quota.format_bar(8))
                } else {
                    format!("  {}: {}", quota.name, quota.summary)
                };
                let _ = sub.append(&MenuItem::new(detail, false, None));
            }
        }

        if !agent.recent_sessions.is_empty() {
            let _ = sub.append(&PredefinedMenuItem::separator());
            let _ = sub.append(&MenuItem::new("Recent Sessions", false, None));
            for recent in &agent.recent_sessions {
                let age = recent
                    .updated_at
                    .map_or_else(String::new, |dt| format!(" ({})", format_relative_time(dt, now)));
                let item = MenuItem::new(
                    format!("  {}{age}", recent.title),
                    recent.project_dir.is_some() && agent.kind.supports_cli_launch(),
                    None,
                );
                if let Some(ref dir) = recent.project_dir
                    && agent.kind.supports_cli_launch()
                {
                    actions.insert(
                        item.id().clone(),
                        MenuAction::LaunchCli {
                            agent: agent.kind,
                            cwd: Some(dir.clone()),
                            pick_folder: false,
                            with_prompt: false,
                        },
                    );
                }
                let _ = sub.append(&item);
            }
        }

        if agent.install_status.is_installed() {
            let _ = sub.append(&PredefinedMenuItem::separator());
            append_agent_launch_items(&sub, agent.kind, actions);
        }

        let _ = menu.append(&sub);
    }
}

fn append_agent_launch_items(sub: &Submenu, kind: AgentKind, actions: &mut ActionMap) {
    if kind.supports_cli_launch() {
        let launch_home = MenuItem::new(format!("Launch {} CLI", kind.slug()), true, None);
        actions.insert(
            launch_home.id().clone(),
            MenuAction::LaunchCli {
                agent: kind,
                cwd: None,
                pick_folder: false,
                with_prompt: false,
            },
        );
        let _ = sub.append(&launch_home);

        let launch_folder = MenuItem::new("Launch in Folder…", true, None);
        actions.insert(
            launch_folder.id().clone(),
            MenuAction::LaunchCli {
                agent: kind,
                cwd: None,
                pick_folder: true,
                with_prompt: false,
            },
        );
        let _ = sub.append(&launch_folder);

        let launch_prompt = MenuItem::new("Launch with Prompt…", true, None);
        actions.insert(
            launch_prompt.id().clone(),
            MenuAction::LaunchCli {
                agent: kind,
                cwd: None,
                pick_folder: false,
                with_prompt: true,
            },
        );
        let _ = sub.append(&launch_prompt);
    }

    if let Some(gui_name) = kind.gui_app_name()
        && crate::scanner::find_app_bundle(gui_name).is_some()
    {
        let open_gui = MenuItem::new(format!("Open {gui_name}"), true, None);
        actions.insert(
            open_gui.id().clone(),
            MenuAction::LaunchGui {
                agent: kind,
                cwd: None,
            },
        );
        let _ = sub.append(&open_gui);
    }
}

fn append_quick_launch_section(
    menu: &Menu,
    snapshot: &BarSnapshot,
    actions: &mut ActionMap,
) {
    let quick = Submenu::new("⚡ Quick Launch Agent", true);
    let mut added = 0_usize;

    for agent in &snapshot.agents {
        if !agent.install_status.is_installed() {
            continue;
        }
        if agent.kind.supports_cli_launch() {
            let item = MenuItem::new(
                format!("New {} Session", agent.kind.display_name()),
                true,
                None,
            );
            actions.insert(
                item.id().clone(),
                MenuAction::LaunchCli {
                    agent: agent.kind,
                    cwd: None,
                    pick_folder: false,
                    with_prompt: false,
                },
            );
            let _ = quick.append(&item);
            added += 1;
        } else if let Some(gui) = agent.kind.gui_app_name() {
            let item = MenuItem::new(format!("Open {gui}"), true, None);
            actions.insert(
                item.id().clone(),
                MenuAction::LaunchGui {
                    agent: agent.kind,
                    cwd: None,
                },
            );
            let _ = quick.append(&item);
            added += 1;
        }
    }

    if added == 0 {
        let _ = quick.append(&MenuItem::new("No installed agents found", false, None));
    }

    let _ = menu.append(&quick);
}

fn append_footer_section(
    menu: &Menu,
    auto_refresh: bool,
    terminal: TerminalKind,
    actions: &mut ActionMap,
) {
    let term_sub = Submenu::new(
        format!("Terminal: {}", terminal.resolve().display_name()),
        true,
    );
    for kind in TerminalKind::all_selectable() {
        let check = CheckMenuItem::new(kind.display_name(), true, *kind == terminal, None);
        actions.insert(check.id().clone(), MenuAction::SelectTerminal(*kind));
        let _ = term_sub.append(&check);
    }
    let _ = menu.append(&term_sub);

    let auto_item = CheckMenuItem::new("Auto-Refresh (30s)", true, auto_refresh, None);
    actions.insert(auto_item.id().clone(), MenuAction::ToggleAutoRefresh);
    let _ = menu.append(&auto_item);

    let refresh_item = MenuItem::new("Refresh Now", true, None);
    actions.insert(refresh_item.id().clone(), MenuAction::RefreshNow);
    let _ = menu.append(&refresh_item);

    let _ = menu.append(&PredefinedMenuItem::separator());

    let quit_item = MenuItem::new("Quit AgentBar", true, None);
    actions.insert(quit_item.id().clone(), MenuAction::Quit);
    let _ = menu.append(&quit_item);
}

fn agent_summary_badge(agent: &AgentSnapshot) -> String {
    if !agent.active_sessions.is_empty() {
        return format!("{} running", agent.active_sessions.len());
    }
    if let Some(acc) = agent.accounts.first() {
        return acc.identity.clone();
    }
    if agent.install_status.is_installed() {
        "installed".to_string()
    } else {
        "not installed".to_string()
    }
}
