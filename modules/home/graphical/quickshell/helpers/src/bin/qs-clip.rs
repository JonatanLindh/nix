//! `qs-clip`: cliphist for the launcher's clipboard mode, as JSON on stdout.
//!
//!   qs-clip list DIR       the entries as one line, then {"thumb": id} for each of the newest
//!                          images as it is decoded into DIR (for the preview pane)
//!   qs-clip preview ID DIR {"id", "kind", "meta", "text"} for text (the first 8000 bytes, with
//!                          kind and meta from all of it), {"id", "thumb": true} for an image
//!   qs-clip pick ID [paste]  copy it, and paste it into the focused window (qs-paste)
//!   qs-clip delete LINE    LINE as `cliphist list` printed it
//!   qs-clip wipe

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::json;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const PREVIEW_BYTES: usize = 8000;
const THUMBS: usize = 30;

#[derive(Serialize)]
struct Entry {
    id: String,
    /// The `cliphist list` line, which `cliphist delete` wants back.
    line: String,
    /// What the search matches against.
    text: String,
    title: String,
    kind: &'static str,
    meta: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let done = match args.as_slice() {
        ["list", dir] => list(Path::new(dir)),
        ["preview", id, dir] => preview(id, Path::new(dir)),
        ["pick", id] => pick(id, false),
        ["pick", id, "paste"] => pick(id, true),
        ["delete", line] => delete(line),
        ["wipe"] => cliphist(&["wipe"]).map(drop),
        _ => {
            eprintln!(
                "usage: qs-clip list DIR | preview ID DIR | pick ID [paste] | delete LINE | wipe"
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = done {
        eprintln!("qs-clip: {e:#}");
        std::process::exit(1);
    }
}

fn cliphist(args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("cliphist")
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .context("couldn't run cliphist")?;
    if !out.status.success() {
        bail!("cliphist {} failed", args[0]);
    }
    Ok(out.stdout)
}

fn emit(v: impl Serialize) {
    let mut out = std::io::stdout().lock();
    let _ = serde_json::to_writer(&mut out, &v);
    let _ = writeln!(out);
    let _ = out.flush();
}

// ---- Listing

/// "[[ binary data 1.2 MiB png 1920x1080 ]]" → (size, type, width, height)
fn binary(preview: &str) -> Option<(&str, &str, &str, &str)> {
    let inner = preview
        .strip_prefix("[[ binary data ")?
        .strip_suffix(" ]]")?;
    let mut parts = inner.rsplitn(3, ' ');
    let dims = parts.next()?;
    let mime = parts.next()?;
    let size = parts.next()?;
    let (w, h) = dims.split_once('x')?;
    Some((size, mime, w, h))
}

fn list(dir: &Path) -> Result<()> {
    let out = cliphist(&["list"])?;
    let text = String::from_utf8_lossy(&out);
    let entries: Vec<Entry> = text
        .lines()
        .filter_map(|line| {
            let (id, preview) = line.split_once('\t')?;
            let entry = match binary(preview) {
                Some((size, mime, w, h)) => Entry {
                    text: format!("image {mime} {w}x{h}"),
                    title: format!("Image · {w} × {h}"),
                    kind: "image",
                    meta: format!("{} · {w} × {h} · {size}", mime.to_uppercase()),
                    id: id.into(),
                    line: line.into(),
                },
                None => {
                    let kind = kind(preview);
                    let title = preview.split_whitespace().collect::<Vec<_>>().join(" ");
                    Entry {
                        meta: meta(kind, preview),
                        title: if title.is_empty() {
                            "Whitespace".into()
                        } else {
                            title
                        },
                        text: preview.into(),
                        kind,
                        id: id.into(),
                        line: line.into(),
                    }
                }
            };
            Some(entry)
        })
        .collect();
    emit(json!({ "entries": entries }));

    let images: Vec<&str> = (entries.iter())
        .filter(|e| e.kind == "image")
        .take(THUMBS)
        .map(|e| e.id.as_str())
        .collect();
    for id in images {
        if thumb(id, dir, None).is_ok() {
            emit(json!({ "thumb": id }));
        }
    }
    Ok(())
}

/// Writes an image entry to DIR/ID once (the preview pane shows it from there).
fn thumb(id: &str, dir: &Path, data: Option<&[u8]>) -> Result<()> {
    let path = dir.join(id);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 0) {
        return Ok(());
    }
    let decoded;
    let data = match data {
        Some(d) => d,
        None => {
            decoded = cliphist(&["decode", id])?;
            &decoded
        }
    };
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{id}.tmp"));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// ---- Text

/// A guess at what the text is.
fn kind(text: &str) -> &'static str {
    let t = text.trim();
    let link = (t.starts_with("http://") || t.starts_with("https://") || t.starts_with("www."))
        && !t.chars().any(char::is_whitespace);
    if link {
        return "link";
    }
    const STARTS: &[&str] = &[
        "$ ", "sudo ", "nix ", "git ", "cd ", "just ", "hl.", "import ", "const ", "let ", "def ",
        "fn ",
    ];
    let code = t.lines().any(|l| {
        let end = l.trim_end();
        end.ends_with(['{', '}', ';'])
            || l.contains("=>")
            || l.contains("&&")
            || l.contains("||")
            || STARTS.iter().any(|s| l.trim_start().starts_with(s))
    }) || t
        .lines()
        .skip(1)
        .any(|l| l.starts_with([' ', '\t']) && !l.trim().is_empty());
    if code { "code" } else { "text" }
}

/// "Text · 27 characters", "Code · 5 lines", "Link · github.com".
fn meta(kind: &str, text: &str) -> String {
    if kind == "link" {
        let rest = text.trim();
        let rest = rest
            .strip_prefix("https://")
            .or(rest.strip_prefix("http://"))
            .unwrap_or(rest);
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        return format!("Link · {host}");
    }
    let label = if kind == "code" { "Code" } else { "Text" };
    let lines = text.strip_suffix('\n').unwrap_or(text).split('\n').count();
    if lines > 1 {
        format!("{label} · {lines} lines")
    } else {
        format!("{label} · {} characters", text.chars().count())
    }
}

fn is_image(data: &[u8]) -> bool {
    data.starts_with(b"\x89PNG")
        || data.starts_with(&[0xff, 0xd8, 0xff])
        || data.starts_with(b"GIF8")
        || (data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"))
        || data
            .get(4..12)
            .is_some_and(|b| b.starts_with(b"ftypavif") || b.starts_with(b"ftypheic"))
        || data.starts_with(b"BM")
}

fn preview(id: &str, dir: &Path) -> Result<()> {
    let data = cliphist(&["decode", id])?;
    if is_image(&data) {
        thumb(id, dir, Some(&data))?;
        emit(json!({ "id": id, "thumb": true }));
        return Ok(());
    }
    let full = String::from_utf8_lossy(&data);
    let mut end = full.len().min(PREVIEW_BYTES);
    while !full.is_char_boundary(end) {
        end -= 1;
    }
    let kind = kind(&full);
    emit(json!({ "id": id, "kind": kind, "meta": meta(kind, &full), "text": &full[..end] }));
    Ok(())
}

// ---- Actions

fn pick(id: &str, paste: bool) -> Result<()> {
    let mut decode = Command::new("cliphist")
        .args(["decode", id])
        .stdout(Stdio::piped())
        .spawn()
        .context("couldn't run cliphist")?;
    let copied = Command::new("wl-copy")
        .stdin(decode.stdout.take().unwrap())
        .status()
        .context("couldn't run wl-copy")?;
    if !decode.wait()?.success() || !copied.success() {
        bail!("couldn't copy entry {id}");
    }
    if paste {
        Command::new("qs-paste")
            .status()
            .context("couldn't run qs-paste")?;
    }
    Ok(())
}

fn delete(line: &str) -> Result<()> {
    let mut child = Command::new("cliphist")
        .arg("delete")
        .stdin(Stdio::piped())
        .spawn()
        .context("couldn't run cliphist")?;
    child.stdin.take().unwrap().write_all(line.as_bytes())?;
    if !child.wait()?.success() {
        bail!("cliphist delete failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_lines() {
        assert_eq!(
            binary("[[ binary data 1.2 MiB png 1920x1080 ]]"),
            Some(("1.2 MiB", "png", "1920", "1080"))
        );
        assert_eq!(binary("hello"), None);
    }

    #[test]
    fn kinds() {
        assert_eq!(kind("https://github.com/x"), "link");
        assert_eq!(kind("let x = 1;"), "code");
        assert_eq!(kind("fn main() {\n    x\n}"), "code");
        assert_eq!(kind("some plain words"), "text");
        assert_eq!(meta("link", "https://github.com/a?b"), "Link · github.com");
        assert_eq!(meta("text", "one\ntwo\n"), "Text · 2 lines");
        assert_eq!(meta("text", "åäö"), "Text · 3 characters");
    }
}
