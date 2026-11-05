//! Clipboard history from cliphist, as Clip.qml had it: the data behind the launcher's clipboard
//! mode. Ported from `qs-clip` (helpers/ in ~/nix), which the shell used to run for this; it
//! still runs `cliphist` and `wl-copy`.
//!
//! - [`list`]: the entries, then each of the newest images as it is decoded into a directory
//!   (for the preview pane)
//! - [`preview`]: the first 8000 bytes of an entry's text, with kind and meta from all of it, or
//!   an image decoded into that directory
//! - [`pick`]: copy an entry, and paste it into the focused window
//! - [`delete`], [`wipe`]

use std::path::{Path, PathBuf};
use std::process::Stdio;

use futures::SinkExt;
use futures::channel::mpsc;
use tokio::io::AsyncWriteExt;

const PREVIEW_BYTES: usize = 8000;
/// How many of the newest images get a thumbnail up front.
const THUMBS: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    /// The `cliphist list` line, which `cliphist delete` wants back.
    pub line: String,
    /// What the search matches against: a one-line preview.
    pub text: String,
    pub title: String,
    /// "text", "link", "code" or "image"; also its icon.
    pub kind: String,
    /// "Text · 27 characters", "Link · github.com", "PNG · 1920 × 1080 · 2 MiB".
    pub meta: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Listed {
    Entries(Vec<Entry>),
    /// An image's thumbnail is in the directory now, named by its id.
    Thumb(String),
}

/// What the preview pane needs for an entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Preview {
    /// The full text (up to 8000 bytes); none for images.
    pub text: Option<String>,
    /// From the full text, where the listing only saw a line.
    pub meta: Option<String>,
    pub thumb: bool,
}

/// Where image entries are decoded for the preview pane.
pub fn thumb_dir() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(runtime).join("shell-clip")
}

/// The thumbnail file of image entry `id`.
pub fn thumb(dir: &Path, id: &str) -> PathBuf {
    dir.join(id)
}

/// Runs cliphist for its output, logging a failure.
async fn cliphist(args: &[&str]) -> Option<Vec<u8>> {
    let output = tokio::process::Command::new("cliphist")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await;
    match output {
        Ok(o) if o.status.success() => Some(o.stdout),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("cliphist {}: {}", args[0], stderr.trim());
            None
        }
        Err(e) => {
            eprintln!("cliphist: {e}");
            None
        }
    }
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

/// One line of `cliphist list`: "ID\tPREVIEW".
fn parse_line(line: &str) -> Option<Entry> {
    let (id, preview) = line.split_once('\t')?;
    let entry = match binary(preview) {
        Some((size, mime, w, h)) => Entry {
            text: format!("image {mime} {w}x{h}"),
            title: format!("Image · {w} × {h}"),
            kind: "image".into(),
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
                kind: kind.into(),
                id: id.into(),
                line: line.into(),
            }
        }
    };
    Some(entry)
}

/// The entries, then each thumbnail as it's written into `dir`.
pub fn list(dir: PathBuf) -> mpsc::Receiver<Listed> {
    crate::runtime::stream(64, async move |mut out| {
        let Some(listing) = cliphist(&["list"]).await else {
            return;
        };
        let entries: Vec<Entry> = String::from_utf8_lossy(&listing)
            .lines()
            .filter_map(parse_line)
            .collect();
        let images: Vec<String> = (entries.iter())
            .filter(|e| e.kind == "image")
            .take(THUMBS)
            .map(|e| e.id.clone())
            .collect();
        if out.send(Listed::Entries(entries)).await.is_err() {
            return;
        }
        for id in images {
            if write_thumb(&id, &dir, None).await.is_ok()
                && out.send(Listed::Thumb(id)).await.is_err()
            {
                break;
            }
        }
    })
}

/// Writes image entry `id` to `dir`/`id` once (the preview pane shows it from there), decoding
/// it unless it's `data` already.
async fn write_thumb(id: &str, dir: &Path, data: Option<&[u8]>) -> std::io::Result<()> {
    let path = thumb(dir, id);
    if tokio::fs::metadata(&path).await.is_ok_and(|m| m.len() > 0) {
        return Ok(());
    }
    let decoded;
    let data = match data {
        Some(d) => d,
        None => {
            decoded = cliphist(&["decode", id])
                .await
                .ok_or_else(|| std::io::Error::other("cliphist decode failed"))?;
            &decoded
        }
    };
    tokio::fs::create_dir_all(dir).await?;
    // Written aside and moved into place, so the preview never shows half an image.
    let tmp = dir.join(format!(".{id}.tmp"));
    tokio::fs::write(&tmp, data).await?;
    tokio::fs::rename(&tmp, &path).await
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

/// The preview of decoded text entry `data`: its start, with meta from all of it.
fn text_preview(data: &[u8]) -> Preview {
    let full = String::from_utf8_lossy(data);
    let mut end = full.len().min(PREVIEW_BYTES);
    while !full.is_char_boundary(end) {
        end -= 1;
    }
    let kind = kind(&full);
    Preview {
        text: Some(full[..end].to_string()),
        meta: Some(meta(kind, &full)),
        thumb: false,
    }
}

/// What the preview pane shows for entry `id`; an image is decoded into `dir`. Entries that fail
/// to decode count as done, with nothing to show.
pub async fn preview(id: String, dir: PathBuf) -> Preview {
    let Some(data) = cliphist(&["decode", &id]).await else {
        return Preview::default();
    };
    if is_image(&data) {
        return Preview {
            thumb: write_thumb(&id, &dir, Some(&data)).await.is_ok(),
            ..Preview::default()
        };
    }
    text_preview(&data)
}

// ---- Actions

/// Copies the entry, and with `paste` pastes it into the window that gets focus back.
pub async fn pick(id: String, paste: bool) {
    let Some(data) = cliphist(&["decode", &id]).await else {
        return;
    };
    match wl_copy(&[], &data).await {
        Ok(()) if paste => crate::services::hyprland::paste().await,
        Ok(()) => {}
        Err(e) => eprintln!("wl-copy: {e}"),
    }
}

/// Puts `data` on the clipboard through `wl-copy ARGS`, from its stdin. wl-copy stays behind to
/// serve the clipboard, so none of its output is piped back (that would never reach EOF).
pub(super) async fn wl_copy(args: &[&str], data: &[u8]) -> std::io::Result<()> {
    let mut child = tokio::process::Command::new("wl-copy")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(data).await?;
        // Dropped here: EOF tells wl-copy that's all.
    }
    let status = child.wait().await?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("wl-copy: {status}")))
    }
}

/// Deletes the entry `line` as `cliphist list` printed it.
pub async fn delete(line: String) {
    let done = async {
        let mut child = tokio::process::Command::new("cliphist")
            .arg("delete")
            .stdin(Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(line.as_bytes()).await?;
        }
        child.wait().await
    };
    match done.await {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("cliphist delete: {s}"),
        Err(e) => eprintln!("cliphist delete: {e}"),
    }
}

pub async fn wipe() {
    cliphist(&["wipe"]).await;
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

        let image = parse_line("40\t[[ binary data 2 MiB png 1920x1080 ]]").unwrap();
        assert_eq!(image.id, "40");
        assert_eq!(image.kind, "image");
        assert_eq!(image.title, "Image · 1920 × 1080");
        assert_eq!(image.text, "image png 1920x1080");
        assert_eq!(image.meta, "PNG · 1920 × 1080 · 2 MiB");
        assert_eq!(image.line, "40\t[[ binary data 2 MiB png 1920x1080 ]]");

        let text = parse_line("12\thello   there").unwrap();
        assert_eq!(text.id, "12");
        assert_eq!(text.kind, "text");
        assert_eq!(text.title, "hello there");
        assert_eq!(text.text, "hello   there");
        assert_eq!(text.meta, "Text · 13 characters");
        assert_eq!(text.line, "12\thello   there");

        // Tabs after the first belong to the preview.
        assert_eq!(parse_line("7\ta\tb").unwrap().text, "a\tb");
        assert_eq!(parse_line("13\t  ").unwrap().title, "Whitespace");
        assert_eq!(
            parse_line("3\thttps://github.com/x").unwrap().meta,
            "Link · github.com"
        );
        assert_eq!(parse_line("no tab"), None);
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

    #[test]
    fn previews() {
        let code = text_preview(b"fn main() {\n    x\n}\n");
        assert_eq!(code.text.as_deref(), Some("fn main() {\n    x\n}\n"));
        assert_eq!(code.meta.as_deref(), Some("Code · 3 lines"));
        assert!(!code.thumb);

        // Cut at 8000 bytes, on a character boundary; meta from all of it.
        let long = "å".repeat(5000);
        let cut = text_preview(long.as_bytes());
        assert_eq!(cut.text.as_ref().map(String::len), Some(8000));
        assert_eq!(cut.meta.as_deref(), Some("Text · 5000 characters"));
        let odd = format!("a{long}");
        assert_eq!(text_preview(odd.as_bytes()).text.unwrap().len(), 7999);

        assert!(is_image(b"\x89PNG\r\n\x1a\n...."));
        assert!(is_image(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(is_image(b"\0\0\0\x1cftypavif"));
        assert!(!is_image(b"plain text"));
    }
}
