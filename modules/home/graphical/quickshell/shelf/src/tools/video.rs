//! Video: ffmpeg re-encodes for chat upload limits and makes GIFs.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::Context;

use super::{Outcome, exec, output, size_of};
use crate::files::{human_size, plural, results_dir, runtime_dir, stem, token, unique};

/// Discord and friends cap uploads at 10 MB; aim a little under for container overhead.
const TARGET_BYTES: f64 = 9.5e6;
const AUDIO_KBPS: f64 = 96.0;

async fn duration(file: &Path) -> anyhow::Result<f64> {
    let out = output(
        "ffprobe",
        [
            "-v".as_ref(),
            "error".as_ref(),
            "-show_entries".as_ref(),
            "format=duration".as_ref(),
            "-of".as_ref(),
            "default=nw=1:nk=1".as_ref(),
            file.as_os_str(),
        ],
    )
    .await?;
    out.trim()
        .parse::<f64>()
        .ok()
        .filter(|d| *d > 0.0)
        .with_context(|| format!("Can't tell how long {} is", file.display()))
}

pub async fn shrink(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
    let mut made = Vec::new();
    for file in files {
        let secs = duration(file).await?;
        let video_kbps = (TARGET_BYTES * 8.0 / 1000.0 / secs - AUDIO_KBPS).max(100.0);
        let bitrate = format!("{}k", video_kbps.round());
        let out = unique(&dir, &format!("{}-10MB.mp4", stem(file)));
        // Two passes hit the size target far more reliably than one.
        let log = runtime_dir()?.join(format!("ffmpeg2pass-{}", token()));
        let common = |pass: &str| -> Vec<OsString> {
            vec![
                "-y".into(),
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-i".into(),
                file.into(),
                // Chat apps show it small anyway; fewer pixels look better per bit.
                "-vf".into(),
                "scale='min(1280,iw)':-2".into(),
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "medium".into(),
                "-b:v".into(),
                bitrate.clone().into(),
                "-pass".into(),
                pass.into(),
                "-passlogfile".into(),
                log.clone().into(),
            ]
        };
        let mut first = common("1");
        first.extend(["-an".into(), "-f".into(), "null".into(), "/dev/null".into()]);
        let mut second = common("2");
        second.extend([
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            format!("{AUDIO_KBPS}k").into(),
            "-movflags".into(),
            "+faststart".into(),
            out.clone().into(),
        ]);
        let result = async {
            exec("ffmpeg", first).await?;
            exec("ffmpeg", second).await
        }
        .await;
        for suffix in ["-0.log", "-0.log.mbtree"] {
            let mut path = log.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
        result?;
        made.push(out);
    }
    let size = made.iter().map(|p| size_of(p)).max().unwrap_or(0);
    let message = if made.len() == 1 {
        format!("Re-encoded to {} for chat", human_size(size))
    } else {
        format!("Re-encoded {} to fit 10 MB", plural(made.len(), "video"))
    };
    Ok(Outcome::made(message, made))
}

pub async fn gif(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
    let mut made = Vec::new();
    for file in files {
        let out = unique(&dir, &format!("{}.gif", stem(file)));
        exec(
            "ffmpeg",
            [
                "-y".into(),
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-i".into(),
                OsString::from(file),
                // A palette made from the clip itself keeps colours from banding.
                "-vf".into(),
                "fps=12,scale='min(480,iw)':-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse".into(),
                "-loop".into(),
                "0".into(),
                out.clone().into(),
            ],
        )
        .await?;
        made.push(out);
    }
    Ok(Outcome::made(
        format!("Made {}", plural(made.len(), "GIF")),
        made,
    ))
}
