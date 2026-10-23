//! Which actions apply to a set of files. An action is offered only if every file fits it.

use serde::Serialize;

use crate::files::{Kind, in_dir, kind_of, results_path};

#[derive(Serialize, Debug)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    /// Name in the shell's Icons.qml.
    pub icon: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Options>,
}

/// A choice made before running (convert to, resize to). Taildrop's peers are listed
/// separately since they're live (`shelf peers`).
#[derive(Serialize, Debug)]
pub struct Options {
    pub label: &'static str,
    pub items: Vec<Choice>,
}

#[derive(Serialize, Debug)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
}

const fn action(id: &'static str, label: &'static str, icon: &'static str) -> Action {
    Action {
        id,
        label,
        icon,
        options: None,
    }
}

fn choices(label: &'static str, items: &[(&'static str, &'static str)]) -> Option<Options> {
    Some(Options {
        label,
        items: items
            .iter()
            .map(|&(id, label)| Choice { id, label })
            .collect(),
    })
}

pub const CONVERT_FORMATS: &[(&str, &str)] = &[
    ("png", "PNG"),
    ("jpg", "JPG"),
    ("webp", "WebP"),
    ("avif", "AVIF"),
];
pub const RESIZE_SIZES: &[(&str, &str)] = &[
    ("50%", "50%"),
    ("1920", "1920 px"),
    ("1280", "1280 px"),
    ("800", "800 px"),
];

pub fn for_files(files: &[String]) -> Vec<Action> {
    let n = files.len();
    if n == 0 {
        return Vec::new();
    }
    let kinds: Vec<Kind> = files.iter().map(|f| kind_of(f)).collect();
    let all = |k: Kind| kinds.iter().all(|&x| x == k);
    let mut out = Vec::new();

    if n == 1 {
        out.push(action("open", "Open", "open"));
    }
    if all(Kind::Link) {
        out.push(action(
            "copy-url",
            if n == 1 { "Copy URL" } else { "Copy URLs" },
            "copy",
        ));
        if n == 1 {
            out.push(action("qr", "QR code", "qr"));
        }
    }
    if all(Kind::Image) {
        out.push(Action {
            options: choices("Convert to", CONVERT_FORMATS),
            ..action("convert", "Convert", "convert")
        });
        out.push(Action {
            options: choices("Resize to", RESIZE_SIZES),
            ..action("resize", "Resize", "resize")
        });
        out.push(action("strip", "Strip metadata", "eye-off"));
        out.push(action("ocr", "Copy text", "ocr"));
    }
    if all(Kind::Pdf) {
        if n >= 2 {
            out.push(action("merge", "Merge PDFs", "merge"));
        }
        out.push(action("compress", "Compress", "compress"));
        out.push(action("split", "Split pages", "scissors"));
    }
    if all(Kind::Video) {
        out.push(action("shrink", "Shrink for chat", "video-shrink"));
        out.push(action("gif", "Make GIF", "gif"));
    }
    if all(Kind::Archive) {
        out.push(action("extract", "Extract", "extract"));
    }
    if n >= 2 {
        out.push(action("archive", "Archive", "archive"));
    }
    // Hands the files to the shelf: they become temp, deleted when they leave it.
    let results = results_path().ok();
    if !files.iter().any(|f| {
        results
            .as_deref()
            .is_some_and(|d| in_dir(f.trim_end_matches('/'), d))
    }) {
        out.push(action("hand", "Hand to shelf", "inbox"));
    }
    // A link's file is only its address: sharing it is Taildrop (or the QR), not a public
    // link to a file.
    if !all(Kind::Link) {
        out.push(action("funnel", "Funnel link", "link"));
        out.push(action("wormhole", "Wormhole", "wormhole"));
        out.push(action("localsend", "LocalSend", "send"));
    }
    out.push(action("taildrop", "Taildrop", "tailscale"));
    if all(Kind::Link) {
        return out;
    }
    out.push(action(
        "copy-paths",
        if n == 1 { "Copy path" } else { "Copy paths" },
        "copy",
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(files: &[&str]) -> Vec<&'static str> {
        let files: Vec<String> = files.iter().map(|s| s.to_string()).collect();
        for_files(&files).into_iter().map(|a| a.id).collect()
    }

    #[test]
    fn nothing_for_no_files() {
        assert!(ids(&[]).is_empty());
    }

    #[test]
    fn one_image() {
        assert_eq!(
            ids(&["/a/shot.PNG"]),
            [
                "open",
                "convert",
                "resize",
                "strip",
                "ocr",
                "hand",
                "funnel",
                "wormhole",
                "localsend",
                "taildrop",
                "copy-paths"
            ]
        );
    }

    #[test]
    fn one_link() {
        let own = results_path().unwrap().join("43d7.url");
        assert_eq!(
            ids(&[own.to_str().unwrap()]),
            ["open", "copy-url", "qr", "taildrop"]
        );
    }

    #[test]
    fn merge_needs_two_pdfs() {
        assert!(!ids(&["/a.pdf"]).contains(&"merge"));
        assert!(!ids(&["/a.pdf", "/b.pdf"]).contains(&"open"));
        let two = ids(&["/a.pdf", "/b.pdf"]);
        assert!(two.contains(&"merge") && two.contains(&"archive"));
    }

    #[test]
    fn mixed_files_only_get_generic_actions() {
        let mixed = ids(&["/a.pdf", "/b.png"]);
        assert!(!mixed.contains(&"convert") && !mixed.contains(&"compress"));
        assert!(mixed.contains(&"archive"));
    }

    #[test]
    fn no_hand_for_the_shelfs_own_files() {
        let own = results_path().unwrap().join("a.png");
        assert!(!ids(&[own.to_str().unwrap()]).contains(&"hand"));
    }

    #[test]
    fn copy_label_is_plural_for_several() {
        let files = vec!["/a".to_string(), "/b".to_string()];
        let last = for_files(&files).pop().map(|a| a.label);
        assert_eq!(last, Some("Copy paths"));
    }
}
