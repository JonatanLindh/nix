//! Archives, all through bsdtar (libarchive): it reads zip, tar.*, 7z and rar alike.

use std::{
    collections::HashSet,
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::Context;

use super::{Outcome, exec};
use crate::files::{name, numbered, plural, results_dir, runtime_dir, stem, token, unique};

pub async fn extract(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
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
    let out = unique(&results_dir()?, &format!("shelf-{}-files.zip", files.len()));
    zip_to(files, &out).await?;
    Ok(Outcome::made(
        format!("Packed {}", plural(files.len(), "file")),
        vec![out],
    ))
}

/// Zips files (and folders, recursively) by their names, wherever each one lives. Names
/// that clash (`a/notes.txt`, `b/notes.txt`) go in as `notes (2).txt` etc., like `unique`
/// does on disk, so extracting doesn't keep only one of them.
pub async fn zip_to(files: &[PathBuf], out: &Path) -> anyhow::Result<()> {
    let names = entry_names(files);
    // A renamed entry is a symlink by its new name in a scratch dir, which `-H` follows.
    // That also follows a shelf item that is itself a symlink, which is what a zip for
    // somewhere else wants anyway.
    let scratch = runtime_dir()?.join(format!("zip-{}", token()));
    let mut args: Vec<OsString> = vec!["-a".into(), "-H".into(), "-cf".into(), out.into()];
    for (file, entry) in files.iter().zip(&names) {
        if *entry == name(file) {
            let parent = file.parent().unwrap_or(Path::new("/"));
            args.extend(["-C".into(), parent.into(), entry.into()]);
        } else {
            std::fs::create_dir_all(&scratch)
                .with_context(|| format!("Can't create {}", scratch.display()))?;
            std::os::unix::fs::symlink(file, scratch.join(entry))
                .with_context(|| format!("Can't stage {}", name(file)))?;
            args.extend(["-C".into(), scratch.clone().into(), entry.into()]);
        }
    }
    let result = exec("bsdtar", args).await.map(|_| ());
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Each file's name in the archive: its own, or for a later one with the same name the
/// first `name (i).ext` that no file has.
fn entry_names(files: &[PathBuf]) -> Vec<String> {
    let own: HashSet<String> = files.iter().map(|f| name(f)).collect();
    let mut used = HashSet::new();
    files
        .iter()
        .map(|f| {
            let n = name(f);
            let entry = if used.contains(&n) {
                (2..)
                    .map(|i| numbered(&n, i))
                    .find(|c| !own.contains(c) && !used.contains(c))
                    .unwrap_or(n)
            } else {
                n
            };
            used.insert(entry.clone());
            entry
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clashing_names_are_numbered() {
        let files: Vec<PathBuf> = [
            "/a/notes.txt",
            "/b/notes.txt",
            "/c/notes (2).txt",
            "/d/notes.txt",
            "/e/x",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            entry_names(&files),
            [
                "notes.txt",
                "notes (3).txt",
                "notes (2).txt",
                "notes (4).txt",
                "x"
            ]
        );
    }
}
