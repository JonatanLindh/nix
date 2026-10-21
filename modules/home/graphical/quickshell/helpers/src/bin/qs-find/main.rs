//! `qs-find`: the launcher's search. Quickshell keeps one running and talks to it over
//! stdin/stdout, one JSON object per line; replies carry the request's `seq` so the launcher
//! can drop answers to queries it has moved on from.
//!
//!   {"op": "items", "list": "apps", "items": [{"id", "name", "hay"}]}  set a list to rank
//!   {"op": "rank", "seq": 1, "list": "apps", "q": "fi", "limit": 0}
//!       → {"re": "rank", "seq": 1, "list": "apps", "ids": [...], "recent": [...]}
//!   {"op": "used", "list": "apps", "id": "firefox.desktop"}             count a launch or pick
//!   {"op": "calc", "seq": 2, "q": "5 km/h to m/s"}
//!       → {"re": "calc", "seq": 2, "ok": true, "text": "(25/18) m/s", "math": <tree>,
//!          "approx": "1.388 888 889 m/s", "value": "1.388888889 m/s", "exact": ..., "tex": ...}
//!   (text is the exact form and math the same typeset, see tex.rs; approx is the decimal
//!   one when it differs; value, exact and tex are what to copy)
//!   {"op": "files", "seq": 3, "scope": "home", "q": "notes"}
//!       → {"re": "files", "seq": 3, "files": [...], "total": 12, "capped": false}
//!
//! Lists are "apps" and "pass"; `--usage LIST=PATH` names the JSON file each one's launch
//! counts are kept in. File searches and calculations (qalc) run on their own threads, and a
//! newer one of the same kind stops the older.

mod calc;
mod files;
mod rank;
mod tex;

use rank::{Item, List};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Request {
    Items {
        list: String,
        items: Vec<Item>,
    },
    Rank {
        seq: u64,
        list: String,
        q: String,
        #[serde(default)]
        limit: usize,
    },
    Used {
        list: String,
        id: String,
    },
    Calc {
        seq: u64,
        q: String,
    },
    Files {
        seq: u64,
        scope: String,
        q: String,
    },
}

fn emit(v: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

fn main() {
    let mut lists: HashMap<String, List> = HashMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--usage"
            && let Some((list, path)) = args.next().as_deref().and_then(|a| a.split_once('='))
        {
            lists.insert(list.into(), List::new(Some(PathBuf::from(path))));
        }
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
    // The newest file search and calculation; older ones see it change and stop.
    let search = Arc::new(AtomicU64::new(0));
    let calcs = Arc::new(AtomicU64::new(0));

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("qs-find: bad request: {e}");
                continue;
            }
        };
        match request {
            Request::Items { list, items } => {
                lists
                    .entry(list)
                    .or_insert_with(|| List::new(None))
                    .set(items);
            }
            Request::Rank {
                seq,
                list,
                q,
                limit,
            } => {
                let (ids, recent) = lists
                    .get(&list)
                    .map(|l| l.rank(&q, limit))
                    .unwrap_or_default();
                emit(
                    json!({ "re": "rank", "seq": seq, "list": list, "ids": ids, "recent": recent }),
                );
            }
            Request::Used { list, id } => {
                lists
                    .entry(list)
                    .or_insert_with(|| List::new(None))
                    .used(&id);
            }
            Request::Calc { seq, q } => {
                let generation = calcs.fetch_add(1, Ordering::SeqCst) + 1;
                let calcs = calcs.clone();
                std::thread::spawn(move || {
                    let stale = || calcs.load(Ordering::SeqCst) != generation;
                    if let Some(a) = calc::eval(&q, &stale) {
                        emit(json!({
                            "re": "calc",
                            "seq": seq,
                            "ok": a.ok,
                            "text": a.text,
                            "math": a.math,
                            "approx": a.approx,
                            "value": a.value,
                            "exact": a.exact,
                            "tex": a.tex,
                        }));
                    }
                });
            }
            Request::Files { seq, scope, q } => {
                let generation = search.fetch_add(1, Ordering::SeqCst) + 1;
                let search = search.clone();
                let home = home.clone();
                std::thread::spawn(move || {
                    let stale = || search.load(Ordering::SeqCst) != generation;
                    let found = files::search(&scope, &q, &home, &stale);
                    if !stale() {
                        emit(json!({
                            "re": "files",
                            "seq": seq,
                            "files": found.files,
                            "total": found.total,
                            "capped": found.capped,
                        }));
                    }
                });
            }
        }
    }
}
