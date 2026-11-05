//! The notification daemon: `org.freedesktop.Notifications` on the session bus.
//!
//! Each notification goes to the app as an [`Event`]; the app reports back what happened to it
//! ([`closed`], [`invoked`]), which this emits as the spec's signals.
//!
//! Only one process can own the name. `SHELL_NOTIFY_NAME` serves under another one, to run a
//! development copy beside the real daemon.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use futures::channel::mpsc::{self, UnboundedSender};
use futures::{SinkExt, StreamExt};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

use super::Result;
use super::icons::{self, Image};

const PATH: &str = "/org/freedesktop/Notifications";

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    pub app: String,
    pub summary: String,
    /// Plain text: markup is advertised (so clients send it predictably) and then flattened.
    pub body: String,
    pub icon: Option<Image>,
    /// The theme name or path `icon` was found by, to find it again after a restart.
    pub icon_name: Option<String>,
    pub picture: Option<Picture>,
    /// `(key, label)`, without the default action.
    pub actions: Vec<(String, String)>,
    /// Clicking the notification invokes "default".
    pub has_default: bool,
    pub urgency: Urgency,
    /// Stays after an action is invoked.
    pub resident: bool,
    /// Never goes to the history.
    pub transient: bool,
    /// What the app asked for, in milliseconds: -1 the server's choice, 0 never.
    pub expire_timeout: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

/// The notification's picture. Small, squarish ones are avatars and take the icon's place;
/// others are shown under the text.
#[derive(Debug, Clone)]
pub struct Picture {
    pub image: Image,
    pub avatar: bool,
    /// The file it was loaded from; `None` for pixels sent with the notification.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum Event {
    /// A new notification, or an update to one (same id).
    Notify(std::sync::Arc<Notification>),
    /// The app closed it.
    Closed(u32),
    /// Another daemon owns the name.
    Unavailable,
}

/// Why a notification closed, as the spec numbers them.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Closed by call is the server's own (in CloseNotification).
pub enum Reason {
    Expired = 1,
    Dismissed = 2,
    ClosedByCall = 3,
}

static CONNECTION: Mutex<Option<zbus::Connection>> = Mutex::new(None);
/// The next new notification's id. Shared by every run of the server, so a restarted one
/// doesn't hand out an id that's still in use.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// New notifications get ids above `id`: it belongs to one restored from an earlier run, which
/// a new one mustn't be mistaken for an update to.
pub fn skip_ids(id: u32) {
    NEXT_ID.fetch_max(id.saturating_add(1), Ordering::Relaxed);
}

pub fn watch() -> futures::channel::mpsc::Receiver<Event> {
    crate::runtime::stream(16, async |out| {
        super::supervise("notifications", Duration::from_secs(10), out, serve).await
    })
}

async fn serve(mut out: futures::channel::mpsc::Sender<Event>) -> Result {
    let name = std::env::var("SHELL_NOTIFY_NAME")
        .unwrap_or_else(|_| "org.freedesktop.Notifications".into());
    let (tx, mut rx) = mpsc::unbounded();
    let server = Server { tx };
    let connection = zbus::connection::Builder::session()?
        .serve_at(PATH, server)?
        .build()
        .await?;
    if let Err(e) = connection
        .request_name_with_flags(
            name.as_str(),
            zbus::fdo::RequestNameFlags::DoNotQueue.into(),
        )
        .await
    {
        out.send(Event::Unavailable).await?;
        return Err(format!("can't own {name}: {e}").into());
    }
    *CONNECTION.lock().unwrap() = Some(connection);
    while let Some(event) = rx.next().await {
        out.send(event).await?;
    }
    Ok(())
}

/// Tells the app that sent notification `id` it closed, and why.
pub async fn closed(id: u32, reason: Reason) {
    emit(|e| async move { Server::notification_closed(&e, id, reason as u32).await }).await;
}

/// Tells the app the user picked action `key` (then, unless it's resident, closes it).
pub async fn invoked(id: u32, key: String) {
    emit(|e| async move { Server::action_invoked(&e, id, &key).await }).await;
}

async fn emit<F, Fut>(f: F)
where
    F: FnOnce(SignalEmitter<'static>) -> Fut,
    Fut: Future<Output = zbus::Result<()>>,
{
    let Some(connection) = CONNECTION.lock().unwrap().clone() else {
        return;
    };
    let result = match SignalEmitter::new(&connection, PATH) {
        Ok(emitter) => f(emitter.into_owned()).await,
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        eprintln!("notifications: {e}");
    }
}

struct Server {
    tx: UnboundedSender<Event>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&'static str> {
        vec![
            "actions",
            "body",
            "body-markup",
            "icon-static",
            "persistence",
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id > 0 {
            replaces_id
        } else {
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        };
        let hint = |key: &str| hints.get(key);
        let string = |key: &str| {
            hint(key)
                .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
                .filter(|s| !s.is_empty())
        };
        let flag = |key: &str| hint(key).and_then(|v| bool::try_from(v).ok()) == Some(true);

        let urgency = match hint("urgency").and_then(|v| u8::try_from(v).ok()) {
            Some(0) => Urgency::Low,
            Some(2) => Urgency::Critical,
            _ => Urgency::Normal,
        };
        // The icon: the app's own, else its desktop entry's (usually named the same).
        let (icon_name, icon) = [
            Some(app_icon).filter(|s| !s.is_empty()),
            string("desktop-entry"),
        ]
        .into_iter()
        .flatten()
        .find_map(|name| Some((name.clone(), icons::by_name(&name, None)?)))
        .unzip();
        let picture = ["image-data", "image_data", "icon_data"]
            .into_iter()
            .find_map(|key| hint(key).and_then(image_data))
            .or_else(|| {
                let path = string("image-path").or_else(|| string("image_path"))?;
                let path = icons::path_for(&path, None)?;
                let image = icons::load(&path)?;
                // Size from the file when it's a raster; SVGs count as avatars.
                let avatar =
                    ::image::image_dimensions(&path).map_or(true, |(w, h)| is_avatar(w, h));
                Some(Picture {
                    image,
                    avatar,
                    path: Some(path),
                })
            });

        let pairs: Vec<(String, String)> = actions
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| (p[0].clone(), p[1].clone()))
            .collect();
        let notification = Notification {
            id,
            app: if app_name.is_empty() {
                "notification".into()
            } else {
                app_name
            },
            summary: plain(&summary),
            body: plain(&body),
            icon,
            icon_name,
            picture,
            has_default: pairs.iter().any(|(k, _)| k == "default"),
            actions: pairs.into_iter().filter(|(k, _)| k != "default").collect(),
            urgency,
            resident: flag("resident"),
            transient: flag("transient"),
            expire_timeout,
        };
        let _ = self
            .tx
            .unbounded_send(Event::Notify(std::sync::Arc::new(notification)));
        id
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let _ = self.tx.unbounded_send(Event::Closed(id));
        let _ = Self::notification_closed(&emitter, id, Reason::ClosedByCall as u32).await;
    }

    fn get_server_information(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        ("shell", "jonatanlindh", env!("CARGO_PKG_VERSION"), "1.2")
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

fn is_avatar(width: u32, height: u32) -> bool {
    width <= 256 && (width as f32 / height.max(1) as f32) < 1.3
}

/// The spec's raw image: (width, height, rowstride, has alpha, bits per sample, channels, data).
fn image_data(value: &OwnedValue) -> Option<Picture> {
    let (width, height, stride, _alpha, _bits, channels, data): (
        i32,
        i32,
        i32,
        bool,
        i32,
        i32,
        Vec<u8>,
    ) = value.try_clone().ok()?.try_into().ok()?;
    let (w, h, stride, channels) = (
        width as usize,
        height as usize,
        stride as usize,
        channels as usize,
    );
    if !(3..=4).contains(&channels) || data.len() < stride * h.saturating_sub(1) + w * channels {
        return None;
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        for px in data[row * stride..][..w * channels].chunks_exact(channels) {
            rgba.extend_from_slice(&[px[0], px[1], px[2], if channels == 4 { px[3] } else { 255 }]);
        }
    }
    Some(Picture {
        image: Image::Raster(icons::raster(width as u32, height as u32, rgba)?),
        avatar: is_avatar(width as u32, height as u32),
        path: None,
    })
}

/// Markup flattened to plain text: tags dropped, entities decoded, `<br>` kept as a newline.
fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
        if tag.starts_with("br") {
            out.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    decode_entities(&out).trim().to_string()
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &after[..end];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            }?;
            Some((c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
