//! The drag shelf's backend: it holds the items (saved in $XDG_STATE_HOME/shelf/items.json) and
//! does the work on them: converting and packing (`tools`), sharing (Tailscale Funnel links and
//! magic-wormhole sends in `jobs`, Taildrop both ways), and noticing files that go away
//! (`watch`). The UI (`crate::shelf`) calls in here and follows [`watch`].
//!
//! It used to be a daemon of its own (`shelf daemon`, org.jonatanlindh.Shelf on the session
//! bus, in ~/nix); it runs in the shell now, so the calls are plain calls and the changes come
//! through a channel instead of D-Bus signals. Shares stop when the shell does ([`shutdown`]).

mod actions;
mod clip;
mod files;
mod hand;
mod http;
mod jobs;
mod peers;
mod qr;
mod store;
mod taildrop;
mod tools;
mod watch;

use std::path::PathBuf;
use std::sync::LazyLock;

use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

pub use actions::{Action, Options};
pub use jobs::JobInfo as Job;
pub use peers::Peer;
pub use qr::Qr;
pub use store::Item;
pub use tools::{LinkQr, Outcome};

use jobs::Jobs;
use store::Store;

#[derive(Debug, Clone)]
pub enum Event {
    /// The whole item list, and why it changed: "load", "add", "remove", "clear", "result",
    /// "hand" (handed to the shelf), "taildrop" (received) or "gone" (files that went away).
    Items { items: Vec<Item>, reason: String },
    /// A share changed: "sending", "done", "failed", "cancelled" or "stopped".
    Job {
        job: String,
        state: String,
        message: String,
    },
}

struct Shelf {
    store: Store,
    jobs: Jobs,
}

/// Changes, for whoever [`watch`]es.
static EVENTS: LazyLock<broadcast::Sender<Event>> = LazyLock::new(|| broadcast::channel(64).0);

/// Loaded on first use, which is on the shell's runtime ([`start`], or any call here): the file
/// watcher and the Taildrop receiver need it.
static SHELF: LazyLock<Shelf> = LazyLock::new(|| {
    let (store, events) = Store::load();
    // Files sent to this device by Taildrop go on the shelf.
    tokio::spawn(taildrop::receive(store.clone()));
    // Items whose file went away (dragged out by a file manager that moves) leave the shelf.
    if let Some(mut events) = events {
        let store = store.clone();
        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let event = match event {
                    Ok(event) => event,
                    Err(e) => {
                        eprintln!("shelf: watching files stopped: {e}");
                        return;
                    }
                };
                if store.check(&store.watcher_paths(&event)) {
                    items_changed(&store, "gone");
                }
            }
        });
    }
    Shelf {
        store,
        jobs: Jobs::default(),
    }
});

fn shelf() -> &'static Shelf {
    &SHELF
}

/// Loads the shelf and starts receiving Taildrop, without waiting for the UI to ask.
pub fn start() {
    crate::runtime::detach(async {
        shelf();
    });
}

/// Stops the running shares, so no link stays public with no one to stop it. For the shell's
/// exit.
pub async fn shutdown() {
    if let Some(shelf) = LazyLock::get(&SHELF) {
        shelf.jobs.cancel_all().await;
    }
}

fn items_changed(store: &Store, reason: &str) {
    let _ = EVENTS.send(Event::Items {
        items: store.items(),
        reason: reason.into(),
    });
}

fn job_changed(job: &str, state: &str, message: &str) {
    let _ = EVENTS.send(Event::Job {
        job: job.into(),
        state: state.into(),
        message: message.into(),
    });
}

/// The items now, then every change.
pub fn watch() -> futures::channel::mpsc::Receiver<Event> {
    crate::runtime::stream(8, async |mut out| {
        // Subscribed before reading the items, so no change falls in between.
        let mut changes = EVENTS.subscribe();
        let store = &shelf().store;
        let mut next = Some(Event::Items {
            items: store.items(),
            reason: "load".into(),
        });
        while let Some(event) = next.take() {
            if out.send(event).await.is_err() {
                return;
            }
            next = match changes.recv().await {
                Ok(event) => Some(event),
                // Missed some: the items as they are now cover them (a missed share change
                // shows when the share is next looked at).
                Err(broadcast::error::RecvError::Lagged(_)) => Some(Event::Items {
                    items: store.items(),
                    reason: "load".into(),
                }),
                Err(broadcast::error::RecvError::Closed) => None,
            };
        }
    })
}

/// Adds absolute `paths` to the end of the shelf, skipping ones already on it.
pub async fn add(paths: Vec<String>) {
    let store = &shelf().store;
    if store.add(&paths) {
        items_changed(store, "add");
    }
}

pub async fn remove(path: String) {
    let store = &shelf().store;
    if store.remove(&[path]) {
        items_changed(store, "remove");
    }
}

pub async fn clear() {
    let store = &shelf().store;
    if store.clear() {
        items_changed(store, "clear");
    }
}

/// Stops a running share; its end comes back as an [`Event::Job`].
pub async fn cancel(job: String) {
    if let Err(e) = shelf().jobs.cancel(&job).await {
        eprintln!("shelf: {e:#}");
    }
}

/// Saves clipboard `data` of type `mime` as a new item on top.
fn save(mime: &str, data: &[u8]) -> Outcome {
    let outcome = clip::save(mime, data).unwrap_or_else(|e| Outcome::failed(format!("{e:#}")));
    let store = &shelf().store;
    if store.add_results(&outcome.outputs) {
        items_changed(store, "result");
    }
    outcome
}

/// Puts `text` on the shelf as a new item: a web address becomes a link.
#[allow(dead_code)] // For links dropped in from a browser, once drops can carry them.
pub async fn paste_text(text: String) -> Outcome {
    save("text/plain", text.as_bytes())
}

/// Puts the clipboard on the shelf: copied files go on as they are; a web address, text or an
/// image becomes a new item.
pub async fn paste() -> Outcome {
    let listed = match tools::output("wl-paste", ["--list-types"]).await {
        Ok(listed) => listed,
        Err(_) => return Outcome::failed("The clipboard is empty"),
    };
    let types: Vec<&str> = listed.lines().map(str::trim).collect();
    if types.contains(&"text/uri-list")
        && let Ok(list) =
            tools::output("wl-paste", ["--no-newline", "--type", "text/uri-list"]).await
        && let Some(paths) = clip::file_paths(&list)
    {
        let n = paths.len();
        add(paths).await;
        return Outcome::said(format!("Added {}", files::plural(n, "file")));
    }
    let Some(mime) = clip::pick_type(&types) else {
        return Outcome::failed("Nothing on the clipboard the shelf can hold");
    };
    match tools::exec("wl-paste", ["--no-newline", "--type", &mime]).await {
        Ok(out) => save(&mime, &out.stdout),
        Err(e) => Outcome::failed(format!("{e:#}")),
    }
}

/// Taildrop targets (your own tailnet devices), or why there are none.
pub async fn peers() -> Result<Vec<Peer>, String> {
    peers::list().await.map_err(|e| format!("{e:#}"))
}

/// The tools that apply to every one of `files`.
pub async fn actions(files: Vec<String>) -> Vec<Action> {
    actions::for_files(&files)
}

/// Runs tool `action` (with its chosen `option`, or "") on `files`. Failures are outcomes too.
/// Results that are files go onto the shelf, on top.
pub async fn run_action(action: String, option: String, files: Vec<String>) -> Outcome {
    let shelf = shelf();
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    if action == "hand" {
        let outcome = hand::hand_over(&shelf.store, &files)
            .await
            .unwrap_or_else(|e| Outcome::failed(format!("{e:#}")));
        if !outcome.select.is_empty() {
            items_changed(&shelf.store, "hand");
        }
        return outcome;
    }
    let result = match action.as_str() {
        "funnel" => shelf.jobs.funnel(&files).await,
        "wormhole" => shelf.jobs.wormhole(&files).await,
        _ => tools::run(&action, &option, &files).await,
    };
    let outcome = result.unwrap_or_else(|e| Outcome::failed(format!("{e:#}")));
    if shelf.store.add_results(&outcome.outputs) {
        items_changed(&shelf.store, "result");
    }
    outcome
}
