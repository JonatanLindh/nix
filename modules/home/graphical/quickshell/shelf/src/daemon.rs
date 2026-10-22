//! The org.jonatanlindh.Shelf session-bus service. It owns the shelf's items and runs the
//! tools on them. Results are JSON strings: the only client is the CLI, which prints them
//! for Quickshell as is.

use std::{path::PathBuf, time::Duration};

use futures_util::StreamExt;
use serde_json::json;
use tokio::signal::unix::{SignalKind, signal};
use zbus::{interface, object_server::SignalEmitter};

use crate::{
    actions, hand,
    jobs::Jobs,
    store::Store,
    tools::{self, Outcome},
};

pub const SERVICE_NAME: &str = "org.jonatanlindh.Shelf";
pub const OBJECT_PATH: &str = "/org/jonatanlindh/Shelf";

pub struct Shelf {
    jobs: Jobs,
    store: Store,
}

impl Shelf {
    async fn changed(&self, emitter: &SignalEmitter<'_>, changed: bool, reason: &str) {
        if !changed {
            return;
        }
        let items = json!(self.store.items()).to_string();
        if let Err(e) = Self::items_changed(emitter, &items, reason).await {
            eprintln!("can't emit ItemsChanged: {e}");
        }
    }
}

#[interface(
    name = "org.jonatanlindh.Shelf",
    proxy(
        default_path = "/org/jonatanlindh/Shelf",
        default_service = "org.jonatanlindh.Shelf"
    )
)]
impl Shelf {
    /// The actions that apply to every one of `files`, as a JSON array.
    async fn actions(&self, files: Vec<String>) -> String {
        json!(actions::for_files(&files)).to_string()
    }

    /// Runs `action` (with its chosen `option`, or "") on `files`; a JSON outcome
    /// `{ok, message, outputs, copy?, job?}`. Failures are outcomes too, not D-Bus errors.
    /// Results that are files go onto the shelf, on top and marked new.
    async fn run(
        &self,
        action: &str,
        option: &str,
        files: Vec<String>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        if action == "hand" {
            let outcome = hand::hand_over(&self.store, &files)
                .await
                .unwrap_or_else(|e| Outcome::failed(format!("{e:#}")));
            self.changed(&emitter, !outcome.select.is_empty(), "hand")
                .await;
            return json!(outcome).to_string();
        }
        let result = match action {
            "funnel" => self.jobs.funnel(&files).await,
            "wormhole" => self.jobs.wormhole(&files).await,
            _ => tools::run(action, option, &files).await,
        };
        let outcome = result.unwrap_or_else(|e| Outcome::failed(format!("{e:#}")));
        let added = self.store.add_results(&outcome.outputs);
        self.changed(&emitter, added, "result").await;
        serde_json::to_string(&outcome)
            .unwrap_or_else(|e| json!({ "ok": false, "message": e.to_string() }).to_string())
    }

    /// The shelf's items, as a JSON array of `{path, name, kind, temp}`.
    async fn items(&self) -> String {
        json!(self.store.items()).to_string()
    }

    /// Adds absolute `paths` to the end of the shelf, skipping ones already on it.
    async fn add(&self, paths: Vec<String>, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let changed = self.store.add(&paths);
        self.changed(&emitter, changed, "add").await;
    }

    async fn remove(&self, paths: Vec<String>, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let changed = self.store.remove(&paths);
        self.changed(&emitter, changed, "remove").await;
    }

    async fn clear(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let changed = self.store.clear();
        self.changed(&emitter, changed, "clear").await;
    }

    /// Stops a running share.
    async fn cancel(&self, job: &str) -> zbus::fdo::Result<()> {
        self.jobs
            .cancel(job)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("{e:#}")))
    }

    /// The running shares, as a JSON array of jobs.
    async fn jobs(&self) -> String {
        json!(self.jobs.list()).to_string()
    }

    /// A share changed: state is "sending", "done", "failed", "cancelled" or "stopped".
    #[zbus(signal)]
    pub async fn job_changed(
        emitter: &SignalEmitter<'_>,
        job: &str,
        state: &str,
        message: &str,
    ) -> zbus::Result<()>;

    /// The items changed (all of them, as in `Items`); reason is "add", "remove", "clear",
    /// "result", "hand" (handed to the shelf) or "gone" (files that went away).
    #[zbus(signal)]
    pub async fn items_changed(
        emitter: &SignalEmitter<'_>,
        items: &str,
        reason: &str,
    ) -> zbus::Result<()>;
}

pub async fn serve() -> anyhow::Result<()> {
    let conn = zbus::connection::Builder::session()?.build().await?;
    let jobs = Jobs::new(conn.clone());
    let (store, events) = Store::load();
    conn.object_server()
        .at(
            OBJECT_PATH,
            Shelf {
                jobs: jobs.clone(),
                store: store.clone(),
            },
        )
        .await?;
    // Items whose file went away (dragged out by a file manager that moves) leave the shelf.
    if let Some(mut events) = events {
        let emitter = SignalEmitter::new(&conn, OBJECT_PATH)?.into_owned();
        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let event = match event {
                    Ok(event) => event,
                    Err(e) => {
                        eprintln!("watching files stopped: {e}");
                        return;
                    }
                };
                let paths = store.watcher_paths(&event);
                if store.check(&paths) {
                    let items = json!(store.items()).to_string();
                    if let Err(e) = Shelf::items_changed(&emitter, &items, "gone").await {
                        eprintln!("can't emit ItemsChanged: {e}");
                    }
                }
            }
        });
    }
    // Claim the name only once the object is there, so no call arrives to nothing.
    conn.request_name(SERVICE_NAME).await?;
    eprintln!("serving {SERVICE_NAME}");

    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
    jobs.cancel_all().await;
    Ok(())
}

/// A client connection to the daemon. Calls may take minutes (re-encoding a video), far
/// past D-Bus's default 25 s timeout.
pub async fn proxy() -> anyhow::Result<ShelfProxy<'static>> {
    let conn = zbus::connection::Builder::session()?
        .method_timeout(Duration::from_secs(60 * 60))
        .build()
        .await?;
    Ok(ShelfProxy::new(&conn).await?)
}
