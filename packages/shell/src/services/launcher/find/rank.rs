//! Ranking for the launcher's apps and 1Password items, with launch counts (qs-find's rank.rs).
//!
//! Name prefix > word prefix > substring (of the name or the item's other text) > fuzzy match
//! of the name, the fuzzy ones by how well they match. Within a tier the more used go first
//! (frequency, weighted towards recent use), then alphabetical.
//!
//! The counts live in a JSON file per list, the format Quickshell's launcher kept; it's written
//! in the background after each launch.

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const RECENT: usize = 6;

/// Something to rank: an app or a 1Password item.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Other text to find it by: generic name and keywords, username and URLs, ...
    pub hay: String,
}

struct Entry {
    id: String,
    name: String,
    lname: String,
    words: Vec<String>,
    hay: String,
}

/// Uses per id, as the launcher has always kept them: { id: { n: count, t: last use in ms } }.
#[derive(Serialize, Deserialize, Clone, Copy)]
struct Use {
    n: u64,
    t: u64,
}

pub struct List {
    entries: Vec<Entry>,
    usage: HashMap<String, Use>,
    path: Option<PathBuf>,
    /// Saves started, and the newest one written, so a slow older save can't undo a newer one.
    saves: u64,
    saved: Arc<Mutex<u64>>,
    matcher: RefCell<Matcher>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Through a temporary file, so a crash mid-write doesn't lose the counts.
fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

impl List {
    pub fn new(path: Option<PathBuf>) -> Self {
        // A missing or broken file just means nothing has been used yet.
        let usage = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        List {
            entries: Vec::new(),
            usage,
            path,
            saves: 0,
            saved: Arc::default(),
            matcher: RefCell::new(Matcher::new(Config::DEFAULT)),
        }
    }

    pub fn set(&mut self, items: Vec<Item>) {
        self.entries = items
            .into_iter()
            .map(|i| {
                let lname = i.name.to_lowercase();
                Entry {
                    words: lname
                        .split(|c: char| c.is_whitespace() || "-_.@".contains(c))
                        .filter(|w| !w.is_empty())
                        .map(String::from)
                        .collect(),
                    hay: i.hay.to_lowercase(),
                    lname,
                    name: i.name,
                    id: i.id,
                }
            })
            .collect();
        self.entries.sort_by(|a, b| a.lname.cmp(&b.lname));
    }

    pub fn used(&mut self, id: &str) {
        let u = self
            .usage
            .entry(id.to_string())
            .or_insert(Use { n: 0, t: 0 });
        u.n += 1;
        u.t = now_ms();
        let Some(path) = self.path.clone() else {
            return;
        };
        let bytes = match serde_json::to_vec(&self.usage) {
            Ok(b) => b,
            Err(e) => return eprintln!("launcher: couldn't save {}: {e}", path.display()),
        };
        self.saves += 1;
        let (save, saved) = (self.saves, self.saved.clone());
        crate::runtime::detach(async move {
            let _ = tokio::task::spawn_blocking(move || {
                let mut newest = saved.lock().unwrap_or_else(|e| e.into_inner());
                if save > *newest {
                    *newest = save;
                    if let Err(e) = write(&path, &bytes) {
                        eprintln!("launcher: couldn't save {}: {e}", path.display());
                    }
                }
            })
            .await;
        });
    }

    fn frecency(&self, id: &str, now: u64) -> f64 {
        let Some(u) = self.usage.get(id) else {
            return 0.0;
        };
        let days = now.saturating_sub(u.t) as f64 / 86_400_000.0;
        let weight = match days {
            d if d < 1.0 => 4.0,
            d if d < 7.0 => 2.0,
            d if d < 30.0 => 1.0,
            _ => 0.5,
        };
        u.n as f64 * weight
    }

    /// Ids in order (at most `limit`, 0 for all), and with an empty query the most used few.
    pub fn rank(&self, query: &str, limit: usize) -> (Vec<String>, Vec<String>) {
        let q = query.trim().to_lowercase();
        let now = now_ms();
        let atom = Atom::new(
            &q,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
            false,
        );
        let mut matcher = self.matcher.borrow_mut();
        let mut buf = Vec::new();

        // (tier, fuzzy score, use, alphabetical position)
        let mut scored: Vec<(u8, u16, f64, usize)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            let (tier, fuzzy) = if q.is_empty() || e.lname.starts_with(&q) {
                (0, 0)
            } else if e.words.iter().any(|w| w.starts_with(&q)) {
                (1, 0)
            } else if e.lname.contains(&q) || e.hay.contains(&q) {
                (2, 0)
            } else if let Some(s) = atom.score(Utf32Str::new(&e.name, &mut buf), &mut matcher) {
                (3, s)
            } else {
                continue;
            };
            scored.push((tier, fuzzy, self.frecency(&e.id, now), i));
        }
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.cmp(&a.1))
                .then(b.2.total_cmp(&a.2))
                .then(a.3.cmp(&b.3))
        });
        if limit > 0 {
            scored.truncate(limit);
        }
        let ids = scored
            .iter()
            .map(|s| self.entries[s.3].id.clone())
            .collect();

        let mut recent = Vec::new();
        if q.is_empty() {
            let mut used: Vec<(f64, usize)> = (self.entries.iter().enumerate())
                .filter(|(_, e)| self.usage.contains_key(&e.id))
                .map(|(i, e)| (self.frecency(&e.id, now), i))
                .collect();
            used.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            recent = used
                .iter()
                .take(RECENT)
                .map(|u| self.entries[u.1].id.clone())
                .collect();
        }
        (ids, recent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(names: &[(&str, &str)]) -> List {
        let mut l = List::new(None);
        l.set(
            (names.iter())
                .map(|(name, hay)| Item {
                    id: name.to_lowercase(),
                    name: name.to_string(),
                    hay: hay.to_string(),
                })
                .collect(),
        );
        l
    }

    #[test]
    fn tiers() {
        let l = list(&[
            ("Firefox", "web browser"),
            ("Thunderbird", "mail"),
            ("GNU Image Manipulation Program", ""),
            ("Files", "file manager"),
            ("Profile Editor", ""),
            ("Fuzzy Fixer", ""),
        ]);
        // Name prefixes (alphabetical), then a word prefix, then a substring, then fuzzy.
        let (ids, recent) = l.rank("fi", 0);
        assert_eq!(
            ids,
            ["files", "firefox", "fuzzy fixer", "profile editor"].map(String::from)
        );
        assert!(recent.is_empty());
        // The other text counts as a substring match.
        assert_eq!(l.rank("browser", 0).0, ["firefox"].map(String::from));
        // Fuzzy, on the name only.
        assert_eq!(
            l.rank("gimp", 0).0,
            ["gnu image manipulation program"].map(String::from)
        );
        assert_eq!(l.rank("fi", 2).0.len(), 2);
        assert_eq!(l.rank("", 0).0.len(), 6);
    }

    #[test]
    fn usage() {
        let mut l = list(&[("Alpha", ""), ("Beta", ""), ("Gamma", "")]);
        let now = now_ms();
        l.usage.insert("gamma".into(), Use { n: 3, t: now });
        // Many uses, but long ago: 10 × 0.5 against 3 × 4.
        l.usage.insert(
            "beta".into(),
            Use {
                n: 10,
                t: now - 60 * 86_400_000,
            },
        );
        let (ids, recent) = l.rank("", 0);
        assert_eq!(ids, ["gamma", "beta", "alpha"].map(String::from));
        assert_eq!(recent, ["gamma", "beta"].map(String::from));
        // Used ones first within a tier; a typed query has no recent row.
        l.used("alpha");
        let (ids, recent) = l.rank("a", 0);
        assert_eq!(ids, ["alpha", "gamma", "beta"].map(String::from));
        assert!(recent.is_empty());
    }

    #[test]
    fn usage_file() {
        let dir = std::env::temp_dir().join(format!("shell-rank-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("launcher.json");
        std::fs::write(&path, r#"{"b.desktop":{"n":2,"t":1700000000000}}"#).unwrap();
        let l = List::new(Some(path));
        assert_eq!(l.usage["b.desktop"].n, 2);
        // A broken file is as good as none.
        std::fs::write(dir.join("broken.json"), "{").unwrap();
        assert!(List::new(Some(dir.join("broken.json"))).usage.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
