use std::io::{BufRead, BufReader, Read, Write};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use uds_windows::{UnixListener, UnixStream};
use winit::event_loop::EventLoopProxy;

use crate::app::AppMessage;

#[derive(Debug, Deserialize)]
struct KWorkspace {
    name: Option<String>,
    containers: Ring<serde_json::Value>,
    maximized_window: Option<serde_json::Value>,
    monocle_container: Option<serde_json::Value>,
    floating_windows: Vec<serde_json::Value>,
}

impl KWorkspace {
    fn is_empty(&self) -> bool {
        self.containers.is_empty()
            && self.maximized_window.is_none()
            && self.monocle_container.is_none()
            && self.floating_windows.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct Ring<T> {
    elements: Vec<T>,
    focused: usize,
}

impl<T> Ring<T> {
    fn focused(&self) -> Option<&T> {
        self.elements.get(self.focused)
    }

    fn focused_idx(&self) -> usize {
        self.focused
    }

    fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    fn iter(&self) -> std::slice::Iter<T> {
        self.elements.iter()
    }
}

#[derive(Debug, Deserialize)]
struct KMonitor {
    workspaces: Ring<KWorkspace>,
}

#[derive(Debug, Deserialize)]
struct KState {
    monitors: Ring<KMonitor>,
}

#[derive(Debug, strum::Display, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
enum KSocketMessage {
    State,
    AddSubscriberSocket(String),
    FocusWorkspaceNumber(usize),
}

#[derive(Debug, strum::Display, Serialize, Deserialize)]
#[serde(tag = "type")]
enum KSocketEvent {
    FocusWorkspaceNumber,
    FocusMonitorNumber,
    FocusMonitorWorkspaceNumber,
    FocusNamedWorkspace,
    FocusWorkspaceNumbers,
    CycleFocusMonitor,
    CycleFocusWorkspace,
    ReloadConfiguration,
    ReplaceConfiguration,
    CompleteConfiguration,
    ReloadStaticConfiguration,
    MoveContainerToMonitorNumber,
    MoveContainerToMonitorWorkspaceNumber,
    MoveContainerToNamedWorkspace,
    MoveContainerToWorkspaceNumber,
    MoveWorkspaceToMonitorNumber,
    CycleMoveContainerToMonitor,
    CycleMoveContainerToWorkspace,
    CycleMoveWorkspaceToMonitor,
    CloseWorkspace,
    SendContainerToMonitorNumber,
    SendContainerToMonitorWorkspaceNumber,
    SendContainerToNamedWorkspace,
    SendContainerToWorkspaceNumber,
    CycleSendContainerToMonitor,
    CycleSendContainerToWorkspace,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum KNotificationEvent {
    #[allow(unused)]
    Socket(KSocketEvent),
}

#[derive(Debug, Deserialize)]
struct KNotification {
    #[allow(unused)]
    event: KNotificationEvent,
    state: KState,
}

const KOMOREBI_SOCK: &str = "komorebi.sock";

fn send_message(message: &KSocketMessage) -> anyhow::Result<()> {
    let socket = dirs::data_local_dir()
        .context("there is no local data directory")?
        .join("komorebi")
        .join(KOMOREBI_SOCK);

    let mut stream = UnixStream::connect(socket)?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
    stream.write_all(serde_json::to_string(message)?.as_bytes())?;

    Ok(())
}

fn send_query(message: &KSocketMessage) -> anyhow::Result<String> {
    let socket = dirs::data_local_dir()
        .context("there is no local data directory")?
        .join("komorebi")
        .join(KOMOREBI_SOCK);

    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
    stream.write_all(serde_json::to_string(message)?.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_to_string(&mut response)?;

    Ok(response)
}

fn subscribe(name: &str) -> anyhow::Result<UnixListener> {
    let socket = dirs::data_local_dir()
        .context("there is no local data directory")?
        .join("komorebi")
        .join(name);

    match std::fs::remove_file(&socket) {
        Ok(()) => {}
        Err(error) => match error.kind() {
            std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(error.into());
            }
        },
    };

    let listener = UnixListener::bind(&socket)?;

    send_message(&KSocketMessage::AddSubscriberSocket(name.to_string()))?;

    Ok(listener)
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub idx: usize,
    pub focused: bool,
    pub is_empty: bool,
}

fn workspaces_from_state(state: KState) -> anyhow::Result<Vec<Workspace>> {
    let monitor = state.monitors.focused().context("No focused monintor?")?;

    let focused_workspace = monitor.workspaces.focused_idx();

    let workspaces = monitor.workspaces.iter().enumerate().map(|(idx, w)| {
        let name = w.name.clone().unwrap_or_else(|| (idx + 1).to_string());

        Workspace {
            name,
            idx,
            focused: focused_workspace == idx,
            is_empty: w.is_empty(),
        }
    });

    Ok(workspaces.collect())
}

pub fn read_workspaces() -> anyhow::Result<Vec<Workspace>> {
    log::info!("Reading komorebi workspaces");

    let response = send_query(&KSocketMessage::State)?;
    let state: KState = serde_json::from_str(&response)?;
    workspaces_from_state(state)
}

pub fn change_workspace(idx: usize) {
    log::info!("Changing komorebi workspace to {idx}");

    if let Err(e) = send_message(&KSocketMessage::FocusWorkspaceNumber(idx)) {
        log::error!("Failed to change workspace: {e}")
    }
}

const SOCK_NAME: &str = "komorebi-switcher.sock";

pub fn listen_for_workspaces(proxy: EventLoopProxy<AppMessage>) {
    let socket = loop {
        match subscribe(SOCK_NAME) {
            Ok(socket) => break socket,
            Err(_) => std::thread::sleep(std::time::Duration::from_secs(1)),
        };
    };

    log::info!("Listenting for messages from komorebi");

    for incoming in socket.incoming().map_while(Result::ok) {
        log::debug!("Received a message from komorebi");

        let reader = BufReader::new(incoming);

        for line in reader.lines().map_while(Result::ok) {
            log::trace!("Reading line from komorebi message: {line}");

            let Ok(notification) = serde_json::from_str::<KNotification>(&line) else {
                continue;
            };

            let new_workspaces = match workspaces_from_state(notification.state) {
                Ok(new_workspaces) => new_workspaces,
                Err(e) => {
                    log::error!("Failed to read workspaces from state: {e}");
                    continue;
                }
            };

            if let Err(e) = proxy.send_event(AppMessage::UpdateWorkspaces(new_workspaces)) {
                log::error!("Failed to send `AppMessage::UpdateWorkspaces`: {e}")
            }
        }
    }
}
