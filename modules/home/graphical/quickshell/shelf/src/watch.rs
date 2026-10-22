//! Notices when an item's file goes away (moved by a file manager the drag went to,
//! deleted, its disk unmounted), through inotify watches on the items themselves.
//! Network and FUSE mounts don't report changes made elsewhere; such an item stays until
//! the daemon restarts.

use std::{collections::HashMap, io, sync::Mutex};

use inotify::{EventMask, EventOwned, EventStream, Inotify, WatchDescriptor, WatchMask, Watches};

/// Unmounts and IGNORED (the watch is gone, e.g. after a delete) always come too.
const MASK: WatchMask = WatchMask::MOVE_SELF.union(WatchMask::DELETE_SELF);

pub type Events = EventStream<[u8; 4096]>;

pub struct Watcher {
    watches: Mutex<Watches>,
    by_path: Mutex<HashMap<String, WatchDescriptor>>,
}

impl Watcher {
    pub fn new() -> io::Result<(Self, Events)> {
        let events = Inotify::init()?.into_event_stream([0; 4096])?;
        let watcher = Self {
            watches: Mutex::new(events.watches()),
            by_path: Mutex::default(),
        };
        Ok((watcher, events))
    }

    /// Watches exactly `paths` (all the items), adding and dropping watches as needed.
    /// `paths` is called under the watcher's lock, so the last sync always sees the latest
    /// items however calls interleave.
    pub fn sync(&self, paths: impl FnOnce() -> Vec<String>) {
        let mut by_path = self.by_path.lock().unwrap();
        let paths = paths();
        let stale: Vec<WatchDescriptor> = by_path
            .extract_if(|p, _| !paths.contains(p))
            .map(|(_, wd)| wd)
            .collect();
        let mut watches = self.watches.lock().unwrap();
        for wd in stale {
            // The same file under two paths shares a watch.
            if !by_path.values().any(|w| *w == wd) {
                // Already gone if the file was deleted.
                let _ = watches.remove(wd);
            }
        }
        for p in paths {
            if !by_path.contains_key(&p)
                && let Ok(wd) = watches.add(&p, MASK)
            {
                by_path.insert(p, wd);
            }
        }
    }

    /// Watches `path` again: after an event its file may be a new one (saved over by
    /// rename) or the watch gone. The old file's watch is dropped.
    pub fn rewatch(&self, path: &str) {
        let mut by_path = self.by_path.lock().unwrap();
        let Some(old) = by_path.remove(path) else {
            return;
        };
        let mut watches = self.watches.lock().unwrap();
        if let Ok(wd) = watches.add(path, MASK) {
            by_path.insert(path.to_string(), wd);
        }
        // Unless it's still this file's (same inode, same watch) or another item's.
        if !by_path.values().any(|w| *w == old) {
            // Already gone if the file was deleted.
            let _ = watches.remove(old);
        }
    }

    /// The paths an event may concern: those of its watch, or all of them if events were
    /// lost.
    pub fn paths_for(&self, event: &EventOwned) -> Vec<String> {
        let by_path = self.by_path.lock().unwrap();
        let all = event.mask.contains(EventMask::Q_OVERFLOW);
        by_path
            .iter()
            .filter(|(_, wd)| all || **wd == event.wd)
            .map(|(p, _)| p.clone())
            .collect()
    }
}
