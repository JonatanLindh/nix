//! Desktop entries: the apps the launcher lists (Quickshell's `DesktopEntries` in
//! Launcher.qml). Read from `applications/` under the XDG data directories, the user's own
//! first; an entry found earlier hides one with the same id found later, and a hidden
//! (`Hidden=true`) one removes it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A visible app.
#[derive(Debug, Clone, PartialEq)]
pub struct App {
    /// The desktop file id without ".desktop" ("org.kde.dolphin"), which is also what its
    /// launch count is kept under.
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    /// What else it can be found by: generic name, id, keywords.
    pub hay: String,
    /// Shown after the name: the generic name, or the comment.
    pub detail: String,
}

/// The parsed `[Desktop Entry]` group; `None` fields were missing.
#[derive(Debug, Default)]
struct Entry {
    kind: Option<String>,
    name: Option<String>,
    generic_name: Option<String>,
    comment: Option<String>,
    keywords: Vec<String>,
    icon: Option<String>,
    no_display: bool,
    hidden: bool,
}

/// The locale's message names to try, most specific first ("sv_SE", "sv").
fn locales() -> Vec<String> {
    let lang = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    let lang = lang.split(['.', '@']).next().unwrap_or_default();
    if lang.is_empty() || lang == "C" || lang == "POSIX" {
        return Vec::new();
    }
    let mut out = vec![lang.to_string()];
    if let Some((language, _)) = lang.split_once('_') {
        out.push(language.to_string());
    }
    out
}

/// A value with its escapes (\s \n \t \r \\) undone.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// A list value ("a;b;c;"), split on unescaped semicolons.
fn split_list(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    if next == ';' {
                        current.push(';');
                    } else {
                        current.push('\\');
                        current.push(next);
                    }
                }
            }
            ';' => out.push(unescape(&std::mem::take(&mut current))),
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(unescape(&current));
    }
    out.retain(|s| !s.trim().is_empty());
    out
}

fn parse(text: &str, locales: &[String]) -> Entry {
    // (key, locale) -> value, for the main group only.
    let mut values: HashMap<(&str, Option<&str>), &str> = HashMap::new();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let (key, locale) = match key.split_once('[') {
            Some((k, rest)) => (k, Some(rest.trim_end_matches(']'))),
            None => (key, None),
        };
        values.entry((key, locale)).or_insert(value.trim());
    }
    let get = |key: &str| -> Option<&str> {
        locales
            .iter()
            .find_map(|l| values.get(&(key, Some(l.as_str()))).copied())
            .or_else(|| values.get(&(key, None)).copied())
    };
    let string = |key: &str| get(key).map(unescape).filter(|s| !s.is_empty());
    let flag = |key: &str| get(key).is_some_and(|v| v == "true");
    Entry {
        kind: string("Type"),
        name: string("Name"),
        generic_name: string("GenericName"),
        comment: string("Comment"),
        keywords: get("Keywords").map(split_list).unwrap_or_default(),
        icon: string("Icon"),
        no_display: flag("NoDisplay"),
        hidden: flag("Hidden"),
    }
}

/// The app an entry describes, if it's one to list.
fn app(id: String, entry: Entry) -> Option<App> {
    if entry.hidden || entry.no_display || entry.kind.as_deref() != Some("Application") {
        return None;
    }
    let name = entry.name?;
    let generic = entry.generic_name.unwrap_or_default();
    let hay = [generic.clone(), id.clone()]
        .into_iter()
        .chain(entry.keywords)
        .collect::<Vec<_>>()
        .join(" ");
    let detail = if generic.is_empty() {
        entry.comment.unwrap_or_default()
    } else {
        generic
    };
    Some(App {
        id,
        name,
        icon: entry.icon,
        hay,
        detail,
    })
}

/// The data directories to look in, in order of precedence.
fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")));
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    data_home
        .into_iter()
        .chain(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from))
        .collect()
}

/// Every desktop file under `dir`, with its id (subdirectories joined with "-").
fn desktop_files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            desktop_files(&path, &format!("{prefix}{name}-"), out);
        } else if let Some(stem) = name.strip_suffix(".desktop") {
            out.push((format!("{prefix}{stem}"), path));
        }
    }
}

/// Every visible app, in no particular order (the ranking sorts).
pub fn scan() -> Vec<App> {
    let locales = locales();
    let mut seen = std::collections::HashSet::new();
    let mut apps = Vec::new();
    for dir in data_dirs() {
        let mut files = Vec::new();
        desktop_files(&dir.join("applications"), "", &mut files);
        for (id, path) in files {
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = app(id, parse(&text, &locales)) {
                apps.push(app);
            }
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "\
[Desktop Entry]
Type=Application
Name=Files
Name[sv]=Filer
GenericName=File Manager
Comment=Browse files
Keywords=folder;manager\\;explorer;
Icon=org.kde.dolphin
Exec=dolphin %u

[Desktop Action new]
Name=New Window
";

    #[test]
    fn entries() {
        let entry = parse(FILE, &[]);
        let app = app("org.kde.dolphin".into(), entry).unwrap();
        assert_eq!(app.name, "Files");
        assert_eq!(app.detail, "File Manager");
        assert_eq!(app.icon.as_deref(), Some("org.kde.dolphin"));
        assert_eq!(
            app.hay,
            "File Manager org.kde.dolphin folder manager;explorer"
        );

        let swedish = parse(FILE, &["sv_SE".into(), "sv".into()]);
        assert_eq!(swedish.name.as_deref(), Some("Filer"));
    }

    #[test]
    fn hidden() {
        let hidden = parse(
            "[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\n",
            &[],
        );
        assert!(app("x".into(), hidden).is_none());
        let link = parse("[Desktop Entry]\nType=Link\nName=X\n", &[]);
        assert!(app("x".into(), link).is_none());
        let commented = parse(
            "[Desktop Entry]\nType=Application\nName=X\nComment=A\\sb\n",
            &[],
        );
        assert_eq!(app("x".into(), commented).unwrap().detail, "A b");
    }
}
