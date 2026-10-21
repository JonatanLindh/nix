//! The launcher's file modes. With a query: every word must be in the name (or in the whole
//! path, once a word has a "/"), case-insensitive unless the word has capitals; hidden files
//! and .gitignored trees are skipped, like fd. Without one: recently used files for home (GTK's
//! list, which most apps add to), newest first for Downloads, nothing for "/".

use ignore::{WalkBuilder, WalkState};
use serde::Serialize;
use std::collections::HashSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

/// Matches looked at; past this the count shows as "400+".
const LIMIT: usize = 400;
/// Rows sent back.
const SHOWN: usize = 50;
const RECENT: usize = 120;
/// Skipped when searching from /: huge, virtual, or both.
const SKIP: [&str; 5] = ["/nix/store", "/proc", "/sys", "/dev", "/run"];

#[derive(Serialize)]
pub struct Hit {
    path: String,
    name: String,
    dir: String,
    mtime: i64,
    kind: &'static str,
}

pub struct Found {
    pub files: Vec<Hit>,
    pub total: usize,
    pub capped: bool,
}

pub fn search(scope: &str, q: &str, home: &Path, stale: &(dyn Fn() -> bool + Sync)) -> Found {
    let words: Vec<&str> = q.split_whitespace().collect();
    let base = match scope {
        "downloads" => home.join("Downloads"),
        "root" => PathBuf::from("/"),
        _ => home.to_path_buf(),
    };
    let mut found: Vec<(PathBuf, Metadata)> = match (words.is_empty(), scope) {
        (false, _) => walk(&base, &words, scope == "root", stale),
        (true, "downloads") => listing(&base),
        (true, "home") => recent(home),
        (true, _) => Vec::new(),
    };
    let capped = found.len() >= LIMIT;

    // Name prefix > name has every word > path match; then shallower, then newer. Listings
    // without a query keep their own order (newest first, or most recently used).
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let tier = |p: &Path| {
        let name = file_name(p).to_lowercase();
        if name.starts_with(&lower[0]) {
            0
        } else if lower.iter().all(|w| name.contains(w.as_str())) {
            1
        } else {
            2
        }
    };
    if !words.is_empty() {
        found.sort_by_cached_key(|(p, m)| (tier(p), p.components().count(), -mtime(m)));
    } else if scope == "downloads" {
        found.sort_by_key(|(_, m)| -mtime(m));
    }

    let home = home.to_string_lossy();
    Found {
        total: found.len(),
        capped,
        files: found
            .iter()
            .take(SHOWN)
            .map(|(p, m)| hit(p, m, &home))
            .collect(),
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| p.to_string_lossy(), |n| n.to_string_lossy())
        .into_owned()
}

fn mtime(m: &Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn hit(path: &Path, meta: &Metadata, home: &str) -> Hit {
    let name = file_name(path);
    let dir = path
        .parent()
        .map_or("/".into(), |d| d.to_string_lossy().into_owned());
    let dir = match dir.strip_prefix(home) {
        Some("") => "~".into(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => dir,
    };
    Hit {
        kind: if meta.is_dir() {
            "folder"
        } else {
            kind(&name.to_lowercase())
        },
        path: path.to_string_lossy().into_owned(),
        mtime: mtime(meta),
        name,
        dir,
    }
}

fn kind(name: &str) -> &'static str {
    const IMAGE: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "webp", "avif", "heic", "svg", "bmp", "tif", "tiff", "cr2",
        "cr3", "arw", "nef", "dng",
    ];
    const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v"];
    const ARCHIVE: &[&str] = &[
        "zip", "tar", "gz", "tgz", "xz", "zst", "bz2", "7z", "rar", "iso",
    ];
    const CODE: &[&str] = &[
        "nix", "lua", "qml", "js", "mjs", "ts", "tsx", "jsx", "py", "rs", "go", "c", "h", "cc",
        "cpp", "hpp", "java", "kt", "sh", "fish", "zsh", "json", "toml", "yml", "yaml", "html",
        "css", "scss", "sql", "zig", "hs", "rb", "php", "lock",
    ];
    if ["makefile", "justfile", "dockerfile"].contains(&name) {
        return "code";
    }
    let Some((_, ext)) = name.rsplit_once('.') else {
        return "file";
    };
    [
        (IMAGE, "image"),
        (VIDEO, "video"),
        (ARCHIVE, "archive"),
        (CODE, "code"),
    ]
    .iter()
    .find(|(exts, _)| exts.contains(&ext))
    .map_or("file", |(_, k)| k)
}

fn walk(
    base: &Path,
    words: &[&str],
    from_root: bool,
    stale: &(dyn Fn() -> bool + Sync),
) -> Vec<(PathBuf, Metadata)> {
    let full = words.iter().any(|w| w.contains('/'));
    // (needle, case-sensitive)
    let needles: Vec<(String, bool)> = words
        .iter()
        .map(|w| match w.chars().any(char::is_uppercase) {
            true => (w.to_string(), true),
            false => (w.to_lowercase(), false),
        })
        .collect();
    let hits = Mutex::new(Vec::new());
    let count = AtomicUsize::new(0);

    let mut walker = WalkBuilder::new(base);
    walker.hidden(true).follow_links(false);
    if from_root {
        walker.filter_entry(|e| !SKIP.iter().any(|s| e.path() == Path::new(s)));
    }
    walker.build_parallel().run(|| {
        Box::new(|entry| {
            if stale() || count.load(Ordering::Relaxed) >= LIMIT {
                return WalkState::Quit;
            }
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if entry.depth() == 0 {
                return WalkState::Continue;
            }
            let target = match full {
                true => entry.path().to_string_lossy(),
                false => entry.file_name().to_string_lossy(),
            };
            let lower = target.to_lowercase();
            let matches = needles.iter().all(|(n, case)| match case {
                true => target.contains(n.as_str()),
                false => lower.contains(n.as_str()),
            });
            if matches
                && count.fetch_add(1, Ordering::Relaxed) < LIMIT
                && let Ok(meta) = entry.metadata()
            {
                hits.lock().unwrap().push((entry.into_path(), meta));
            }
            WalkState::Continue
        })
    });
    hits.into_inner().unwrap()
}

fn listing(dir: &Path) -> Vec<(PathBuf, Metadata)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| Some((e.path(), std::fs::symlink_metadata(e.path()).ok()?)))
        .collect()
}

/// ~/.local/share/recently-used.xbel, most recent first, files that still exist.
fn recent(home: &Path) -> Vec<(PathBuf, Metadata)> {
    let Ok(xml) = std::fs::read_to_string(home.join(".local/share/recently-used.xbel")) else {
        return Vec::new();
    };
    let mut marks: Vec<(&str, String)> = xml
        .split("<bookmark ")
        .skip(1)
        .filter_map(|chunk| {
            let tag = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
            let href = attr(tag, "href")?.strip_prefix("file://")?;
            Some((attr(tag, "modified")?, unescape(href)))
        })
        .collect();
    // ISO timestamps, so newest first is reverse string order.
    marks.sort_by(|a, b| b.0.cmp(a.0));
    let mut seen = HashSet::new();
    marks
        .into_iter()
        .filter(|(_, p)| seen.insert(p.clone()))
        .take(RECENT)
        .filter_map(|(_, p)| {
            let path = PathBuf::from(p);
            let meta = std::fs::symlink_metadata(&path).ok()?;
            Some((path, meta))
        })
        .collect()
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut at = 0;
    // The attribute itself, not one whose name ends the same way.
    while let Some(i) = tag[at..].find(&key) {
        let start = at + i;
        if start == 0 || tag.as_bytes()[start - 1].is_ascii_whitespace() {
            let rest = &tag[start + key.len()..];
            return rest.find('"').map(|end| &rest[..end]);
        }
        at = start + key.len();
    }
    None
}

/// XML entities, then %-escapes.
fn unescape(s: &str) -> String {
    let s = s
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbel() {
        let tag = r#"href="file:///home/u/My%20Notes%C3%A5.txt" added="x" modified="2026-10-20T10:00:00Z" visited="y""#;
        assert_eq!(attr(tag, "modified"), Some("2026-10-20T10:00:00Z"));
        assert_eq!(
            unescape(attr(tag, "href").unwrap().strip_prefix("file://").unwrap()),
            "/home/u/My Noteså.txt"
        );
        assert_eq!(unescape("/a/b&amp;c%2"), "/a/b&c%2");
    }

    #[test]
    fn kinds() {
        assert_eq!(kind("photo.jpeg"), "image");
        assert_eq!(kind("justfile"), "code");
        assert_eq!(kind("flake.lock"), "code");
        assert_eq!(kind("readme"), "file");
        assert_eq!(kind("a.tar.gz"), "archive");
    }
}
