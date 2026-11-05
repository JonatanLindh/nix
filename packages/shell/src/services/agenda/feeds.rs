//! The iCal feeds listed in $XDG_CONFIG_HOME/quickshell-calendar/urls (one per line, e.g.
//! Google Calendar's "secret address in iCal format"), with the last good copy of each kept in
//! $XDG_CACHE_HOME/quickshell-calendar so the calendar also works offline. Ported from
//! `qs-calendar-events` (calendar/ in ~/nix), whose files these are.
//!
//! The URLs are secrets: they never go into a message, which names calendars by position.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDateTime};
use sha2::{Digest, Sha256};

use super::Event;
use super::ical;

/// Everything read from the feeds, unsorted.
pub struct Read {
    pub events: Vec<Event>,
    /// "calendar 2: timed out, showing copy from 05 nov 14:30"
    pub errors: Vec<String>,
    /// There's a list of feeds to read.
    pub configured: bool,
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var).filter(|v| !v.is_empty()) {
        Some(dir) => dir.into(),
        None => std::env::home_dir().unwrap_or_default().join(fallback),
    }
}

/// The feeds' addresses, leaving out blank lines and #comments. None listed if the file isn't
/// there.
fn read_urls(path: &Path) -> std::io::Result<Vec<String>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    Ok(parse_urls(&text))
}

fn parse_urls(text: &str) -> Vec<String> {
    (text.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// Where the copy of feed `url` is kept: named by a hash, so not by the secret.
fn cache_file(dir: &Path, url: &str) -> PathBuf {
    let hash: String = Sha256::digest(url.as_bytes())[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    dir.join(format!("{hash}.ics"))
}

/// An io error in a few words, without the "(os error N)" (or a file name): "connection
/// refused", "name or service not known".
fn io_message(e: &std::io::Error) -> String {
    let text = e.to_string();
    let text = match text.find(" (os error") {
        Some(at) => &text[..at],
        None => &text,
    };
    let text = text
        .strip_prefix("failed to lookup address information: ")
        .unwrap_or(text);
    text.to_lowercase()
}

/// What went wrong fetching `url`, never with `url` in it.
fn describe(e: &ureq::Error, url: &str) -> String {
    let message = match e {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        ureq::Error::Timeout(_) => "timed out".into(),
        ureq::Error::HostNotFound => "host not found".into(),
        ureq::Error::Io(e) if e.kind() == std::io::ErrorKind::TimedOut => "timed out".into(),
        ureq::Error::Io(e) => io_message(e),
        e => e.to_string(),
    };
    // Belt and braces: never leak the secret address through an error message.
    message.replace(url, "<url>")
}

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent.get(url).call().map_err(|e| describe(&e, url))?;
    let data = (response.body_mut().with_config())
        .limit(256 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| describe(&e, url))?;
    if !data.trim_ascii_start().starts_with(b"BEGIN:VCALENDAR") {
        return Err("not an iCal feed".into());
    }
    Ok(data)
}

/// Feed `url`, or the copy of it from the last time, with what went wrong.
fn fetch(agent: &ureq::Agent, url: &str, cache: &Path) -> (Option<Vec<u8>>, Option<String>) {
    match download(agent, url) {
        Ok(data) => {
            // Written aside and moved into place, so a copy is always whole.
            let tmp = cache.with_extension("tmp");
            if let Err(e) = std::fs::write(&tmp, &data).and_then(|()| std::fs::rename(&tmp, cache))
            {
                eprintln!("calendar: can't keep a copy: {}", io_message(&e));
            }
            (Some(data), None)
        }
        Err(problem) => match (std::fs::read(cache), std::fs::metadata(cache)) {
            (Ok(data), Ok(meta)) => {
                let stamp = meta
                    .modified()
                    .map(|t| DateTime::<Local>::from(t).format("%d %b %H:%M").to_string())
                    .unwrap_or_default()
                    .to_lowercase();
                (
                    Some(data),
                    Some(format!("{problem}, showing copy from {stamp}")),
                )
            }
            _ => (None, Some(problem)),
        },
    }
}

/// Reads every feed and its events touching `from`..`to` (local). Blocking: network and files.
/// `Err` only if the list of feeds can't be read.
pub fn read(from: NaiveDateTime, to: NaiveDateTime) -> Result<Read, String> {
    let urls_file = xdg("XDG_CONFIG_HOME", ".config").join("quickshell-calendar/urls");
    let urls = read_urls(&urls_file)
        .map_err(|e| format!("can't read the list of calendars: {}", io_message(&e)))?;
    let cache_dir = xdg("XDG_CACHE_HOME", ".cache").join("quickshell-calendar");
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        eprintln!("calendar: {}: {}", cache_dir.display(), io_message(&e));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(30)))
        .user_agent("shell-calendar")
        .build()
        .into();

    let mut read = Read {
        events: Vec::new(),
        errors: Vec::new(),
        configured: !urls.is_empty(),
    };
    for (i, url) in urls.iter().enumerate() {
        let n = i + 1;
        let (data, problem) = fetch(&agent, url, &cache_file(&cache_dir, url));
        if let Some(problem) = problem {
            read.errors.push(format!("calendar {n}: {problem}"));
        }
        let Some(data) = data else {
            continue;
        };
        match ical::parse(&String::from_utf8_lossy(&data)) {
            Ok(calendar) => {
                let name = (calendar.name.clone()).unwrap_or_else(|| format!("calendar {n}"));
                read.events.extend(calendar.events(from, to, &name));
            }
            Err(e) => (read.errors).push(format!(
                "calendar {n}: can't read feed ({})",
                e.replace(url.as_str(), "<url>")
            )),
        }
    }
    Ok(read)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_file() {
        let urls = parse_urls("# mine\n\n  https://a.example/x.ics  \n#https://off\nhttps://b\n");
        assert_eq!(urls, ["https://a.example/x.ics", "https://b"]);
    }

    #[test]
    fn cache_names() {
        // As qs-calendar-events named them: sha256(url)[:16].ics, so its copies are found.
        // printf %s https://a | sha256sum
        assert_eq!(
            cache_file(Path::new("/c"), "https://a"),
            Path::new("/c/44f833239a3830c0.ics")
        );
    }

    #[test]
    fn messages_keep_the_address_out() {
        let url = "https://calendar.example/secret/basic.ics";
        assert_eq!(describe(&ureq::Error::StatusCode(404), url), "HTTP 404");
        let io = std::io::Error::from_raw_os_error(111);
        assert_eq!(describe(&ureq::Error::Io(io), url), "connection refused");
        let bad = ureq::Error::BadUri(format!("bad {url}"));
        assert!(!describe(&bad, url).contains("secret"));
    }
}
