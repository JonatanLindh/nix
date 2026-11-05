//! Hyprland over its IPC sockets: monitors and workspaces, kept current from the event socket.
//!
//! Own client instead of the `hyprland` crate: this Hyprland runs a Lua config, which rejects the
//! hyprlang-style dispatches that crate sends (`dispatch workspace 2` is a Lua syntax error).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use futures::SinkExt;
use futures::channel::mpsc::Sender;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::Result;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub monitors: Vec<Monitor>,
    pub workspaces: Vec<Workspace>,
    /// Workspaces with a window asking for attention, until they are next focused.
    pub urgent: BTreeSet<i32>,
}

/// What the bar needs of a monitor; where monitors are and how big comes from Wayland.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub name: String,
    #[serde(deserialize_with = "workspace_id")]
    pub active_workspace: i32,
    pub focused: bool,
    /// Space reserved at each edge by exclusive zones (bars): left, top, right, bottom.
    pub reserved: [i32; 4],
}

fn workspace_id<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<i32, D::Error> {
    Ok(WorkspaceRef::deserialize(d)?.id)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Workspace {
    pub id: i32,
    pub name: String,
    pub monitor: String,
    pub windows: u32,
}

pub fn watch() -> futures::channel::mpsc::Receiver<State> {
    crate::runtime::stream(8, async |out| {
        super::supervise("hyprland", Duration::from_secs(2), out, run).await
    })
}

/// Focuses workspace `id` (it need not exist yet). By number, in the Lua config's syntax.
pub async fn focus_workspace(id: i32) {
    dispatch(&format!("hl.dsp.focus({{ workspace = {id} }})")).await;
}

/// Pastes the clipboard into the focused window, as `qs-paste` (scripts/ in ~/nix) did. Wayland
/// has no "paste this" request, so this sends the app's own paste shortcut: Ctrl+Shift+V for
/// terminals, Ctrl+V for everything else. Waits a moment first for focus to come back from the
/// window that just closed.
pub async fn paste() {
    #[derive(Deserialize)]
    struct Active {
        #[serde(default)]
        class: String,
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    // `{}` when nothing has focus, so no class.
    let class = query::<Active>("j/activewindow")
        .await
        .map(|a| a.class)
        .unwrap_or_default();
    let shortcut = format!(
        "hl.dsp.send_shortcut({{ mods = '{}', key = 'V' }})",
        paste_mods(&class)
    );
    dispatch(&shortcut).await;
}

/// The modifiers of the paste shortcut in a window of `class`.
fn paste_mods(class: &str) -> &'static str {
    const TERMINALS: &[&str] = &[
        "ghostty",
        "kitty",
        "alacritty",
        "Alacritty",
        "foot",
        "wezterm",
        "konsole",
        "terminal",
        "Terminal",
    ];
    if TERMINALS.iter().any(|t| class.contains(t)) {
        "CTRL SHIFT"
    } else {
        "CTRL"
    }
}

async fn dispatch(lua: &str) {
    match request(&format!("dispatch {lua}")).await {
        Ok(reply) if reply.trim() == "ok" => {}
        Ok(reply) => eprintln!("hyprland: dispatch {lua}: {}", reply.trim()),
        Err(e) => eprintln!("hyprland: dispatch {lua}: {e}"),
    }
}

async fn run(mut out: Sender<State>) -> Result {
    let events = UnixStream::connect(socket_dir()?.join(".socket2.sock")).await?;
    let mut lines = BufReader::new(events).lines();
    let mut state = State::default();
    refresh(&mut state).await?;
    out.send(state.clone()).await?;

    loop {
        let line = lines.next_line().await?.ok_or("event socket closed")?;
        let mut dirty = handle(&mut state, &line).await?;
        // Events come in bursts (a workspace switch is several); refresh once per burst.
        while let Ok(next) =
            tokio::time::timeout(Duration::from_millis(15), lines.next_line()).await
        {
            let line = next?.ok_or("event socket closed")?;
            dirty |= handle(&mut state, &line).await?;
        }
        if dirty {
            refresh(&mut state).await?;
            out.send(state.clone()).await?;
        }
    }
}

/// Applies one event line; returns whether monitors or workspaces need a refresh.
async fn handle(state: &mut State, line: &str) -> Result<bool> {
    let (event, data) = line.split_once(">>").unwrap_or((line, ""));
    Ok(match event {
        "urgent" => {
            let address = format!("0x{data}");
            let clients: Vec<Client> = query("j/clients").await?;
            if let Some(c) = clients.iter().find(|c| c.address == address) {
                state.urgent.insert(c.workspace.id);
            }
            true
        }
        "workspacev2" => {
            if let Some(id) = data.split(',').next().and_then(|id| id.parse().ok()) {
                state.urgent.remove(&id);
            }
            true
        }
        "workspace" | "focusedmon" | "focusedmonv2" | "createworkspace" | "createworkspacev2"
        | "destroyworkspace" | "destroyworkspacev2" | "moveworkspace" | "moveworkspacev2"
        | "renameworkspace" | "openwindow" | "closewindow" | "movewindow" | "movewindowv2"
        | "monitoradded" | "monitoraddedv2" | "monitorremoved" | "monitorremovedv2"
        // A bar coming or going changes a monitor's reserved space.
        | "openlayer" | "closelayer" => true,
        _ => false,
    })
}

async fn refresh(state: &mut State) -> Result {
    state.monitors = query("j/monitors").await?;
    state.workspaces = query("j/workspaces").await?;
    Ok(())
}

#[derive(Deserialize)]
struct WorkspaceRef {
    id: i32,
}

#[derive(Deserialize)]
struct Client {
    address: String,
    workspace: WorkspaceRef,
}

fn socket_dir() -> Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR unset")?;
    let instance =
        std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").ok_or("not running under Hyprland")?;
    Ok(PathBuf::from(runtime).join("hypr").join(instance))
}

async fn request(command: &str) -> Result<String> {
    let mut socket = UnixStream::connect(socket_dir()?.join(".socket.sock")).await?;
    socket.write_all(command.as_bytes()).await?;
    let mut reply = String::new();
    socket.read_to_string(&mut reply).await?;
    Ok(reply)
}

pub async fn query<T: DeserializeOwned>(command: &str) -> Result<T> {
    Ok(serde_json::from_str(&request(command).await?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_shortcut() {
        assert_eq!(paste_mods("com.mitchellh.ghostty"), "CTRL SHIFT");
        assert_eq!(paste_mods("Alacritty"), "CTRL SHIFT");
        assert_eq!(paste_mods("org.gnome.Terminal"), "CTRL SHIFT");
        assert_eq!(paste_mods("firefox"), "CTRL");
        assert_eq!(paste_mods(""), "CTRL");
    }
}
