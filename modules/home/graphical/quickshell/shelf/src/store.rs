//! The shelf's items: what's parked on it, in order, saved across restarts in
//! $XDG_STATE_HOME/shelf/items.json. The daemon owns them; the UI mirrors them through
//! `shelf watch`. Results (files in `files::results_dir`) belong to the shelf: one is
//! deleted when it leaves it, and any the shelf has lost track of go at startup. Items whose
//! file goes away (moved by a drag, deleted) leave the shelf on their own: `crate::watch`
//! reports them while the daemon runs, a check at startup catches the rest.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::{
    files::{
        Kind, in_dir, kind_of, link_name, link_url, remove_path, results_dir, state_dir,
    },
    watch::{Events, Watcher},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub path: String,
    pub name: String,
    /// The UI's icon: "folder", "video", "image", "archive", "code", "link", "text" or
    /// "file".
    pub kind: String,
    /// One of the shelf's own results, deleted when it leaves the shelf ("Temp" in the UI).
    /// Worked out from where it lives, never taken from the saved file.
    #[serde(default)]
    pub temp: bool,
    /// For a link (`.url` shortcut): its address, which the UI drags out and copies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Item {
    fn new(path: &str, temp: bool) -> Self {
        let clean = clean(path);
        let url = (kind_of(&clean) == Kind::Link)
            .then(|| link_url(Path::new(&clean)))
            .flatten();
        Self {
            name: match &url {
                Some(url) => link_name(url),
                None => Path::new(&clean)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| clean.clone()),
            },
            kind: if url.is_some() { "link" } else { ui_kind(path) }.into(),
            path: clean,
            temp,
            url,
        }
    }
}

/// Without trailing slashes, which folders may arrive with.
fn clean(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() { "/" } else { trimmed }.to_string()
}

fn ui_kind(path: &str) -> &'static str {
    match kind_of(path) {
        Kind::Folder => "folder",
        Kind::Video => "video",
        Kind::Image => "image",
        Kind::Archive => "archive",
        // Only a "link" with a URL in it (`Item::new`).
        Kind::Pdf | Kind::Link => "file",
        Kind::Text => "text",
        Kind::Other => {
            let ext = Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_lowercase();
            match ext.as_str() {
                // Not an image to the tools (they'd rasterise it), but it looks like one.
                "svg" => "image",
                "nix" | "lua" | "qml" | "js" | "ts" | "py" | "rs" | "go" | "c" | "h" | "cpp"
                | "sh" | "json" | "toml" | "yaml" | "yml" | "html" | "css" => "code",
                _ => "file",
            }
        }
    }
}

#[derive(Clone)]
pub struct Store {
    items: Arc<Mutex<Vec<Item>>>,
    file: Option<PathBuf>,
    results: Option<PathBuf>,
    watcher: Option<Arc<Watcher>>,
}

impl Store {
    /// The saved shelf, and the events of its items' files (for `check`), if inotify works.
    pub fn load() -> (Self, Option<Events>) {
        let file = state_dir().ok().map(|d| d.join("items.json"));
        let results = results_dir()
            .inspect_err(|e| eprintln!("no results dir: {e:#}"))
            .ok();
        let saved = match file.as_ref().map(std::fs::read) {
            Some(Ok(bytes)) => serde_json::from_slice::<Vec<Item>>(&bytes).ok(),
            // Never used yet.
            Some(Err(e)) if e.kind() == ErrorKind::NotFound => Some(Vec::new()),
            _ => None,
        };
        let (watcher, events) = match Watcher::new() {
            Ok((w, e)) => (Some(Arc::new(w)), Some(e)),
            Err(e) => {
                eprintln!("can't watch the items' files: {e}");
                (None, None)
            }
        };
        let mut store = Self {
            items: Arc::default(),
            file,
            results,
            watcher,
        };
        let items = saved
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|i| store.item(&i.path))
            .collect();
        store.items = Arc::new(Mutex::new(items));
        // Only with a list that's really the shelf's: an unreadable one would sweep it all.
        match saved {
            Some(items) => store.sweep(&items),
            None => eprintln!("can't read the saved items; starting empty"),
        }
        // Files that went while the daemon wasn't running; this also sets up the watches.
        let gone = store.missing();
        if !store.remove(&gone) {
            store.sync_watches();
        }
        (store, events)
    }

    pub fn items(&self) -> Vec<Item> {
        self.items.lock().unwrap().clone()
    }

    /// Appends `paths` that aren't on the shelf yet. True if anything changed.
    pub fn add(&self, paths: &[String]) -> bool {
        self.change(|items| {
            let before = items.len();
            for p in paths.iter().filter(|p| !p.is_empty()) {
                let item = self.item(p);
                if !items.iter().any(|i| i.path == item.path) {
                    items.push(item);
                }
            }
            items.len() != before
        })
    }

    /// Puts a tool's results on top (moving them up if already there).
    pub fn add_results(&self, paths: &[String]) -> bool {
        if paths.is_empty() {
            return false;
        }
        self.change(|items| {
            let fresh: Vec<Item> = paths.iter().map(|p| self.item(p)).collect();
            items.retain(|i| !fresh.iter().any(|f| f.path == i.path));
            items.splice(0..0, fresh);
            true
        })
    }

    pub fn remove(&self, paths: &[String]) -> bool {
        let gone: Vec<String> = paths.iter().map(|p| clean(p)).collect();
        self.take(|i| gone.contains(&i.path))
    }

    pub fn clear(&self) -> bool {
        self.take(|_| true)
    }

    /// Moves an item to `new` in place, `mv` doing the moving on disk. Under the items'
    /// lock, so an event from the move can't take the item off first. False if `old` isn't
    /// on the shelf (nothing is moved then).
    pub fn relocate(
        &self,
        old: &str,
        new: &str,
        mv: impl FnOnce() -> std::io::Result<()>,
    ) -> std::io::Result<bool> {
        let old = clean(old);
        let mut result = Ok(false);
        self.change(|items| {
            let Some(i) = items.iter().position(|i| i.path == old) else {
                return false;
            };
            result = mv().map(|()| true);
            if result.is_ok() {
                items[i] = self.item(new);
            }
            result.is_ok()
        });
        result
    }

    /// After a watch event on `paths`: takes the gone ones off and watches the rest
    /// again. True if the items changed.
    pub fn check(&self, paths: &[String]) -> bool {
        let (gone, kept): (Vec<String>, Vec<String>) =
            paths.iter().cloned().partition(|p| is_missing(p));
        if let Some(w) = &self.watcher {
            kept.iter().for_each(|p| w.rewatch(p));
        }
        self.remove(&gone)
    }

    /// Items whose file is gone. Checked on a copy of the list, so a slow mount doesn't
    /// hold up every other call; take them off with `remove`.
    fn missing(&self) -> Vec<String> {
        self.items()
            .into_iter()
            .map(|i| i.path)
            .filter(|p| is_missing(p))
            .collect()
    }

    /// The items a watch event may concern.
    pub fn watcher_paths(&self, event: &inotify::EventOwned) -> Vec<String> {
        self.watcher
            .as_ref()
            .map(|w| w.paths_for(event))
            .unwrap_or_default()
    }

    fn sync_watches(&self) {
        if let Some(w) = &self.watcher {
            w.sync(|| self.items().into_iter().map(|i| i.path).collect());
        }
    }

    fn item(&self, path: &str) -> Item {
        Item::new(path, self.result(&clean(path)).is_some())
    }

    /// Takes the items matching `pick` off the shelf, deleting the ones that are results.
    fn take(&self, pick: impl Fn(&Item) -> bool) -> bool {
        let mut taken = Vec::new();
        let changed = self.change(|items| {
            let (out, keep) = std::mem::take(items).into_iter().partition(&pick);
            *items = keep;
            taken = out;
            !taken.is_empty()
        });
        for item in &taken {
            if let Some(path) = self.result(&item.path) {
                delete(&path);
            }
        }
        changed
    }

    /// `path` if it's one of the shelf's results: directly in the results dir, nothing
    /// that could climb out of it.
    fn result(&self, path: &str) -> Option<PathBuf> {
        let dir = self.results.as_ref()?;
        in_dir(path, dir).then(|| PathBuf::from(path))
    }

    /// Deletes results that aren't on the shelf (a crash between making one and adding it).
    fn sweep(&self, items: &[Item]) {
        let Some(dir) = &self.results else { return };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !items.iter().any(|i| Path::new(&i.path) == path) {
                delete(&path);
            }
        }
    }

    fn change(&self, f: impl FnOnce(&mut Vec<Item>) -> bool) -> bool {
        let changed = {
            let mut items = self.items.lock().unwrap();
            let changed = f(&mut items);
            if changed {
                self.save(&items);
            }
            changed
        };
        // Outside the lock: adding a watch can be slow on a network mount.
        if changed {
            self.sync_watches();
        }
        changed
    }

    fn save(&self, items: &[Item]) {
        let Some(file) = &self.file else { return };
        let result = (|| -> std::io::Result<()> {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            // Written aside and renamed in, so a crash never leaves half a file.
            let tmp = file.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_vec(items)?)?;
            std::fs::rename(&tmp, file)
        })();
        if let Err(e) = result {
            eprintln!("can't save {}: {e}", file.display());
        }
    }
}

fn is_missing(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_err_and(|e| e.kind() == ErrorKind::NotFound)
}

fn delete(path: &Path) {
    match remove_path(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            eprintln!("can't delete {}: {e}", path.display())
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store {
            items: Arc::default(),
            file: None,
            results: None,
            watcher: None,
        }
    }

    /// A store whose results dir is a fresh temp dir.
    fn with_results() -> (Store, PathBuf) {
        let root = std::env::temp_dir().join(format!("shelf-store-{}", crate::files::token()));
        let dir = root.join("files");
        std::fs::create_dir_all(&dir).unwrap();
        let s = Store {
            results: Some(dir.clone()),
            ..store()
        };
        (s, root)
    }

    fn paths(s: &Store) -> Vec<String> {
        s.items().into_iter().map(|i| i.path).collect()
    }

    #[test]
    fn add_dedupes_and_trims_slashes() {
        let s = store();
        assert!(s.add(&["/a/dir/".into(), "/b.png".into()]));
        assert!(!s.add(&["/a/dir".into()]));
        assert_eq!(paths(&s), ["/a/dir", "/b.png"]);
        assert_eq!(s.items()[0].kind, "folder");
        assert_eq!(s.items()[1].kind, "image");
    }

    #[test]
    fn results_go_on_top_and_are_temp() {
        let (s, root) = with_results();
        let result = root.join("files/r.png").to_string_lossy().into_owned();
        s.add(&["/a".into(), "/b".into()]);
        s.add_results(&["/b".into(), result.clone()]);
        assert_eq!(paths(&s), ["/b", result.as_str(), "/a"]);
        let temp: Vec<bool> = s.items().iter().map(|i| i.temp).collect();
        assert_eq!(temp, [false, true, false]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_files_are_found() {
        let (s, root) = with_results();
        let here = root.join("here.png");
        std::fs::write(&here, b"").unwrap();
        let gone = root.join("gone.png").to_string_lossy().into_owned();
        s.add(&[here.to_string_lossy().into_owned(), gone.clone()]);
        assert_eq!(s.missing(), [gone]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn remove_and_clear() {
        let s = store();
        s.add(&["/a".into(), "/b".into()]);
        assert!(s.remove(&["/a/".into()]));
        assert_eq!(paths(&s), ["/b"]);
        assert!(s.clear() && !s.clear());
    }

    #[test]
    fn leaving_results_are_deleted_originals_kept() {
        let (s, root) = with_results();
        let dir = root.join("files");
        let original = root.join("original.png");
        let result = dir.join("a.webp");
        let folder = dir.join("extracted");
        std::fs::write(&original, b"").unwrap();
        std::fs::write(&result, b"").unwrap();
        std::fs::create_dir_all(folder.join("inner")).unwrap();
        let p = |x: &Path| x.to_string_lossy().into_owned();
        s.add(&[p(&original)]);
        s.add_results(&[p(&result), p(&folder)]);

        s.remove(&[p(&result)]);
        assert!(!result.exists() && folder.exists());
        s.clear();
        assert!(!folder.exists() && original.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn only_plain_paths_in_the_results_dir_count() {
        let (s, root) = with_results();
        let dir = root.join("files");
        let p = |x: PathBuf| x.to_string_lossy().into_owned();
        assert!(s.result(&p(dir.join("a.png"))).is_some());
        assert!(s.result(&p(dir.join("sub/a.png"))).is_none());
        assert!(s.result(&p(dir.join("../escape"))).is_none());
        assert!(s.result(&p(dir.join("x/../../escape"))).is_none());
        assert!(s.result(&p(dir.clone())).is_none());
        assert!(s.result("/elsewhere/a.png").is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn sweep_deletes_only_results_off_the_shelf() {
        let (s, root) = with_results();
        let dir = root.join("files");
        let kept = dir.join("kept.png");
        let lost = dir.join("lost.png");
        std::fs::write(&kept, b"").unwrap();
        std::fs::write(&lost, b"").unwrap();
        s.sweep(&[Item::new(&kept.to_string_lossy(), true)]);
        assert!(kept.exists() && !lost.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn kinds() {
        assert_eq!(ui_kind("/x/main.rs"), "code");
        assert_eq!(ui_kind("/x/logo.svg"), "image");
        assert_eq!(ui_kind("/x/doc.pdf"), "file");
    }
}
