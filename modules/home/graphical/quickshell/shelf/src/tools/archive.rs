//! Archives, all through bsdtar (libarchive): it reads zip, tar.*, 7z and rar alike.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::Context;

use super::{Outcome, exec};
use crate::files::{name, out_dir, plural, stem, unique};

pub async fn extract(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = out_dir()?;
    let mut made = Vec::new();
    for file in files {
        let folder = unique(&dir, &stem(file));
        std::fs::create_dir_all(&folder)
            .with_context(|| format!("Can't create {}", folder.display()))?;
        exec(
            "bsdtar",
            [
                OsString::from("-xf"),
                file.into(),
                "-C".into(),
                folder.clone().into(),
            ],
        )
        .await?;
        made.push(folder);
    }
    Ok(Outcome::made(
        format!("Extracted {}", plural(files.len(), "archive")),
        made,
    ))
}

pub async fn archive(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let out = unique(&out_dir()?, &format!("shelf-{}-files.zip", files.len()));
    zip_to(files, &out).await?;
    Ok(Outcome::made(
        format!("Packed {}", plural(files.len(), "file")),
        vec![out],
    ))
}

/// Zips files (and folders, recursively) by their names, wherever each one lives.
pub async fn zip_to(files: &[PathBuf], out: &Path) -> anyhow::Result<()> {
    let mut args: Vec<OsString> = vec!["-a".into(), "-cf".into(), out.into()];
    for file in files {
        let parent = file.parent().unwrap_or(Path::new("/"));
        args.extend(["-C".into(), parent.into(), name(file).into()]);
    }
    exec("bsdtar", args).await.map(|_| ())
}
