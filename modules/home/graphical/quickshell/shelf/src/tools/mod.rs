//! One-shot actions: each takes files, runs the right CLI tool and reports what it made.
//! Long-lived shares (funnel, wormhole) live in `crate::jobs`.

mod archive;
mod image;
mod pdf;
mod video;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Output, Stdio},
};

use anyhow::{Context, bail};
use serde::Serialize;
use tokio::process::Command;

pub use archive::zip_to;

use crate::{files::plural, jobs::JobInfo};

/// What an action did, as printed for the UI.
#[derive(Serialize, Debug, Default)]
pub struct Outcome {
    pub ok: bool,
    pub message: String,
    /// New files/folders, to be added to the shelf.
    pub outputs: Vec<String>,
    /// Items the UI should select (when not `outputs`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub select: Vec<String>,
    /// Text the UI should put on the clipboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
    /// A share that keeps running after this call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobInfo>,
}

impl Outcome {
    pub fn made(message: impl Into<String>, outputs: Vec<PathBuf>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            outputs: outputs
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            ..Self::default()
        }
    }

    pub fn said(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            ..Self::default()
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            ..Self::default()
        }
    }
}

pub async fn run(action: &str, opt: &str, files: &[PathBuf]) -> anyhow::Result<Outcome> {
    if files.is_empty() {
        bail!("No files");
    }
    if let Some(missing) = files.iter().find(|f| !f.exists()) {
        bail!("{} no longer exists", crate::files::name(missing));
    }
    match action {
        "convert" => image::convert(files, opt).await,
        "resize" => image::resize(files, opt).await,
        "strip" => image::strip(files).await,
        "ocr" => image::ocr(files).await,
        "merge" => pdf::merge(files).await,
        "compress" => pdf::compress(files).await,
        "split" => pdf::split(files).await,
        "shrink" => video::shrink(files).await,
        "gif" => video::gif(files).await,
        "extract" => archive::extract(files).await,
        "archive" => archive::archive(files).await,
        "open" => open(files),
        "localsend" => localsend(files).await,
        "taildrop" => taildrop(files, opt).await,
        "copy-paths" => Ok(Outcome {
            copy: Some(
                files
                    .iter()
                    .map(|f| f.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            ..Outcome::said(format!("Copied {}", plural(files.len(), "path")))
        }),
        _ => bail!("Unknown action “{action}”"),
    }
}

fn open(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    for f in files {
        let mut cmd = if which("uwsm-app") {
            let mut c = Command::new("uwsm-app");
            c.args(["--", "xdg-open"]);
            c
        } else {
            Command::new("xdg-open")
        };
        let mut child = cmd
            .arg(f)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| not_found("xdg-open", e))?;
        tokio::spawn(async move { child.wait().await });
    }
    Ok(Outcome::said(format!(
        "Opened {}",
        plural(files.len(), "file")
    )))
}

async fn localsend(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    // Through uwsm so the app gets its own scope instead of living (and dying) in ours.
    let mut cmd = if which("uwsm-app") {
        let mut c = Command::new("uwsm-app");
        c.args(["--", "localsend_app"]);
        c
    } else {
        Command::new("localsend_app")
    };
    let mut child = cmd
        .args(files)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| not_found("localsend_app", e))?;
    // Reap it whenever it exits.
    tokio::spawn(async move { child.wait().await });
    Ok(Outcome::said(format!(
        "Opened LocalSend with {}",
        plural(files.len(), "file")
    )))
}

async fn taildrop(files: &[PathBuf], peer: &str) -> anyhow::Result<Outcome> {
    if peer.is_empty() {
        bail!("Pick a device to send to");
    }
    let mut args: Vec<&OsStr> = vec!["file".as_ref(), "cp".as_ref()];
    args.extend(files.iter().map(|f| f.as_os_str()));
    let target = format!("{peer}:");
    args.push(target.as_ref());
    exec("tailscale", args).await?;
    Ok(Outcome::said(format!(
        "Sent {} to {peer}",
        plural(files.len(), "file")
    )))
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(program).is_file()))
}

fn not_found(program: &str, e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!("{program} isn't installed")
    } else {
        anyhow::anyhow!("Couldn't start {program}: {e}")
    }
}

/// Runs a program to completion; a non-zero exit becomes an error carrying its last
/// line of stderr, which is usually the useful one.
pub async fn exec<I, S>(program: &str, args: I) -> anyhow::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| not_found(program, e))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();
        bail!(
            "{program} failed{}{last}",
            if last.is_empty() { "" } else { ": " }
        );
    }
    Ok(out)
}

/// Like `exec`, returning stdout as text.
pub async fn output<I, S>(program: &str, args: I) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = exec(program, args).await?;
    String::from_utf8(out.stdout).with_context(|| format!("{program} printed non-UTF-8"))
}

fn size_of(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
