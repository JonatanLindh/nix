//! Notifications (Notifs.qml + NotifPopups.qml + NotifCenter.qml): popups for what's new, and a
//! history panel for everything that isn't transient, where its actions still work until it's
//! removed. The daemon itself is `services::notifications`; this entity is its UI-side model,
//! and owns the popup window.
//!
//! The history and do-not-disturb are kept across restarts (`saved`). Restored notifications
//! are closed: their app has long let go of them, so they're for reading and removing only.

mod saved;
mod view;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local};
use futures::StreamExt;
use gpui::component::Root;
use gpui::{
    Context, Size, WindowBackgroundAppearance, WindowHandle,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    prelude::*,
    px,
};
use saved::Saved;
pub use view::{Center, indicator};

use crate::services::icons;
use crate::services::notifications::{self as daemon, Event, Notification, Reason, Urgency};
use crate::shell::{display_named, open_layer};
use crate::{runtime, theme};

const HISTORY: usize = 50;
const MAX_POPUPS: usize = 4;
/// How often popups count down.
const TICK: Duration = Duration::from_millis(200);
const WIDTH: f32 = theme::PANEL_WIDTH;
/// Fixed: resizing a layer surface makes Hyprland animate it. Only the cards take input and
/// are blurred behind.
const HEIGHT: f32 = 720.;
/// Between popup cards.
const SPACING: f32 = 10.;

/// Where popups go.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Place {
    /// The focused monitor.
    pub monitor: Option<String>,
    /// How far down its bars reach.
    pub top: i32,
    /// A panel is open there, which hides the popups meanwhile.
    pub covered: bool,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub notification: Arc<Notification>,
    pub time: DateTime<Local>,
    /// Held back by do-not-disturb and not yet seen in the history.
    pub held: bool,
    /// Still open on the bus, so its actions can be invoked.
    pub live: bool,
}

#[derive(Debug)]
struct Popup {
    id: u32,
    /// `None` stays until dismissed.
    remaining: Option<Duration>,
}

pub struct Notifications {
    /// Newest first.
    pub history: Vec<Entry>,
    /// Newest first.
    popups: Vec<Popup>,
    pub dnd: bool,
    /// Expiry stops while the pointer is over the popups.
    paused: bool,
    place: Place,
    /// The popup window (`None` while it opens) and the monitor it's on.
    window: Option<(Option<WindowHandle<Root>>, String)>,
    /// What was last written to disk (`Saved::to_json`), so it's written again only on a change.
    saved: String,
    /// The saved history is still being loaded; nothing is written meanwhile, so it isn't lost.
    restoring: bool,
}

impl Notifications {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let saved = Saved::load();
        // Before the daemon starts handing out ids.
        if let Some(id) = saved.history.iter().map(|e| e.id).max() {
            daemon::skip_ids(id);
        }
        let dnd = saved.dnd;
        // Icons are looked up and decoded off the UI thread.
        let restored = cx.background_executor().spawn(async move {
            saved
                .history
                .into_iter()
                .map(|e| e.restore(|name| icons::by_name(name, None)))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let entries = restored.await;
            let _ = this.update(cx, |n, cx| n.restore(entries, cx));
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let mut events = daemon::watch();
            while let Some(event) = events.next().await {
                if this.update(cx, |n, cx| n.event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |n, cx| n.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            history: Vec::new(),
            popups: Vec::new(),
            dnd,
            paused: false,
            place: Place::default(),
            window: None,
            saved: String::new(),
            restoring: true,
        }
    }

    /// The history from before the restart, older than anything that has come in since.
    fn restore(&mut self, entries: Vec<Entry>, cx: &mut Context<Self>) {
        for entry in entries {
            if self.entry(entry.notification.id).is_none() && self.history.len() < HISTORY {
                self.history.push(entry);
            }
        }
        self.restoring = false;
        self.save();
        cx.notify();
    }

    /// Writes the history and do-not-disturb out, if they've changed.
    fn save(&mut self) {
        if self.restoring {
            return;
        }
        let json = Saved::new(self.dnd, &self.history).to_json();
        if json != self.saved {
            Saved::write(&json);
            self.saved = json;
        }
    }

    pub fn unread(&self) -> usize {
        self.history.iter().filter(|e| e.held).count()
    }

    /// The popups on screen: those still open, newest first.
    fn shown(&self) -> impl Iterator<Item = &Entry> {
        self.popups
            .iter()
            .filter_map(|p| self.entry(p.id))
            .filter(|e| e.live)
            .take(MAX_POPUPS)
    }

    /// Transient notifications are in `history` too, while their popup is up; the history panel
    /// leaves them out.
    fn entry(&self, id: u32) -> Option<&Entry> {
        self.history.iter().find(|e| e.notification.id == id)
    }

    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Notify(n) => self.receive(n),
            Event::Closed(id) => {
                if let Some(e) = self.history.iter_mut().find(|e| e.notification.id == id) {
                    e.live = false;
                }
                self.popups.retain(|p| p.id != id);
            }
            Event::Unavailable => {}
        }
        self.changed(cx);
    }

    /// New notifications, and updates to existing ones (same id): both pop up again.
    fn receive(&mut self, n: Arc<Notification>) {
        let id = n.id;
        let critical = n.urgency == Urgency::Critical;
        let old = self.history.iter().position(|e| e.notification.id == id);
        let time = old.map_or_else(Local::now, |i| self.history[i].time);
        if let Some(i) = old {
            self.history.remove(i);
        }
        // Transient ones only pop up, but they still live in the list so the popup can find
        // them; they're dropped once off screen.
        self.history.insert(
            0,
            Entry {
                notification: n.clone(),
                time,
                held: self.dnd && !critical,
                live: true,
            },
        );
        while self.history.len() > HISTORY {
            if let Some(e) = self.history.pop() {
                self.close_on_bus(&e, Reason::Dismissed);
            }
        }

        self.popups.retain(|p| p.id != id);
        if self.dnd && !critical {
            if n.transient {
                self.drop_transient(id);
            }
            return;
        }
        self.popups.insert(
            0,
            Popup {
                id,
                remaining: timeout(&n),
            },
        );
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.paused || self.window.is_none() {
            return;
        }
        let shown: Vec<u32> = self.shown().map(|e| e.notification.id).collect();
        let mut done = Vec::new();
        for popup in self.popups.iter_mut().filter(|p| shown.contains(&p.id)) {
            if let Some(left) = &mut popup.remaining {
                *left = left.saturating_sub(TICK);
                if left.is_zero() {
                    done.push(popup.id);
                }
            }
        }
        if !done.is_empty() {
            for id in done {
                self.hide_popup(id);
            }
            self.changed(cx);
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// A click on a notification: its default action, or out of the way.
    pub fn activate(&mut self, id: u32, cx: &mut Context<Self>) {
        if self
            .entry(id)
            .is_some_and(|e| e.live && e.notification.has_default)
        {
            self.invoke(id, "default".into(), cx);
        } else {
            self.hide(id, cx);
        }
    }

    /// Off the screen (still in the history).
    pub fn hide(&mut self, id: u32, cx: &mut Context<Self>) {
        self.hide_popup(id);
        self.changed(cx);
    }

    fn hide_popup(&mut self, id: u32) {
        self.popups.retain(|p| p.id != id);
        if self.entry(id).is_some_and(|e| e.notification.transient) {
            self.drop_transient(id);
        }
    }

    /// Out of the history, and closed.
    pub fn remove(&mut self, id: u32, cx: &mut Context<Self>) {
        if let Some(e) = self.entry(id).cloned() {
            self.close_on_bus(&e, Reason::Dismissed);
        }
        self.history.retain(|e| e.notification.id != id);
        self.popups.retain(|p| p.id != id);
        self.changed(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        for e in std::mem::take(&mut self.history) {
            self.close_on_bus(&e, Reason::Dismissed);
        }
        self.popups.clear();
        self.changed(cx);
    }

    pub fn toggle_dnd(&mut self, cx: &mut Context<Self>) {
        self.dnd = !self.dnd;
        if self.dnd {
            // Only critical popups stay up.
            let history = &self.history;
            self.popups.retain(|p| {
                history.iter().any(|e| {
                    e.notification.id == p.id && e.notification.urgency == Urgency::Critical
                })
            });
        }
        self.changed(cx);
    }

    /// The history has been looked at.
    pub fn seen(&mut self, cx: &mut Context<Self>) {
        for e in &mut self.history {
            e.held = false;
        }
        self.save();
        cx.notify();
    }

    /// Invokes an action. Unless the notification is resident, that closes it, so it leaves the
    /// history too.
    pub fn invoke(&mut self, id: u32, key: String, cx: &mut Context<Self>) {
        let Some(entry) = self.entry(id).filter(|e| e.live) else {
            return;
        };
        let resident = entry.notification.resident;
        self.popups.retain(|p| p.id != id);
        runtime::detach(daemon::invoked(id, key));
        if !resident {
            self.history.retain(|e| e.notification.id != id);
            runtime::detach(daemon::closed(id, Reason::Dismissed));
        }
        self.changed(cx);
    }

    /// A transient notification is done once it's off screen.
    fn drop_transient(&mut self, id: u32) {
        self.history.retain(|e| e.notification.id != id);
        runtime::detach(daemon::closed(id, Reason::Expired));
    }

    fn close_on_bus(&self, entry: &Entry, reason: Reason) {
        if entry.live {
            runtime::detach(daemon::closed(entry.notification.id, reason));
        }
    }

    /// Where popups go, when focus moves or a panel opens or closes.
    pub fn set_place(&mut self, place: Place, cx: &mut Context<Self>) {
        if self.place != place {
            self.place = place;
            self.sync_window(cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.save();
        self.sync_window(cx);
        cx.notify();
    }

    /// Popups show on the focused monitor while there are any and no panel covers them.
    fn sync_window(&mut self, cx: &mut Context<Self>) {
        let want = (self.shown().next().is_some() && !self.place.covered)
            .then(|| self.place.monitor.clone())
            .flatten();
        if self.window.as_ref().map(|(_, m)| m) == want.as_ref() {
            return;
        }
        if let Some((Some(window), _)) = self.window.take() {
            crate::shell::close_window(window, cx);
        }
        self.paused = false;
        let Some(monitor) = want else {
            return;
        };
        // Opened once this update is over: the popups draw as they open, and read this.
        self.window = Some((None, monitor.clone()));
        let (model, top) = (cx.entity(), self.place.top);
        cx.defer(move |cx| {
            let display = display_named(&monitor, cx);
            let opened = open_layer(
                cx,
                display,
                layer(top),
                Size::new(px(WIDTH), px(HEIGHT)),
                WindowBackgroundAppearance::Transparent,
                |window, cx| {
                    let model = model.clone();
                    cx.new(|cx| view::Popups::new(model, window, cx)).into()
                },
            );
            let window = match opened {
                Ok(window) => window,
                Err(e) => return eprintln!("notifications: opening the popups: {e:#}"),
            };
            model.update(cx, |n, cx| match &mut n.window {
                Some((slot @ None, on)) if *on == monitor => *slot = Some(window),
                // Moved on while it opened.
                _ => {
                    crate::shell::close_window(window, cx);
                }
            });
        });
    }
}

/// mako's old config: 5 s, critical stays; low is quicker. The app's own timeout wins when it
/// sets one, except for critical.
fn timeout(n: &Notification) -> Option<Duration> {
    match n.urgency {
        Urgency::Critical => None,
        _ if n.expire_timeout > 0 => Some(Duration::from_millis(n.expire_timeout as u64)),
        Urgency::Low => Some(Duration::from_secs(3)),
        Urgency::Normal => Some(Duration::from_secs(5)),
    }
}

/// Top right, `top` down (under the bars), over windows; never takes the keyboard.
fn layer(top: i32) -> LayerShellOptions {
    LayerShellOptions {
        namespace: "shell:notifications".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::RIGHT,
        // Ignoring exclusive zones, so it clears the bars by itself (`top`): Hyprland doesn't
        // blur an overlay surface that respects them.
        exclusive_zone: Some(px(-1.)),
        margin: Some((px(top as f32 + 4.), px(8.), px(0.), px(0.))),
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    }
}
