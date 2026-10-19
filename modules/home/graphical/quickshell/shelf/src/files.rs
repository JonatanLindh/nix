//! File classification and where results go.

use std::{
    env,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Context;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Image,
    Pdf,
    Video,
    Archive,
    Folder,
    Other,
}

/// Compound archive suffixes, longest first so `.tar.gz` wins over `.gz`.
const ARCHIVE_SUFFIXES: &[&str] = &[
    ".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2", ".tgz", ".txz", ".zip", ".tar", ".7z", ".rar",
    ".gz", ".xz", ".zst", ".bz2",
];

pub fn kind_of(file: &str) -> Kind {
    if file.ends_with('/') || Path::new(file).is_dir() {
        return Kind::Folder;
    }
    let lower = file.to_lowercase();
    if ARCHIVE_SUFFIXES.iter().any(|s| lower.ends_with(s)) {
        return Kind::Archive;
    }
    match Path::new(&lower).extension().and_then(|e| e.to_str()) {
        Some(
            "png" | "jpg" | "jpeg" | "webp" | "avif" | "gif" | "heic" | "heif" | "tif" | "tiff"
            | "bmp",
        ) => Kind::Image,
        Some("pdf") => Kind::Pdf,
        Some("mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v") => Kind::Video,
        _ => Kind::Other,
    }
}

/// File name without its extension (and without a compound archive suffix).
pub fn stem(file: &Path) -> String {
    let name = name(file);
    let lower = name.to_lowercase();
    if let Some(s) = ARCHIVE_SUFFIXES.iter().find(|s| lower.ends_with(*s)) {
        return name[..name.len() - s.len()].to_string();
    }
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_string(),
        _ => name,
    }
}

pub fn ext(file: &Path) -> String {
    file.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase()
}

pub fn name(file: &Path) -> String {
    file.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Results land in ~/Downloads/Shelf, where they're easy to find again.
pub fn out_dir() -> anyhow::Result<PathBuf> {
    let home = env::var_os("HOME").context("HOME isn't set")?;
    let dir = PathBuf::from(home).join("Downloads").join("Shelf");
    std::fs::create_dir_all(&dir).with_context(|| format!("Can't create {}", dir.display()))?;
    Ok(dir)
}

/// Scratch space for shares (zips sent by wormhole or served by funnel); gone on logout.
pub fn runtime_dir() -> anyhow::Result<PathBuf> {
    let base = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let dir = base.join("shelf");
    std::fs::create_dir_all(&dir).with_context(|| format!("Can't create {}", dir.display()))?;
    Ok(dir)
}

/// `dir/name`, or `dir/stem (2).ext` etc. if that's taken. Never overwrites.
pub fn unique(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (base, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..)
        .map(|i| dir.join(format!("{base} ({i}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// A short random lowercase token for share paths.
pub fn token() -> String {
    let mut bytes = [0u8; 8];
    // /dev/urandom can't really fail; fall back to the clock rather than erroring.
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_err()
    {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        bytes = (t as u64).to_le_bytes();
    }
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    bytes
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.0} kB", b / 1e3)
    }
}

pub fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(kind_of("/x/a.JPG"), Kind::Image);
        assert_eq!(kind_of("/x/a.tar.gz"), Kind::Archive);
        assert_eq!(kind_of("/x/a.pdf"), Kind::Pdf);
        assert_eq!(kind_of("/x/a.mkv"), Kind::Video);
        assert_eq!(kind_of("/x/a.txt"), Kind::Other);
        assert_eq!(kind_of("/x/dir/"), Kind::Folder);
    }

    #[test]
    fn stems() {
        assert_eq!(stem(Path::new("/x/a.b.png")), "a.b");
        assert_eq!(stem(Path::new("/x/src.tar.gz")), "src");
        assert_eq!(stem(Path::new("/x/.hidden")), ".hidden");
    }

    #[test]
    fn unique_names() {
        let dir = env::temp_dir().join(format!("shelf-test-{}", token()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique(&dir, "a.png"), dir.join("a.png"));
        std::fs::write(dir.join("a.png"), b"").unwrap();
        assert_eq!(unique(&dir, "a.png"), dir.join("a (2).png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
