//! "Hand to shelf": files move into the results dir and become the shelf's own (temp),
//! staying where they are in the list.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::{
    files::{in_dir, name, plural, remove_path, results_dir, unique},
    store::Store,
    tools::{Outcome, exec},
};

pub async fn hand_over(store: &Store, files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
    let mut moved = Vec::new();
    let mut kept_originals = Vec::new();
    for file in files {
        let old = file.to_string_lossy();
        if in_dir(&old, &dir) {
            continue;
        }
        let new = unique(&dir, &name(file));
        let new_str = new.to_string_lossy();
        // On the same disk, a rename: done under the store's lock (see `Store::relocate`).
        match store.relocate(&old, &new_str, || std::fs::rename(file, &new)) {
            Ok(true) => moved.push(new),
            Ok(false) => {}
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                if copy_in(store, file, &new).await? {
                    if remove_path(file).is_err() {
                        kept_originals.push(name(file));
                    }
                    moved.push(new);
                }
            }
            Err(e) => {
                return Err(e).with_context(|| format!("Can't move {}", name(file)));
            }
        }
    }
    if moved.is_empty() {
        bail!("Nothing to move");
    }
    let mut message = match moved.as_slice() {
        [one] => format!("Handed {} to the shelf", name(one)),
        _ => format!("Handed {} to the shelf", plural(moved.len(), "item")),
    };
    if !kept_originals.is_empty() {
        message = format!("Copied, but can't delete {}", kept_originals.join(", "));
    }
    // Not `outputs`: those would go on top as new results. The UI selects them.
    Ok(Outcome {
        select: moved
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        ..Outcome::said(message)
    })
}

/// To another disk: copies (outside the lock, it can take a while), then points the item
/// at the copy. False if the item left the shelf meanwhile.
async fn copy_in(store: &Store, file: &Path, new: &Path) -> anyhow::Result<bool> {
    let copied = exec(
        "cp",
        [
            "-a".as_ref(),
            "--reflink=auto".as_ref(),
            "-T".as_ref(),
            "--".as_ref(),
            file.as_os_str(),
            new.as_os_str(),
        ],
    )
    .await;
    let on_shelf = copied.is_ok()
        && store
            .relocate(&file.to_string_lossy(), &new.to_string_lossy(), || Ok(()))
            .unwrap_or(false);
    if !on_shelf {
        let _ = remove_path(new);
    }
    copied.with_context(|| format!("Can't copy {}", name(file)))?;
    Ok(on_shelf)
}
