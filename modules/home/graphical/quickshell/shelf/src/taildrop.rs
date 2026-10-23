//! Receiving Taildrop: files sent to this device land on top of the shelf as its own
//! (temp). `tailscale file get --wait` long-polls tailscaled until something arrives, so the
//! wait costs nothing. Each batch goes into an inbox next to the results dir and is renamed
//! in from there (same disk), so a half-received batch never shows up on the shelf.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::json;
use zbus::object_server::SignalEmitter;

use crate::{
    daemon::Shelf,
    files::{name, results_dir, state_dir, unique},
    store::Store,
    tools::exec,
};

/// Retry delays while tailscaled is down, logged out or missing.
const FIRST_RETRY: Duration = Duration::from_secs(5);
const LAST_RETRY: Duration = Duration::from_secs(5 * 60);

pub async fn receive(store: Store, emitter: SignalEmitter<'static>) {
    let inbox = match inbox() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("not receiving Taildrop: {e:#}");
            return;
        }
    };
    let mut retry = FIRST_RETRY;
    loop {
        // Also picks up what a crash left in the inbox last time.
        shelve(&store, &emitter, &inbox).await;
        let got = exec(
            "tailscale",
            [
                "file".as_ref(),
                "get".as_ref(),
                "--wait".as_ref(),
                "--conflict=rename".as_ref(),
                inbox.as_os_str(),
            ],
        )
        .await;
        match got {
            Ok(_) => retry = FIRST_RETRY,
            Err(e) => {
                eprintln!("Taildrop: {e:#}; retrying in {}s", retry.as_secs());
                tokio::time::sleep(retry).await;
                retry = (retry * 2).min(LAST_RETRY);
            }
        }
    }
}

fn inbox() -> anyhow::Result<PathBuf> {
    let dir = state_dir()?.join("taildrop");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Moves everything in the inbox into the results dir and onto the shelf.
async fn shelve(store: &Store, emitter: &SignalEmitter<'_>, inbox: &Path) {
    let moved = match move_in(inbox) {
        Ok(moved) => moved,
        Err(e) => {
            eprintln!("can't shelve received files: {e:#}");
            return;
        }
    };
    if !store.add_results(&moved) {
        return;
    }
    let items = json!(store.items()).to_string();
    if let Err(e) = Shelf::items_changed(emitter, &items, "taildrop").await {
        eprintln!("can't emit ItemsChanged: {e}");
    }
}

fn move_in(inbox: &Path) -> anyhow::Result<Vec<String>> {
    let dir = results_dir()?;
    let mut moved = Vec::new();
    let entries = match std::fs::read_dir(inbox) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(moved),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let from = entry.path();
        let to = unique(&dir, &name(&from));
        match std::fs::rename(&from, &to) {
            Ok(()) => moved.push(to.to_string_lossy().into_owned()),
            Err(e) => eprintln!("can't move {} in: {e}", from.display()),
        }
    }
    Ok(moved)
}

