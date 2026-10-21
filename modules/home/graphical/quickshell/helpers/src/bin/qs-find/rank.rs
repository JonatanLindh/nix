//! Ranking for the launcher's apps and 1Password items, with launch counts.
//!
//! Name prefix > word prefix > substring (of the name or the item's other text) > fuzzy match
//! of the name, the fuzzy ones by how well they match. Within a tier the more used go first
//! (frequency, weighted towards recent use), then alphabetical.

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const RECENT: usize = 6;

#[derive(Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Other text to find it by: generic name and keywords, username and URLs, ...
    #[serde(default)]
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
    matcher: RefCell<Matcher>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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
        if let Some(path) = &self.path {
            let tmp = path.with_extension("json.tmp");
            let written = serde_json::to_vec(&self.usage)
                .map_err(std::io::Error::other)
                .and_then(|b| std::fs::write(&tmp, b))
                .and_then(|()| std::fs::rename(&tmp, path));
            if let Err(e) = written {
                eprintln!("qs-find: couldn't save {}: {e}", path.display());
            }
        }
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
