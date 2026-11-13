use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use muda::MenuEvent;
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::icon::build_tray_icon;
use crate::launcher::{
    TerminalKind, launch_agent_cli, launch_agent_gui, open_in_finder, open_terminal_at,
    pick_project_folder, prompt_for_input, terminate_pid,
};
use crate::menu::{ActionMap, MenuAction, build_menu};
use crate::model::BarSnapshot;
use crate::providers::collect_bar_snapshot;

const AUTO_REFRESH_SECS: u64 = 30;

#[derive(Debug)]
enum UserEvent {
    SnapshotReady(BarSnapshot),
    MenuTriggered(MenuEvent),
}

struct AppState {
    tray: TrayIcon,
    snapshot: BarSnapshot,
    actions: ActionMap,
    auto_refresh: Arc<AtomicBool>,
    terminal: TerminalKind,
    refresh_tx: std::sync::mpsc::Sender<()>,
}

/// Starts the native macOS menu bar application event loop.
pub fn run() {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy = event_loop.create_proxy();
    setup_menu_event_forwarder(proxy.clone());

    let (refresh_tx, refresh_rx) = std::sync::mpsc::channel::<()>();
    let auto_refresh = Arc::new(AtomicBool::new(true));
    spawn_scanner_thread(refresh_rx, proxy.clone());
    spawn_auto_refresh_ticker(Arc::clone(&auto_refresh), refresh_tx.clone());

    let initial_snapshot = collect_bar_snapshot();
    let initial_terminal = TerminalKind::Auto;
    let (initial_menu, initial_actions) = build_menu(&initial_snapshot, true, initial_terminal);

    let icon = build_tray_icon().expect("valid RGBA tray icon");
    let title = format_tray_title(&initial_snapshot);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(initial_menu))
        .with_menu_on_left_click(true)
        .with_icon(icon)
        .with_icon_as_template(true)
        .with_title(title)
        .with_tooltip("AgentBar — AI Agents Monitor")
        .build()
        .expect("failed to create macOS tray icon");

    let mut state = AppState {
        tray,
        snapshot: initial_snapshot,
        actions: initial_actions,
        auto_refresh,
        terminal: initial_terminal,
        refresh_tx,
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::UserEvent(user_event) = event {
            match user_event {
                UserEvent::SnapshotReady(snapshot) => {
                    state.snapshot = snapshot;
                    rebuild_tray_menu(&mut state);
                }
                UserEvent::MenuTriggered(menu_event) => {
                    if let Some(action) = state.actions.get(&menu_event.id).cloned()
                        && handle_menu_action(&mut state, action)
                    {
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
        }
    });
}

fn setup_menu_event_forwarder(proxy: EventLoopProxy<UserEvent>) {
    let receiver = MenuEvent::receiver();
    thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if proxy.send_event(UserEvent::MenuTriggered(event)).is_err() {
                break;
            }
        }
    });
}

fn spawn_scanner_thread(
    refresh_rx: std::sync::mpsc::Receiver<()>,
    proxy: EventLoopProxy<UserEvent>,
) {
    thread::spawn(move || {
        while refresh_rx.recv().is_ok() {
            // Drain any queued duplicate refresh requests.
            while refresh_rx.try_recv().is_ok() {}
            let snapshot = collect_bar_snapshot();
            if proxy.send_event(UserEvent::SnapshotReady(snapshot)).is_err() {
                break;
            }
        }
    });
}

fn spawn_auto_refresh_ticker(
    auto_refresh: Arc<AtomicBool>,
    refresh_tx: std::sync::mpsc::Sender<()>,
) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(AUTO_REFRESH_SECS));
            if auto_refresh.load(Ordering::Relaxed) && refresh_tx.send(()).is_err() {
                break;
            }
        }
    });
}

fn rebuild_tray_menu(state: &mut AppState) {
    let auto = state.auto_refresh.load(Ordering::Relaxed);
    let (menu, actions) = build_menu(&state.snapshot, auto, state.terminal);
    state.actions = actions;
    state.tray.set_menu(Some(Box::new(menu)));
    state.tray.set_title(Some(format_tray_title(&state.snapshot)));
}

fn format_tray_title(snapshot: &BarSnapshot) -> String {
    let active = snapshot.active_sessions.len();
    if active > 0 {
        format!(" {active}")
    } else {
        String::new()
    }
}

/// Executes a menu action. Returns `true` if the application should exit.
fn handle_menu_action(state: &mut AppState, action: MenuAction) -> bool {
    match action {
        MenuAction::RefreshNow => {
            let _ = state.refresh_tx.send(());
        }
        MenuAction::ToggleAutoRefresh => {
            let current = state.auto_refresh.load(Ordering::Relaxed);
            state.auto_refresh.store(!current, Ordering::Relaxed);
            rebuild_tray_menu(state);
        }
        MenuAction::SelectTerminal(terminal) => {
            state.terminal = terminal;
            rebuild_tray_menu(state);
        }
        MenuAction::LaunchCli {
            agent,
            cwd,
            pick_folder,
            with_prompt,
        } => {
            let terminal = state.terminal;
            let refresh_tx = state.refresh_tx.clone();
            thread::spawn(move || {
                let target_cwd = if pick_folder {
                    let Some(picked) = pick_project_folder() else {
                        return;
                    };
                    Some(picked)
                } else {
                    cwd
                };

                let prompt = if with_prompt {
                    let Some(text) = prompt_for_input(
                        &format!("Launch {}", agent.display_name()),
                        "Enter initial prompt for the agent:",
                    ) else {
                        return;
                    };
                    Some(text)
                } else {
                    None
                };

                if let Err(err) =
                    launch_agent_cli(agent, target_cwd.as_deref(), prompt.as_deref(), terminal)
                {
                    log::error!("{err}");
                } else {
                    thread::sleep(Duration::from_millis(800));
                    let _ = refresh_tx.send(());
                }
            });
        }
        MenuAction::LaunchGui { agent, cwd } => {
            if let Err(err) = launch_agent_gui(agent, cwd.as_deref()) {
                log::error!("{err}");
            }
        }
        MenuAction::OpenInFinder(path) => {
            if let Err(err) = open_in_finder(&path) {
                log::error!("{err}");
            }
        }
        MenuAction::OpenTerminal(path) => {
            if let Err(err) = open_terminal_at(&path, state.terminal) {
                log::error!("{err}");
            }
        }
        MenuAction::TerminateProcess(pid) => {
            if let Err(err) = terminate_pid(pid) {
                log::error!("{err}");
            } else {
                let _ = state.refresh_tx.send(());
            }
        }
        MenuAction::Quit => return true,
    }

    false
}
