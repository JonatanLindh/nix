//! Pasting onto the shelf: what's on the clipboard (or dropped text) becomes one of the
//! shelf's own files. A web address becomes a link (`.url`), other text a note (`.txt`), an
//! image an image file. Copied files aren't pasted here; the CLI adds them as they are.

use std::path::Path;

use anyhow::bail;

use crate::{
    files::{results_dir, unique},
    tools::Outcome,
};

/// Saves clipboard data of type `mime` as a result.
pub fn save(mime: &str, data: &[u8]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
    if let Some(ext) = image_ext(mime) {
        if data.is_empty() {
            bail!("The clipboard is empty");
        }
        let path = unique(&dir, &format!("Pasted image.{ext}"));
        std::fs::write(&path, data)?;
        return Ok(Outcome::made("Pasted an image", vec![path]));
    }
    let text = String::from_utf8_lossy(data);
    let text = text.trim();
    if text.is_empty() {
        bail!("The clipboard is empty");
    }
    if let Some(url) = web_address(text) {
        let path = unique(&dir, &format!("{}.url", file_safe(host(url))));
        std::fs::write(&path, format!("[InternetShortcut]\r\nURL={url}\r\n"))?;
        return Ok(Outcome::made("Pasted a link", vec![path]));
    }
    let path = unique(&dir, &format!("{}.txt", title(text)));
    std::fs::write(&path, format!("{text}\n"))?;
    Ok(Outcome::made("Pasted text", vec![path]))
}

/// The clipboard type to paste, from `wl-paste --list-types`: an image, else text.
pub fn pick_type(types: &[&str]) -> Option<String> {
    let image = types
        .iter()
        .find(|t| **t == "image/png")
        .or_else(|| types.iter().find(|t| image_ext(t).is_some()));
    let text = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING"]
        .iter()
        .find(|want| types.contains(want));
    image.or(text).map(|t| t.to_string())
}

fn image_ext(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        _ => return None,
    })
}

/// `text` if it's a single http(s) address and nothing else.
fn web_address(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase();
    let web = lower.starts_with("https://") || lower.starts_with("http://");
    (web && !text.chars().any(char::is_whitespace)).then_some(text)
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit('@').next().unwrap_or(host);
    host.strip_prefix("www.").unwrap_or(host)
}

/// A note's file name: its first line, cut at a word to about 40 characters.
fn title(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    let mut out = String::new();
    for word in first.split_whitespace() {
        let next = if out.is_empty() {
            word.to_string()
        } else {
            format!("{out} {word}")
        };
        if next.chars().count() > 40 {
            if out.is_empty() {
                out = word.chars().take(40).collect();
            }
            break;
        }
        out = next;
    }
    let safe = file_safe(&out);
    if safe.is_empty() { "Note".into() } else { safe }
}

/// Without characters that are awkward (or impossible) in a file name.
fn file_safe(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    // No hidden files or `..`.
    cleaned.trim().trim_start_matches('.').trim().to_string()
}

/// `file://` paths in a `text/uri-list`, if that's all it holds.
pub fn file_paths(uri_list: &str) -> Option<Vec<String>> {
    let uris: Vec<&str> = uri_list
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if uris.is_empty() {
        return None;
    }
    uris.iter()
        .map(|u| {
            let rest = u.strip_prefix("file://")?;
            // file://host/path: only this machine's.
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            if !path.starts_with('/') {
                return None;
            }
            let path = percent_decode(path)?;
            Path::new(&path).is_absolute().then_some(path)
        })
        .collect()
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(web_address("https://x.org/a?b"), Some("https://x.org/a?b"));
        assert_eq!(web_address("see https://x.org"), None);
        assert_eq!(web_address("ftp://x.org"), None);
        assert_eq!(host("https://user@www.reddit.com:443/r/a"), "reddit.com:443");
        assert_eq!(file_safe(host("https://www.reddit.com:443/r")), "reddit.com-443");
    }

    #[test]
    fn titles() {
        assert_eq!(title("Buy milk\nand eggs"), "Buy milk");
        assert_eq!(
            title("the quick brown fox jumps over the lazy dog again and again"),
            "the quick brown fox jumps over the lazy"
        );
        assert_eq!(title("../../etc/passwd"), "-..-etc-passwd");
        assert_eq!(title("..."), "Note");
    }

    #[test]
    fn types() {
        assert_eq!(
            pick_type(&["text/html", "image/png", "text/plain"]).as_deref(),
            Some("image/png")
        );
        assert_eq!(
            pick_type(&["text/html", "UTF8_STRING", "text/plain"]).as_deref(),
            Some("text/plain")
        );
        assert_eq!(pick_type(&["application/x-thing"]), None);
    }

    #[test]
    fn uri_lists() {
        assert_eq!(
            file_paths("file:///home/me/a%20b.png\r\nfile://localhost/c\r\n"),
            Some(vec!["/home/me/a b.png".into(), "/c".into()])
        );
        assert_eq!(file_paths("file://elsewhere/c"), None);
        assert_eq!(file_paths("https://x.org\r\n"), None);
    }
}
