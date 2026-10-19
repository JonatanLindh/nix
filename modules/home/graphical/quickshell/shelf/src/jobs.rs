//! Shares that outlive the call that started them: a Tailscale Funnel link (until stopped)
//! and a magic-wormhole send (until received or cancelled). Their changes go out as the
//! daemon's JobChanged signal.

use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, bail};
use serde::Serialize;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::{mpsc, oneshot},
};
use zbus::{Connection, object_server::SignalEmitter};

use crate::{
    daemon::{OBJECT_PATH, Shelf},
    files::{name, plural, runtime_dir, token},
    http,
    qr::{self, Qr},
    tools::{Outcome, exec, output, zip_to},
};

#[derive(Serialize, Clone, Debug)]
pub struct JobInfo {
    pub id: String,
    /// "funnel" or "wormhole".
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub qr: Qr,
    pub files: Vec<String>,
}

struct Job {
    info: JobInfo,
    /// Funnel: the serve path to turn off.
    serve_path: Option<String>,
    stop: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub struct Jobs {
    conn: Connection,
    jobs: Arc<Mutex<HashMap<String, Job>>>,
}

impl Jobs {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn,
            jobs: Arc::default(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
        // A panic while holding the lock leaves the map itself intact; keep going.
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<JobInfo> {
        self.lock().values().map(|j| j.info.clone()).collect()
    }

    async fn emit(&self, job: &str, state: &str, message: &str) {
        let sent = match SignalEmitter::new(&self.conn, OBJECT_PATH) {
            Ok(emitter) => Shelf::job_changed(&emitter, job, state, message).await,
            Err(e) => Err(e),
        };
        if let Err(e) = sent {
            eprintln!("can't emit JobChanged: {e}");
        }
    }

    pub async fn cancel(&self, id: &str) -> anyhow::Result<()> {
        let job = self
            .lock()
            .remove(id)
            .with_context(|| format!("No running share “{id}”"))?;
        if let Some(stop) = job.stop {
            // Wormhole: its task kills the process, cleans up and emits.
            // Funnel: stops the local file server.
            let _ = stop.send(());
        }
        if let Some(path) = job.serve_path {
            let result = funnel_off(&path).await;
            remove_scratch(id);
            self.emit(id, "stopped", "Stopped sharing").await;
            result?;
        }
        Ok(())
    }

    /// Stops everything; on shutdown, so nothing stays public with no one to stop it.
    pub async fn cancel_all(&self) {
        let ids: Vec<String> = self.lock().keys().cloned().collect();
        for id in ids {
            if let Err(e) = self.cancel(&id).await {
                eprintln!("stopping {id}: {e:#}");
            }
        }
    }

    pub async fn funnel(&self, files: &[PathBuf]) -> anyhow::Result<Outcome> {
        let id = token();
        // One file per link, so a folder goes out zipped like several files do.
        let target = share_target(files, &id, true).await?;
        let started = async {
            let host = dns_name().await?;
            let (stop, stopped) = oneshot::channel();
            let port = http::serve(target.clone(), stopped).await?;
            // The link ends in the file name so downloads get a sensible one.
            let mount = format!("/shelf/{id}");
            let set_path = format!("--set-path={mount}");
            let backend = format!("http://127.0.0.1:{port}");
            exec(
                "tailscale",
                [
                    "funnel",
                    "--bg",
                    "--yes",
                    "--https=443",
                    &set_path,
                    &backend,
                ],
            )
            .await?;
            let url = format!("https://{host}{mount}/{}", url_safe(&name(&target)));
            anyhow::Ok((url, mount, stop))
        }
        .await;
        let (url, mount, stop) = match started {
            Ok(started) => started,
            Err(e) => {
                remove_scratch(&id);
                return Err(e);
            }
        };
        let info = JobInfo {
            id: id.clone(),
            kind: "funnel",
            qr: qr::encode(&url)?,
            url: Some(url),
            code: None,
            files: paths(files),
        };
        self.lock().insert(
            id,
            Job {
                info: info.clone(),
                serve_path: Some(mount),
                stop: Some(stop),
            },
        );
        Ok(Outcome {
            job: Some(info),
            ..Outcome::said(format!("Sharing {} publicly", plural(files.len(), "file")))
        })
    }

    pub async fn wormhole(&self, files: &[PathBuf]) -> anyhow::Result<Outcome> {
        let id = token();
        let target = share_target(files, &id, false).await?;
        let mut child = match Command::new("wormhole")
            .arg("send")
            .arg(&target)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                remove_scratch(&id);
                if e.kind() == std::io::ErrorKind::NotFound {
                    bail!("wormhole isn't installed");
                }
                bail!("Couldn't start wormhole: {e}");
            }
        };
        let (tx, mut lines) = mpsc::unbounded_channel();
        if let Some(out) = child.stdout.take() {
            tokio::spawn(pump(out, tx.clone()));
        }
        if let Some(err) = child.stderr.take() {
            tokio::spawn(pump(err, tx));
        }

        // The code arrives once wormhole has reached the mailbox server.
        let mut last = String::new();
        let code = tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(line) = lines.recv().await {
                if let Some((_, code)) = line.split_once("Wormhole code is:") {
                    return Some(code.trim().to_string());
                }
                if !line.trim().is_empty() {
                    last = line;
                }
            }
            None
        })
        .await;
        let code = match code {
            Ok(Some(code)) => code,
            Ok(None) => {
                remove_scratch(&id);
                bail!("wormhole failed: {}", last.trim());
            }
            Err(_) => {
                remove_scratch(&id);
                bail!("wormhole didn't get a code (offline?)");
            }
        };

        let (stop, stopped) = oneshot::channel();
        let info = JobInfo {
            id: id.clone(),
            kind: "wormhole",
            qr: qr::encode(&code)?,
            url: None,
            code: Some(code),
            files: paths(files),
        };
        self.lock().insert(
            id.clone(),
            Job {
                info: info.clone(),
                serve_path: None,
                stop: Some(stop),
            },
        );
        tokio::spawn(self.clone().supervise_wormhole(id, child, lines, stopped));
        Ok(Outcome {
            job: Some(info),
            ..Outcome::said("Waiting for the other side…")
        })
    }

    async fn supervise_wormhole(
        self,
        id: String,
        mut child: Child,
        mut lines: mpsc::UnboundedReceiver<String>,
        mut stopped: oneshot::Receiver<()>,
    ) {
        let mut sending = false;
        let mut last = String::new();
        let (state, message) = loop {
            tokio::select! {
                Some(line) = lines.recv() => {
                    if !sending && line.starts_with("Sending") {
                        sending = true;
                        self.emit(&id, "sending", "Sending…").await;
                    }
                    if !line.trim().is_empty() {
                        last = line;
                    }
                }
                status = child.wait() => break match status {
                    Ok(s) if s.success() => ("done", "Sent with wormhole".to_string()),
                    _ => ("failed", format!("Wormhole failed: {}", last.trim())),
                },
                _ = &mut stopped => {
                    let _ = child.kill().await;
                    break ("cancelled", "Wormhole cancelled".to_string());
                }
            }
        };
        self.lock().remove(&id);
        remove_scratch(&id);
        self.emit(&id, state, &message).await;
    }
}

/// What to share as one thing: a single file or folder as is, several zipped together.
async fn share_target(files: &[PathBuf], id: &str, zip_folder: bool) -> anyhow::Result<PathBuf> {
    match files {
        [] => bail!("No files"),
        [one] if !(zip_folder && one.is_dir()) => Ok(one.clone()),
        [one] => {
            let dir = runtime_dir()?.join(id);
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Can't create {}", dir.display()))?;
            let out = dir.join(format!("{}.zip", name(one)));
            zip_to(files, &out).await?;
            Ok(out)
        }
        _ => {
            let dir = runtime_dir()?.join(id);
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Can't create {}", dir.display()))?;
            let out = dir.join(format!("shelf-{}-files.zip", files.len()));
            zip_to(files, &out).await?;
            Ok(out)
        }
    }
}

fn remove_scratch(id: &str) {
    if let Ok(dir) = runtime_dir() {
        let _ = std::fs::remove_dir_all(dir.join(id));
    }
}

async fn funnel_off(path: &str) -> anyhow::Result<()> {
    let arg = format!("--set-path={path}");
    exec("tailscale", ["funnel", "--yes", "--https=443", &arg, "off"])
        .await
        .map(|_| ())
}

/// This machine's MagicDNS name, e.g. "xps.tail-net.ts.net".
async fn dns_name() -> anyhow::Result<String> {
    let status: serde_json::Value =
        serde_json::from_str(&output("tailscale", ["status", "--json"]).await?)?;
    status["Self"]["DNSName"]
        .as_str()
        .map(|s| s.trim_end_matches('.').to_string())
        .filter(|s| !s.is_empty())
        .context("Tailscale isn't connected")
}

/// A file name as a URL path segment: spaces and the like become dashes.
fn url_safe(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    if safe.trim_matches(['.', '-']).is_empty() {
        "file".into()
    } else {
        safe
    }
}

fn paths(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .map(|f| f.to_string_lossy().into_owned())
        .collect()
}

/// Forwards a process's output line by line, splitting on \r too (progress bars).
async fn pump(mut reader: impl AsyncRead + Unpin, tx: mpsc::UnboundedSender<String>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Ok(n) = reader.read(&mut chunk).await {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        while let Some(i) = buf.iter().position(|&b| b == b'\n' || b == b'\r') {
            let line: Vec<u8> = buf.drain(..=i).collect();
            if tx
                .send(String::from_utf8_lossy(&line[..i]).into_owned())
                .is_err()
            {
                return;
            }
        }
    }
    if !buf.is_empty() {
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn url_safe_names() {
        assert_eq!(super::url_safe("my file (1).pdf"), "my-file--1-.pdf");
        assert_eq!(super::url_safe("åäö"), "file");
    }
}
