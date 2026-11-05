//! What's kept across restarts: do-not-disturb and the history, as JSON in
//! `$XDG_STATE_HOME/shell/notifications.json`.
//!
//! History entries are written in the shape Notifs.qml keeps its entries in (`hasDefault`,
//! `time` in milliseconds, `icon` a theme name or path, `image` a path). What can't come back is
//! dropped: nothing is still open on the bus after a restart, so restored entries are no longer
//! live (their actions can't be invoked), and pictures sent as pixels rather than a file aren't
//! kept.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};

use super::Entry;
use crate::services::icons;
use crate::services::notifications::{Notification, Picture, Urgency};

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub dnd: bool,
    /// Newest first.
    pub history: Vec<SavedEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedEntry {
    pub id: u32,
    /// Milliseconds since the epoch.
    pub time: i64,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub critical: bool,
    pub actions: Vec<SavedAction>,
    pub has_default: bool,
    pub resident: bool,
    pub held: bool,
    /// The app icon's theme name or path; empty for none.
    pub icon: String,
    /// The picture's path; empty for none.
    pub image: String,
    pub avatar: bool,
}

impl Default for SavedEntry {
    fn default() -> Self {
        Self {
            id: 0,
            time: 0,
            app: "notification".into(),
            summary: String::new(),
            body: String::new(),
            critical: false,
            actions: Vec::new(),
            has_default: false,
            resident: false,
            held: false,
            icon: String::new(),
            image: String::new(),
            avatar: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedAction {
    pub id: String,
    pub text: String,
}

impl SavedEntry {
    /// `None` for what doesn't go to the history (transient ones).
    pub fn from_entry(entry: &Entry) -> Option<Self> {
        let n = &entry.notification;
        if n.transient {
            return None;
        }
        let image = n
            .picture
            .as_ref()
            .and_then(|p| Some((p.path.as_ref()?, p.avatar)));
        Some(Self {
            id: n.id,
            time: entry.time.timestamp_millis(),
            app: n.app.clone(),
            summary: n.summary.clone(),
            body: n.body.clone(),
            critical: n.urgency == Urgency::Critical,
            actions: n
                .actions
                .iter()
                .map(|(id, text)| SavedAction {
                    id: id.clone(),
                    text: text.clone(),
                })
                .collect(),
            has_default: n.has_default,
            resident: n.resident,
            held: entry.held,
            icon: n.icon_name.clone().unwrap_or_default(),
            image: image
                .map(|(p, _)| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            avatar: image.is_none_or(|(_, avatar)| avatar),
        })
    }

    /// Back as a history entry, closed. Finding and decoding its pictures is up to `load`, so
    /// this can run off the main thread.
    pub fn restore(self, load: impl Fn(&str) -> Option<icons::Image>) -> Entry {
        let found = |name: &str| (!name.is_empty()).then(|| load(name)).flatten();
        let icon = found(&self.icon);
        let picture = found(&self.image).map(|image| Picture {
            image,
            avatar: self.avatar,
            path: Some(PathBuf::from(&self.image)),
        });
        Entry {
            notification: Arc::new(Notification {
                id: self.id,
                app: self.app,
                summary: self.summary,
                body: self.body,
                icon,
                icon_name: (!self.icon.is_empty()).then_some(self.icon),
                picture,
                actions: self.actions.into_iter().map(|a| (a.id, a.text)).collect(),
                has_default: self.has_default,
                urgency: if self.critical {
                    Urgency::Critical
                } else {
                    Urgency::Normal
                },
                resident: self.resident,
                transient: false,
                expire_timeout: -1,
            }),
            time: Local
                .timestamp_millis_opt(self.time)
                .single()
                .unwrap_or_else(|| DateTime::<Local>::from(std::time::UNIX_EPOCH)),
            held: self.held,
            live: false,
        }
    }
}

impl Saved {
    pub fn new(dnd: bool, history: &[Entry]) -> Self {
        Self {
            dnd,
            history: history.iter().filter_map(SavedEntry::from_entry).collect(),
        }
    }

    fn path() -> Option<PathBuf> {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
        Some(state.join("shell/notifications.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Writes `json` (from `to_json`) through a temporary file, so a crash mid-write doesn't
    /// lose what was there. Readable by the user only: notifications can say private things.
    pub fn write(json: &str) {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let Some(path) = Self::path() else { return };
        let temp = path.with_extension("json.tmp");
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&temp)
            })
            .and_then(|mut file| file.write_all(json.as_bytes()))
            .and_then(|()| std::fs::rename(&temp, &path));
        if let Err(e) = result {
            eprintln!("notifications: saving {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(id: u32) -> Notification {
        Notification {
            id,
            app: "Signal".into(),
            summary: "Anna".into(),
            body: "See you at 8".into(),
            icon: None,
            icon_name: Some("signal-desktop".into()),
            picture: None,
            actions: vec![("reply".into(), "Reply".into())],
            has_default: true,
            urgency: Urgency::Critical,
            resident: true,
            transient: false,
            expire_timeout: 0,
        }
    }

    fn entry(n: Notification) -> Entry {
        Entry {
            notification: Arc::new(n),
            time: Local.timestamp_millis_opt(1_762_300_000_123).unwrap(),
            held: true,
            live: true,
        }
    }

    #[test]
    fn round_trip() {
        let saved = Saved::new(true, &[entry(notification(7))]);
        let json = saved.to_json();
        let back: Saved = serde_json::from_str(&json).unwrap();
        assert_eq!(back, saved);

        let restored = back.history[0].clone().restore(|_| None);
        let n = &restored.notification;
        assert_eq!(n.id, 7);
        assert_eq!((n.app.as_str(), n.summary.as_str()), ("Signal", "Anna"));
        assert_eq!(n.body, "See you at 8");
        assert_eq!(n.actions, vec![("reply".to_string(), "Reply".to_string())]);
        assert!(n.has_default && n.resident && !n.transient);
        assert_eq!(n.urgency, Urgency::Critical);
        assert_eq!(n.icon_name.as_deref(), Some("signal-desktop"));
        assert_eq!(restored.time.timestamp_millis(), 1_762_300_000_123);
        assert!(restored.held);
        // Nothing's open on the bus any more.
        assert!(!restored.live);
    }

    #[test]
    fn camel_case_like_the_qml() {
        let json = Saved::new(false, &[entry(notification(1))]).to_json();
        assert!(json.contains(r#""hasDefault":true"#), "{json}");
        assert!(json.contains(r#""icon":"signal-desktop""#), "{json}");
        assert!(
            json.contains(r#""actions":[{"id":"reply","text":"Reply"}]"#),
            "{json}"
        );
    }

    #[test]
    fn transient_ones_are_left_out() {
        let mut n = notification(2);
        n.transient = true;
        assert!(Saved::new(false, &[entry(n)]).history.is_empty());
    }

    #[test]
    fn pixel_pictures_are_left_out() {
        let mut n = notification(3);
        let pixels = icons::raster(1, 1, vec![0; 4]).unwrap();
        n.picture = Some(Picture {
            image: icons::Image::Raster(pixels),
            avatar: true,
            path: None,
        });
        let saved = SavedEntry::from_entry(&entry(n)).unwrap();
        assert_eq!(saved.image, "");
        // Only the icon is looked for.
        let restored = saved.restore(|name| {
            assert_eq!(name, "signal-desktop");
            None
        });
        assert!(restored.notification.picture.is_none());
    }

    #[test]
    fn pictures_from_files_come_back() {
        let mut n = notification(4);
        n.picture = Some(Picture {
            image: icons::Image::Svg("/tmp/shot.svg".into()),
            avatar: false,
            path: Some("/tmp/shot.svg".into()),
        });
        let saved = SavedEntry::from_entry(&entry(n)).unwrap();
        assert_eq!(
            (saved.image.as_str(), saved.avatar),
            ("/tmp/shot.svg", false)
        );
        let restored = saved.restore(|p| Some(icons::Image::Svg(p.into())));
        let picture = restored.notification.picture.clone().unwrap();
        assert_eq!(
            picture.path.as_deref(),
            Some(std::path::Path::new("/tmp/shot.svg"))
        );
        assert!(!picture.avatar);
        assert!(restored.notification.icon.is_some());
    }

    #[test]
    fn old_files_and_missing_fields() {
        // Before the history was kept.
        let old: Saved = serde_json::from_str(r#"{"dnd":true}"#).unwrap();
        assert!(old.dnd && old.history.is_empty());
        let sparse: Saved =
            serde_json::from_str(r#"{"history":[{"id":5,"summary":"Hi"}]}"#).unwrap();
        let e = sparse.history[0].clone().restore(|_| None);
        assert_eq!(
            (e.notification.id, e.notification.app.as_str()),
            (5, "notification")
        );
        assert_eq!(e.notification.urgency, Urgency::Normal);
    }
}
