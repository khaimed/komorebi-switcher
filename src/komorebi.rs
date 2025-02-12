use std::io::{BufRead, BufReader};

use anyhow::Context;
use komorebi_client::{Notification, NotificationEvent, SocketMessage, State};
use winit::event_loop::EventLoopProxy;

use crate::app::AppMessage;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub idx: usize,
    pub focused: bool,
    pub is_empty: bool,
}

fn workspaces_from_state(state: State) -> anyhow::Result<Vec<Workspace>> {
    let monitor = state.monitors.focused().context("No focused monintor")?;

    let focused_workspace = monitor.focused_workspace_idx();

    let workspaces = monitor.workspaces().iter().enumerate().map(|(idx, w)| {
        let name = w.name().clone().unwrap_or_else(|| (idx + 1).to_string());
        let focused = focused_workspace == idx;

        Workspace {
            name,
            idx,
            focused,
            is_empty: w.is_empty(),
        }
    });

    Ok(workspaces.collect())
}

pub fn read_workspaces() -> anyhow::Result<Vec<Workspace>> {
    let response = komorebi_client::send_query(&SocketMessage::State)?;
    let state: State = serde_json::from_str(&response)?;
    workspaces_from_state(state)
}

pub fn change_workspace(idx: usize) -> anyhow::Result<()> {
    komorebi_client::send_query(&SocketMessage::FocusWorkspaceNumber(idx))?;
    Ok(())
}

const SOCK_NAME: &str = "komorebi-switcher.sock";

pub fn listen_for_workspaces(proxy: EventLoopProxy<AppMessage>) {
    let socket = loop {
        match komorebi_client::subscribe(SOCK_NAME) {
            Ok(socket) => break socket,
            Err(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
        };
    };

    for incoming in socket.incoming() {
        let Ok(data) = incoming else { continue };

        let reader = BufReader::new(data);

        for line in reader.lines().flatten() {
            let Ok(notification) = serde_json::from_str::<Notification>(&line) else {
                continue;
            };

            match notification.event {
                NotificationEvent::Socket(message) if should_update(&message) => {
                    if let Ok(new_workspaces) = workspaces_from_state(notification.state) {
                        let _ = proxy.send_event(AppMessage::UpdateWorkspaces(new_workspaces));
                    }
                }
                _ => {}
            }
        }
    }
}

fn should_update(message: &SocketMessage) -> bool {
    matches!(
        message,
        SocketMessage::FocusLastWorkspace
            | SocketMessage::FocusMonitorNumber(_)
            | SocketMessage::FocusMonitorWorkspaceNumber(_, _)
            | SocketMessage::FocusNamedWorkspace(_)
            | SocketMessage::FocusWorkspaceNumber(_)
            | SocketMessage::FocusWorkspaceNumbers(_)
            | SocketMessage::CycleFocusMonitor(_)
            | SocketMessage::CycleFocusWorkspace(_)
            | SocketMessage::ReloadConfiguration
            | SocketMessage::ReplaceConfiguration(_)
            | SocketMessage::CompleteConfiguration
            | SocketMessage::ReloadStaticConfiguration(_)
            | SocketMessage::MoveContainerToMonitorNumber(_)
            | SocketMessage::MoveContainerToMonitorWorkspaceNumber(_, _)
            | SocketMessage::MoveContainerToNamedWorkspace(_)
            | SocketMessage::MoveContainerToWorkspaceNumber(_)
            | SocketMessage::MoveWorkspaceToMonitorNumber(_)
            | SocketMessage::CycleMoveContainerToMonitor(_)
            | SocketMessage::CycleMoveContainerToWorkspace(_)
            | SocketMessage::CycleMoveWorkspaceToMonitor(_)
            | SocketMessage::CloseWorkspace
            | SocketMessage::SendContainerToMonitorNumber(_)
            | SocketMessage::SendContainerToMonitorWorkspaceNumber(_, _)
            | SocketMessage::SendContainerToNamedWorkspace(_)
            | SocketMessage::SendContainerToWorkspaceNumber(_)
            | SocketMessage::CycleSendContainerToMonitor(_)
            | SocketMessage::CycleSendContainerToWorkspace(_)
    )
}
