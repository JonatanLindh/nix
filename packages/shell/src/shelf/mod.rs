//! The drag shelf (Shelf.qml + ShelfWindow.qml + ShelfButton.qml): files parked for dragging
//! into other apps, plus tools that work on them. The backend owns the items and runs the tools
//! (`services::shelf`); this entity keeps the UI state (selection, which tools apply, the status
//! line) and the shelf window, on one monitor at a time.
//!
//! Drags in and out are gpui's own, which carry files only. `shell msg paste` puts the clipboard
//! on the shelf: copied files, a link, text or an image.
//!
//! Not ported yet: dragging a link item out as its address and dropping links in from a browser
//! (gpui's drag and drop only carries files).

mod view;

use std::time::Duration;

use futures::StreamExt;
use gpui::component::Root;
use gpui::{
    Context, Size, WindowBackgroundAppearance, WindowHandle,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    prelude::*,
    px,
};

pub use view::button;

use crate::services::shelf::{self as backend, Action, Event, Item, Job, LinkQr, Outcome, Peer};
use crate::shell::{display_named, open_layer};
use crate::{runtime, theme};

/// The shelf window's layer namespace.
const NAMESPACE: &str = "shell:shelf";
const WIDTH: f32 = 300.;
/// Fixed: resizing a layer surface as the card grows makes Hyprland animate it, which jiggles.
/// The card sizes itself inside, and only it takes input.
const HEIGHT: f32 = 900.;
/// Holding a drag over the bar button this long opens the shelf (spring-loading).
const SPRING: Duration = Duration::from_millis(400);

pub struct Shelf {
    pub items: Vec<Item>,
    /// Paths of the selected items; tools work on these, or on everything when none are.
    selected: Vec<String>,
    /// Tools for the targets.
    actions: Vec<Action>,
    /// The inline panel open under the tools: an action with options, "taildrop", "qr" or
    /// "more".
    open: Option<String>,
    /// Taildrop targets, loaded as the "taildrop" panel opens.
    peers: Vec<Peer>,
    peers_loading: bool,
    /// The QR the "QR code" tool made, shown while `open` is "qr".
    link_qr: Option<LinkQr>,
    shares: Shares,
    status: Option<Status>,
    /// Actions running now.
    busy: Vec<String>,
    /// The shelf window (`None` while it opens) and the monitor it's on, while shown.
    shown: Option<(Option<WindowHandle<Root>>, String)>,
    /// The monitor whose bar button a drag is resting on.
    spring: Option<String>,
}

#[derive(Debug, Clone)]
struct Status {
    text: String,
    error: bool,
}

impl Shelf {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            let mut events = backend::watch();
            while let Some(event) = events.next().await {
                if this.update(cx, |shelf, cx| shelf.event(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            items: Vec::new(),
            selected: Vec::new(),
            actions: Vec::new(),
            open: None,
            peers: Vec::new(),
            peers_loading: false,
            link_qr: None,
            shares: Shares::default(),
            status: None,
            busy: Vec::new(),
            shown: None,
            spring: None,
        }
    }

    pub fn shown_on(&self, monitor: &str) -> bool {
        self.shown.as_ref().is_some_and(|(_, m)| m == monitor)
    }

    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Items { items, reason } => {
                self.selected.retain(|p| items.iter().any(|i| &i.path == p));
                self.items = items;
                self.refresh_actions(cx);
                // Added from outside the bar (a screenshot's "Add to shelf", a Taildrop arriving): show it, as a
                // drop would.
                if matches!(reason.as_str(), "add" | "taildrop")
                    && self.shown.is_none()
                    && let Some(monitor) = focused_monitor(cx)
                {
                    self.show(&monitor, cx);
                }
            }
            Event::Job {
                job,
                state,
                message,
            } => {
                if self.shares.changed(&job, &state) {
                    self.say(message, state == "failed");
                }
            }
        }
        cx.notify();
    }

    /// Shows the shelf on `monitor`, or hides it if it's showing there.
    pub fn toggle(&mut self, monitor: &str, cx: &mut Context<Self>) {
        if self.shown_on(monitor) {
            self.hide(cx);
        } else {
            self.show(monitor, cx);
        }
    }

    fn show(&mut self, monitor: &str, cx: &mut Context<Self>) {
        self.hide(cx);
        // Opened once this update is over: the card draws as it opens, and reads the shelf.
        let monitor = monitor.to_string();
        self.shown = Some((None, monitor.clone()));
        let model = cx.entity();
        cx.defer(move |cx| {
            let display = display_named(&monitor, cx);
            let opened = open_layer(
                cx,
                display,
                layer(),
                Size::new(px(WIDTH), px(HEIGHT)),
                WindowBackgroundAppearance::Transparent,
                |window, cx| {
                    let model = model.clone();
                    cx.new(|cx| view::Window::new(model, window, cx)).into()
                },
            );
            let window = match opened {
                Ok(window) => window,
                Err(e) => return eprintln!("shelf: opening the window: {e:#}"),
            };
            model.update(cx, |shelf, cx| match &mut shelf.shown {
                Some((slot @ None, on)) if *on == monitor => *slot = Some(window),
                // Hidden or moved while it opened.
                _ => {
                    crate::shell::close_window(window, cx);
                }
            });
        });
        cx.notify();
    }

    pub fn hide(&mut self, cx: &mut Context<Self>) {
        if let Some((window, _)) = self.shown.take() {
            if let Some(window) = window {
                let _ = window.update(cx, |_, window, _| {
                    crate::wayland::forget(window);
                    window.remove_window();
                });
            }
            cx.notify();
        }
    }

    /// Files dropped on the bar button on `monitor`, or on the card (`None`).
    pub fn dropped(&mut self, monitor: Option<String>, paths: Vec<String>, cx: &mut Context<Self>) {
        self.spring = None;
        let n = paths.len();
        self.say(
            if n == 1 {
                "Added 1 item".into()
            } else {
                format!("Added {n} items")
            },
            false,
        );
        if let Some(monitor) = monitor
            && !self.shown_on(&monitor)
        {
            self.show(&monitor, cx);
        }
        runtime::detach(backend::add(paths));
        cx.notify();
    }

    /// A drag is over the bar button on `monitor` (or has left it): resting there a moment opens
    /// the shelf, so the drop can land on the card instead.
    pub fn drag_over_button(&mut self, monitor: Option<String>, cx: &mut Context<Self>) {
        if self.spring == monitor {
            return;
        }
        self.spring = monitor.clone();
        let Some(monitor) = monitor else { return };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SPRING).await;
            let _ = this.update(cx, |shelf, cx| {
                if shelf.spring.as_deref() == Some(&monitor) && !shelf.shown_on(&monitor) {
                    shelf.show(&monitor, cx);
                }
            });
        })
        .detach();
    }

    /// Puts the clipboard on the shelf (`shell msg paste`) and shows the shelf, on `monitor` if
    /// it isn't showing yet. The new item comes back on top, selected.
    pub fn paste(&mut self, monitor: Option<String>, cx: &mut Context<Self>) {
        if self.shown.is_none()
            && let Some(monitor) = monitor.or_else(|| focused_monitor(cx))
        {
            self.show(&monitor, cx);
        }
        self.say("Pasting…".into(), false);
        self.busy.push("paste".into());
        runtime::then(cx, backend::paste(), |this, outcome, cx| {
            this.finished("paste", outcome, cx);
        });
        cx.notify();
    }

    fn select(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(i) = self.selected.iter().position(|p| *p == path) {
            self.selected.remove(i);
        } else {
            self.selected.push(path);
        }
        self.open = None;
        self.refresh_actions(cx);
        cx.notify();
    }

    fn remove(&mut self, path: String) {
        runtime::detach(backend::remove(path));
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.open = None;
        self.status = None;
        runtime::detach(backend::clear());
        cx.notify();
    }

    fn say(&mut self, text: String, error: bool) {
        self.status = (!text.is_empty()).then_some(Status { text, error });
    }

    /// The items tools work on: the selection, or everything.
    fn targets(&self) -> Vec<&Item> {
        if self.selected.is_empty() {
            self.items.iter().collect()
        } else {
            self.items
                .iter()
                .filter(|i| self.selected.contains(&i.path))
                .collect()
        }
    }

    fn target_paths(&self) -> Vec<String> {
        self.targets().iter().map(|i| i.path.clone()).collect()
    }

    fn refresh_actions(&mut self, cx: &mut Context<Self>) {
        // Folders get a trailing slash, so the tools can tell them apart without a stat.
        let paths: Vec<String> = self
            .targets()
            .iter()
            .map(|i| {
                if i.kind == "folder" {
                    format!("{}/", i.path)
                } else {
                    i.path.clone()
                }
            })
            .collect();
        if paths.is_empty() {
            self.actions.clear();
            return;
        }
        runtime::then(cx, backend::actions(paths), |this, actions, cx| {
            this.actions = actions;
            cx.notify();
        });
    }

    /// Whether `id`'s tile is lit: its panel is open, or its share's card is showing.
    fn is_on(&self, id: &str) -> bool {
        self.open.as_deref() == Some(id)
            || match id {
                "funnel" => self.shares.funnel.is_some() && self.shares.funnel_card,
                "wormhole" => self.shares.wormhole.is_some() && self.shares.wormhole_card,
                _ => false,
            }
    }

    fn activate(&mut self, action: Action, cx: &mut Context<Self>) {
        cx.notify();
        // The QR is a card under the tools: its tile folds it away again.
        if action.id == "qr" && self.open.as_deref() == Some("qr") {
            self.open = None;
            return;
        }
        if action.id == "more" || action.id == "taildrop" || action.options.is_some() {
            self.open = (self.open.as_deref() != Some(action.id)).then(|| action.id.to_string());
            if self.open.as_deref() == Some("taildrop") {
                self.load_peers(cx);
            }
            return;
        }
        self.open = None;
        if action.id == "copy-paths" {
            let paths = self.target_paths();
            let n = paths.len();
            self.say(
                if n == 1 {
                    "Copied path".into()
                } else {
                    format!("Copied {n} paths")
                },
                false,
            );
            copy(paths.join("\n"));
            return;
        }
        // A share that's running: the tile shows or hides its card instead of starting another.
        match action.id {
            "funnel" if self.shares.funnel.is_some() => {
                self.shares.funnel_card = !self.shares.funnel_card;
            }
            "wormhole" if self.shares.wormhole.is_some() => {
                self.shares.wormhole_card = !self.shares.wormhole_card;
            }
            _ => self.run(action.id.into(), String::new(), cx),
        }
    }

    fn load_peers(&mut self, cx: &mut Context<Self>) {
        self.peers_loading = true;
        runtime::then(cx, backend::peers(), |this, peers, cx| {
            this.peers_loading = false;
            match peers {
                Ok(peers) => this.peers = peers,
                Err(message) => {
                    this.peers.clear();
                    this.say(message, true);
                }
            }
            cx.notify();
        });
    }

    /// Sends the targets to `peer` with Taildrop.
    fn taildrop(&mut self, peer: String, cx: &mut Context<Self>) {
        self.run("taildrop".into(), peer.clone(), cx);
        self.say(format!("Sending to {peer}…"), false);
    }

    fn cancel(&mut self, job: String) {
        runtime::detach(backend::cancel(job));
    }

    fn copy_wormhole(&mut self, cx: &mut Context<Self>) {
        if let Some(code) = self.shares.wormhole.as_ref().and_then(|j| j.code.as_ref()) {
            copy(format!("wormhole receive {code}"));
            self.shares.wormhole_copied = true;
            cx.notify();
        }
    }

    fn copy_funnel(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = self.shares.funnel.as_ref().and_then(|j| j.url.clone()) {
            copy(url);
            self.say("Link copied".into(), false);
            cx.notify();
        }
    }

    fn run(&mut self, id: String, option: String, cx: &mut Context<Self>) {
        self.open = None;
        let working = match id.as_str() {
            "funnel" => "Starting a funnel link…",
            "wormhole" => "Getting a wormhole code…",
            "shrink" => "Re-encoding, this can take a while…",
            "gif" => "Making a GIF…",
            "ocr" => "Reading text…",
            "hand" => "Handing to the shelf…",
            _ => "Working…",
        };
        self.say(working.into(), false);
        self.busy.push(id.clone());
        let files = self.target_paths();
        runtime::then(
            cx,
            backend::run_action(id.clone(), option, files),
            move |this, outcome, cx| {
                this.finished(&id, outcome, cx);
            },
        );
        cx.notify();
    }

    fn finished(&mut self, id: &str, outcome: Outcome, cx: &mut Context<Self>) {
        self.busy.retain(|b| b != id);
        // A share's card says what's going on; the status line stays quiet.
        let message = if outcome.job.is_some() {
            String::new()
        } else {
            outcome.message
        };
        self.say(message, !outcome.ok);
        if let Some(qr) = outcome.qr {
            self.link_qr = Some(qr);
            self.open = Some("qr".into());
        }
        if let Some(job) = outcome.job {
            self.shares.start(job);
        }
        // The backend has put the results on the shelf; select them, so the next tool works on
        // them.
        let pick = if outcome.outputs.is_empty() {
            outcome.select
        } else {
            outcome.outputs
        };
        if !pick.is_empty() {
            self.selected = pick;
        }
        self.refresh_actions(cx);
        if let Some(text) = outcome.copy {
            copy(text);
        }
        cx.notify();
    }
}

/// The running shares, each with a card that can be folded away while the share goes on.
#[derive(Debug, Default)]
struct Shares {
    funnel: Option<Job>,
    funnel_card: bool,
    wormhole: Option<Job>,
    wormhole_card: bool,
    /// The other side has the code and the files are on their way.
    wormhole_sending: bool,
    /// The receive command has been copied (for this code).
    wormhole_copied: bool,
}

impl Shares {
    /// A share just started (or was already running): show its card.
    fn start(&mut self, job: Job) {
        match job.kind {
            "funnel" => {
                self.funnel = Some(job);
                self.funnel_card = true;
            }
            "wormhole" => {
                self.wormhole = Some(job);
                self.wormhole_card = true;
                self.wormhole_sending = false;
                self.wormhole_copied = false;
            }
            _ => {}
        }
    }

    /// Share `job` changed to `state`. Whether that's news for the status line: a share that's
    /// ended says how, while one that's now sending says so on its card.
    fn changed(&mut self, job: &str, state: &str) -> bool {
        let is = |share: &Option<Job>| share.as_ref().is_some_and(|s| s.id == job);
        if is(&self.wormhole) {
            if state == "sending" {
                self.wormhole_sending = true;
                return false;
            }
            self.wormhole = None;
        } else if is(&self.funnel) {
            self.funnel = None;
        } else {
            return false;
        }
        true
    }
}

fn copy(text: String) {
    runtime::detach(async move { crate::services::spawn("wl-copy", &["--", &text]).await });
}

fn focused_monitor(cx: &gpui::App) -> Option<String> {
    let hypr = crate::services::Services::get(cx).hypr;
    hypr.read(cx)
        .value
        .monitors
        .iter()
        .find(|m| m.focused)
        .map(|m| m.name.clone())
}

/// Top-left, under the bars (it respects their exclusive zones), never taking the keyboard.
fn layer() -> LayerShellOptions {
    LayerShellOptions {
        namespace: NAMESPACE.into(),
        layer: Layer::Top,
        anchor: Anchor::TOP | Anchor::LEFT,
        exclusive_zone: Some(px(0.)),
        margin: Some((px(theme::PANEL_GAP), px(0.), px(0.), px(theme::BAR_PADDING))),
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::shelf::Qr;

    fn job(id: &str, kind: &'static str) -> Job {
        Job {
            id: id.into(),
            kind,
            url: None,
            code: Some("7-crossover-clockwork".into()),
            qr: Qr::default(),
            files: Vec::new(),
        }
    }

    #[test]
    fn wormhole_lifecycle() {
        let mut shares = Shares::default();
        shares.start(job("w", "wormhole"));
        shares.wormhole_card = false;
        shares.wormhole_copied = true;
        // Someone else's share: nothing to say.
        assert!(!shares.changed("other", "done"));
        // Sending shows on the card, not the status line.
        assert!(!shares.changed("w", "sending"));
        assert!(shares.wormhole_sending);
        assert!(shares.wormhole.is_some());
        assert!(shares.changed("w", "done"));
        assert!(shares.wormhole.is_none());
        // A new one starts fresh, card shown.
        shares.start(job("w2", "wormhole"));
        assert!(shares.wormhole_card && !shares.wormhole_sending && !shares.wormhole_copied);
    }
}
