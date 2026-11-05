//! The session menu (Session.qml): the screen dims and a glass card shows two rows of three
//! tiles, each with its key. Press the key, or move with the arrows (or the mouse) and press
//! enter. No confirmation, by request.
//!
//! It covers every monitor: the card on the one it was opened from, the dimmed backdrop on the
//! others, so a click anywhere outside the card closes it.

use gpui::component::Root;
use gpui::{
    App, Context, DisplayId, Entity, FocusHandle, KeyDownEvent, Size, Window, WindowBackgroundAppearance,
    WindowHandle, div,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    prelude::*,
    px,
};
use logind_zbus::manager::{IsSupported, ManagerProxy};

use crate::services::spawn;
use crate::shell::{display_named, open_layer};
use crate::ui::{Width, bar_button, glass, hstack, icon, key_cap, key_hint, label, small, vstack};
use crate::{runtime, theme};

struct Action {
    label: &'static str,
    icon: &'static str,
    key: &'static str,
    command: &'static [&'static str],
}

const ACTIONS: [Action; 6] = [
    Action {
        label: "Lock",
        icon: "lock",
        key: "l",
        command: &["loginctl", "lock-session"],
    },
    Action {
        label: "Hibernate",
        icon: "hibernate",
        key: "h",
        command: &["systemctl", "hibernate"],
    },
    Action {
        label: "Log out",
        icon: "logout",
        key: "e",
        command: &["sh", "-c", r#"loginctl terminate-user "$USER""#],
    },
    Action {
        label: "Shut down",
        icon: "power",
        key: "s",
        command: &["systemctl", "poweroff"],
    },
    Action {
        label: "Suspend",
        icon: "moon-outline",
        key: "u",
        command: &["systemctl", "suspend"],
    },
    Action {
        label: "Reboot",
        icon: "reboot",
        key: "r",
        command: &["systemctl", "reboot"],
    },
];
const HIBERNATE: usize = 1;
const COLUMNS: usize = 3;
const CARD: f32 = 560.;
const PADDING: f32 = 18.;
const GAP: f32 = 10.;
const TILE: f32 = (CARD - 2. * PADDING - (COLUMNS as f32 - 1.) * GAP) / COLUMNS as f32;

#[derive(Default)]
pub struct Session {
    /// Its windows while open, with the display each is on: the card's first.
    windows: Vec<(WindowHandle<Root>, Option<DisplayId>)>,
    /// Its windows are about to open.
    opening: bool,
    selected: usize,
    can_hibernate: bool,
}

impl Session {
    pub fn is_open(&self) -> bool {
        self.opening || !self.windows.is_empty()
    }

    /// Opens with the card on `monitor` and the backdrop on the rest of `monitors`, or closes.
    pub fn toggle(&mut self, monitor: Option<String>, monitors: &[String], cx: &mut Context<Self>) {
        if self.is_open() {
            self.close(cx);
            return;
        }
        self.selected = 0;
        self.opening = true;
        // After this update: the menu draws as it opens, and reads the session.
        let model = cx.entity();
        let monitors = monitors.to_vec();
        cx.defer(move |cx| {
            let windows = open_windows(&model, monitor, &monitors, cx);
            model.update(cx, |session, cx| {
                session.opening = false;
                session.windows = windows;
                cx.notify();
            });
        });
        // Asked each time: it can change (swap appearing, a kernel update).
        runtime::then(cx, can_hibernate(), |this, yes, cx| {
            this.can_hibernate = yes;
            cx.notify();
        });
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        for (window, _) in self.windows.drain(..) {
            crate::shell::close_window(window, cx);
        }
        cx.notify();
    }

    /// Displays came or went: a backdrop whose display is gone would linger (on another monitor,
    /// or nowhere), so it goes too, and the whole menu if the card's display went.
    pub fn displays_changed(&mut self, cx: &mut Context<Self>) {
        let displays = cx.displays();
        let present = |display: &Option<DisplayId>| {
            display.is_none_or(|id| displays.iter().any(|d| d.id() == id))
        };
        if self.windows.first().is_some_and(|(_, d)| !present(d)) {
            self.close(cx);
            return;
        }
        let gone: Vec<_> = self.windows.extract_if(.., |(_, d)| !present(d)).collect();
        for (window, _) in gone {
            crate::shell::close_window(window, cx);
        }
    }
}

/// The menu's windows: the card on `monitor`, backdrops on the rest of `monitors`.
fn open_windows(
    model: &Entity<Session>,
    monitor: Option<String>,
    monitors: &[String],
    cx: &mut App,
) -> Vec<(WindowHandle<Root>, Option<DisplayId>)> {
    let mut windows = Vec::new();
    let display = monitor.as_deref().and_then(|m| display_named(m, cx));
    let main = open_layer(
        cx,
        display,
        layer(true),
        Size::default(),
        WindowBackgroundAppearance::Blurred,
        |window, cx| {
            let model = model.clone();
            cx.new(|cx| Menu::new(model, window, cx)).into()
        },
    );
    match main {
        Ok(window) => windows.push((window, display)),
        Err(e) => {
            eprintln!("session: opening the menu: {e:#}");
            return windows;
        }
    }
    for other in monitors.iter().filter(|m| Some(*m) != monitor.as_ref()) {
        let model = model.clone();
        let display = display_named(other, cx);
        let backdrop = open_layer(
            cx,
            display,
            layer(false),
            Size::default(),
            WindowBackgroundAppearance::Blurred,
            |_, cx| cx.new(|_| Backdrop { model }).into(),
        );
        if let Ok(window) = backdrop {
            windows.push((window, display));
        }
    }
    windows
}

impl Session {
    fn usable(&self, i: usize) -> bool {
        i != HIBERNATE || self.can_hibernate
    }

    fn hover(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.usable(i) && self.selected != i {
            self.selected = i;
            cx.notify();
        }
    }

    fn run(&mut self, i: usize, cx: &mut Context<Self>) {
        if !self.usable(i) {
            return;
        }
        self.close(cx);
        if let [program, args @ ..] = ACTIONS[i].command {
            runtime::detach(spawn(program, args));
        }
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let n = ACTIONS.len();
        match event.keystroke.key.as_str() {
            "escape" => self.close(cx),
            "enter" => self.run(self.selected, cx),
            "left" => self.selected = (self.selected + n - 1) % n,
            "right" => self.selected = (self.selected + 1) % n,
            "up" | "down" => self.selected = (self.selected + COLUMNS) % n,
            key => {
                if let Some(i) = ACTIONS.iter().position(|a| a.key.eq_ignore_ascii_case(key)) {
                    self.run(i, cx);
                }
            }
        }
        cx.notify();
    }
}

/// The dimmed screen; a click on it closes the menu.
fn backdrop(model: &Entity<Session>) -> gpui::Stateful<gpui::Div> {
    let model = model.clone();
    crate::ui::root()
        .id("backdrop")
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::BACKDROP)
        .on_click(move |_, _, cx| model.update(cx, |s, cx| s.close(cx)))
}

struct Backdrop {
    model: Entity<Session>,
}

impl Render for Backdrop {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        backdrop(&self.model)
    }
}

/// The card's window, which has the keyboard.
struct Menu {
    model: Entity<Session>,
    focus: FocusHandle,
    _observer: gpui::Subscription,
}

impl Menu {
    fn new(model: Entity<Session>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let observer = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            focus,
            _observer: observer,
        }
    }
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.clone();
        let session = model.read(cx);
        // Rows of `COLUMNS` tiles, `TILE` wide each, filling the card.
        let tiles = crate::ui::hstack(GAP)
            .flex_wrap()
            .w(px(CARD - 2. * PADDING))
            .children(
                ACTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, a)| tile(&model, session, i, a)),
            );
        let hints = hstack(16.)
            .child(small("Press a letter", theme::TEXT_DIM))
            .child(key_hint(&["←", "→", "↵"], "Pick"))
            .child(key_hint(&["esc"], "Close"));
        // Clicks between the tiles stay on the card instead of closing.
        let card = glass(28.)
            .id("card")
            .w(px(CARD))
            .p(px(PADDING))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(vstack(14.).items_center().child(tiles).child(hints));

        let keys = model.clone();
        backdrop(&model)
            .track_focus(&self.focus)
            .on_key_down(move |event, _, cx| keys.update(cx, |s, cx| s.key(event, cx)))
            .child(card)
    }
}

fn tile(model: &Entity<Session>, session: &Session, i: usize, action: &Action) -> impl IntoElement {
    let usable = session.usable(i);
    let current = i == session.selected && usable;
    let ink = if current {
        theme::INK
    } else if usable {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    let cap_fill = if current {
        theme::alpha(theme::INK, 0.1)
    } else {
        theme::alpha(theme::WHITE, 0.1)
    };
    let content = div()
        .size_full()
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .child(icon(action.icon, 26., ink))
        .child(label(action.label, ink).text_size(px(14.)))
        .child(
            div()
                .absolute()
                .top(px(10.))
                .right(px(10.))
                .child(key_cap(action.key, ink, cap_fill)),
        );
    let (hover, run) = (model.clone(), model.clone());
    let mut button = bar_button(("action", i), content)
        .height(128.)
        .width(Width::Px(TILE))
        .radius(18.)
        .padding(0.)
        .rest(if current { theme::WHITE } else { theme::FILL });
    if usable {
        button = button.on_click(move |_, cx| run.update(cx, |s, cx| s.run(i, cx)));
    }
    div()
        .id(("tile", i))
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                hover.update(cx, |s, cx| s.hover(i, cx));
            }
        })
        .child(button)
}

/// A whole monitor, over everything. Only the card's window takes the keyboard.
fn layer(main: bool) -> LayerShellOptions {
    LayerShellOptions {
        namespace: "shell:session".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: Some(px(-1.)),
        keyboard_interactivity: if main {
            KeyboardInteractivity::Exclusive
        } else {
            KeyboardInteractivity::None
        },
        ..Default::default()
    }
}

async fn can_hibernate() -> bool {
    let result = async {
        let connection = zbus::Connection::system().await?;
        ManagerProxy::new(&connection).await?.can_hibernate().await
    };
    matches!(result.await, Ok(IsSupported::Yes))
}
