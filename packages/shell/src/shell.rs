//! The shell as a whole: a bar on every display, and the surfaces that aren't tied to one bar
//! (the session menu, the shelf, notification popups). It follows displays coming and going and
//! takes `shell msg` commands.

use std::collections::HashMap;

use futures::{FutureExt, StreamExt};
use gpui::component::Root;
use gpui::{
    AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, DisplayId, Entity, Global, Pixels, Size,
    Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, layer_shell::LayerShellOptions, point,
};
use uuid::Uuid;

use crate::bar::Bar;
use crate::notifications::Notifications;
use crate::osd::{self, Osd};
use crate::panel::PanelKind;
use crate::services::{Services, hyprland, ipc};
use crate::screenshot::{Mode, Screenshot};
use crate::session::Session;
use crate::shelf::Shelf;

pub struct Shell {
    bars: HashMap<DisplayId, BarWindow>,
    pub session: Entity<Session>,
    pub shelf: Entity<Shelf>,
    pub notifications: Entity<Notifications>,
    osd: Entity<Osd>,
    _subscriptions: Vec<Subscription>,
}

/// The shell's surfaces besides the bars, for views that show or open them. Handed out
/// instead of read from the [`Shell`], which may be mid-update when a view first renders.
#[derive(Clone)]
pub struct Surfaces {
    pub shell: Entity<Shell>,
    pub shelf: Entity<Shelf>,
    pub notifications: Entity<Notifications>,
    pub session: Entity<Session>,
}

struct BarWindow {
    window: WindowHandle<Root>,
    bar: Entity<Bar>,
    _observer: Subscription,
}

struct GlobalShell(Entity<Shell>);

impl Global for GlobalShell {}

impl Shell {
    pub fn init(cx: &mut App) {
        let shell = cx.new(|cx| {
            let hypr = Services::get(cx).hypr;
            let notifications = cx.new(Notifications::new);
            let mut subscriptions = vec![
                cx.observe_displays(|_: gpui::DisplayEvent, cx: &mut App| {
                    Self::global(cx).update(cx, |shell, cx| shell.sync_bars(cx))
                }),
                // Popups follow the focused monitor.
                cx.observe(&hypr, |shell: &mut Self, _, cx| shell.place_popups(cx)),
            ];
            let shelf = cx.new(Shelf::new);
            let session = cx.new(|_| Session::default());
            let osd = cx.new(Osd::new);
            subscriptions.push(cx.observe(&shelf, |_, _, cx| cx.notify()));
            Self::listen(cx);
            Self {
                bars: HashMap::new(),
                session,
                shelf,
                notifications,
                osd,
                _subscriptions: subscriptions,
            }
        });
        cx.set_global(GlobalShell(shell.clone()));
        shell.update(cx, |shell, cx| shell.sync_bars(cx));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalShell>().0.clone()
    }

    /// `shell msg` commands, as they arrive.
    fn listen(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut commands = ipc::watch();
            while let Some(command) = commands.next().await {
                if this
                    .update(cx, |shell, cx| shell.command(&command, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    /// Opens a bar on each new display and closes those whose display is gone.
    fn sync_bars(&mut self, cx: &mut Context<Self>) {
        let displays = cx.displays();
        self.bars.retain(|id, bar| {
            let keep = displays.iter().any(|d| d.id() == *id);
            if !keep {
                bar.bar.update(cx, |bar, cx| bar.close_panel(cx));
                close_window(bar.window, cx);
            }
            keep
        });
        for display in displays {
            let id = display.id();
            if self.bars.contains_key(&id) {
                continue;
            }
            let uuid = display.uuid().ok();
            let surfaces = Surfaces {
                shell: cx.entity(),
                shelf: self.shelf.clone(),
                notifications: self.notifications.clone(),
                session: self.session.clone(),
            };
            let mut bar = None;
            let window = open_layer(
                cx,
                Some(id),
                crate::bar::layer(),
                Size::new(Pixels::ZERO, gpui::px(crate::theme::BAR_HEIGHT)),
                WindowBackgroundAppearance::Blurred,
                |window, cx| {
                    let view = cx.new(|cx| Bar::new(uuid, window.window_handle(), surfaces, cx));
                    bar = Some(view.clone());
                    view.into()
                },
            );
            match (window, bar) {
                (Ok(window), Some(bar)) => {
                    // A panel opening or closing covers or uncovers the notification popups.
                    let observer = cx.observe(&bar, |shell, _, cx| shell.place_popups(cx));
                    self.bars.insert(
                        id,
                        BarWindow {
                            window,
                            bar,
                            _observer: observer,
                        },
                    );
                }
                (Err(e), _) => eprintln!("bar: opening a window: {e:#}"),
                (Ok(_), None) => {}
            }
        }
        self.session.update(cx, |session, cx| session.displays_changed(cx));
        self.place_popups(cx);
    }

    /// The bar on the monitor named `monitor`.
    fn bar_on(&self, monitor: &str, cx: &App) -> Option<Entity<Bar>> {
        let uuid = monitor_uuid(monitor);
        self.bars
            .values()
            .find(|b| b.bar.read(cx).uuid == Some(uuid))
            .map(|b| b.bar.clone())
    }

    fn focused_monitor(cx: &App) -> Option<String> {
        let hypr = Services::get(cx).hypr;
        hypr.read(cx)
            .value
            .monitors
            .iter()
            .find(|m| m.focused)
            .map(|m| m.name.clone())
    }

    /// Where notification popups go: the focused monitor, under its bars, unless a panel is open
    /// there.
    fn place_popups(&mut self, cx: &mut Context<Self>) {
        let hypr = Services::get(cx).hypr;
        let focused = hypr
            .read(cx)
            .value
            .monitors
            .iter()
            .find(|m| m.focused)
            .cloned();
        let bar = focused.as_ref().and_then(|m| self.bar_on(&m.name, cx));
        let covered = bar.as_ref().is_some_and(|bar| bar.read(cx).panel_open());
        // The OSD hides only behind the panel with its slider.
        let osd_place = osd::Place {
            monitor: focused.as_ref().map(|m| m.name.clone()),
            panel: bar.and_then(|bar| bar.read(cx).open_panel_kind().cloned()),
        };
        self.osd.update(cx, |osd, cx| osd.set_place(osd_place, cx));
        let place = crate::notifications::Place {
            top: focused.as_ref().map_or(0, |m| m.reserved[1]),
            monitor: focused.map(|m| m.name),
            covered,
        };
        self.notifications
            .update(cx, |n, cx| n.set_place(place, cx));
    }

    /// `shell msg <command>`, from Hyprland binds.
    fn command(&mut self, command: &str, cx: &mut Context<Self>) {
        let focused = Self::focused_monitor(cx);
        match command {
            "session" => self.toggle_session(focused, cx),
            "notifications" => {
                if let Some(bar) = focused.and_then(|m| self.bar_on(&m, cx)) {
                    // Once this update is over: the panel's views read the shell as they open.
                    App::defer(cx, move |cx| {
                        bar.update(cx, |bar, cx| {
                            bar.toggle_panel_from_keyboard(PanelKind::Notifications, cx)
                        });
                    });
                }
            }
            // As `qs ipc call bar toggle bt|pwr|ts` did, for scripts.
            "bluetooth" | "power" | "tailscale" => {
                let kind = match command {
                    "bluetooth" => PanelKind::Bluetooth,
                    "power" => PanelKind::Power,
                    _ => PanelKind::Tailscale,
                };
                if let Some(bar) = focused.and_then(|m| self.bar_on(&m, cx)) {
                    // Once this update is over, like "notifications": panels may read the shell.
                    App::defer(cx, move |cx| {
                        bar.update(cx, |bar, cx| bar.toggle_panel_from_keyboard(kind, cx));
                    });
                }
            }
            "launcher" | "clip" | "op" => {
                let command = command.to_string();
                // Once this update is over: the launcher reads the shell as it opens.
                App::defer(cx, move |cx| crate::launcher::toggle(&command, focused, cx));
            }
            "shelf" => {
                if let Some(monitor) = focused {
                    self.shelf
                        .update(cx, |shelf, cx| shelf.toggle(&monitor, cx));
                }
            }
            "picker" => {
                let picker = Services::get(cx).picker;
                picker.update(cx, |picker, cx| picker.pick(cx));
            }
            // The clipboard onto the shelf.
            "paste" => self.shelf.update(cx, |shelf, cx| shelf.paste(focused, cx)),
            "dnd" => self.notifications.update(cx, |n, cx| n.toggle_dnd(cx)),
            "screenshot" | "screenshot-window" | "screenshot-screen" => {
                let mode = match command {
                    "screenshot-window" => Mode::Window,
                    "screenshot-screen" => Mode::Screen,
                    _ => Mode::Selection,
                };
                Screenshot::global(cx).update(cx, |s, cx| s.open(mode, cx));
            }
            // The media keys: they go to the player the now-playing card shows, so the keys and
            // the card always agree.
            "play-pause" | "next" | "previous" => {
                let track = Services::get(cx).media.read(cx).value.clone();
                let Some(player) = track.map(|t| t.player) else {
                    return;
                };
                crate::runtime::detach(match command {
                    "play-pause" => crate::services::media::play_pause(player).boxed(),
                    "next" => crate::services::media::next(player).boxed(),
                    _ => crate::services::media::previous(player).boxed(),
                });
            }
            other => eprintln!("ipc: unknown command {other:?}"),
        }
    }

    /// Opens the session menu with its card on `monitor` (the focused one if `None`), or closes
    /// it.
    pub fn toggle_session(&mut self, monitor: Option<String>, cx: &mut Context<Self>) {
        let monitor = monitor.or_else(|| Self::focused_monitor(cx));
        let monitors: Vec<String> = Services::get(cx)
            .hypr
            .read(cx)
            .value
            .monitors
            .iter()
            .map(|m| m.name.clone())
            .collect();
        self.session
            .update(cx, |session, cx| session.toggle(monitor, &monitors, cx));
    }
}

/// The UUID gpui derives for a Wayland display from its output name ("DP-2"), so displays can be
/// matched with Hyprland's monitors by name.
pub fn monitor_uuid(name: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, name.as_bytes())
}

/// The display Hyprland calls `monitor`.
pub fn display_named(monitor: &str, cx: &App) -> Option<DisplayId> {
    let uuid = monitor_uuid(monitor);
    cx.displays()
        .into_iter()
        .find(|d| d.uuid().ok() == Some(uuid))
        .map(|d| d.id())
}

/// The monitor Hyprland knows by the display with `uuid`.
pub fn monitor_named(uuid: Uuid, hypr: &hyprland::State) -> Option<&hyprland::Monitor> {
    hypr.monitors.iter().find(|m| monitor_uuid(&m.name) == uuid)
}

/// Closes `window` once the current update is done. Closing it straight away fails (quietly)
/// when this runs inside one of that window's own handlers, since gpui has the window checked out
/// while it handles its events.
pub fn close_window(window: impl Into<AnyWindowHandle>, cx: &mut App) {
    let window = window.into();
    cx.defer(move |cx| {
        let _ = window.update(cx, |_, window, _| window.remove_window());
    });
}

/// Opens a layer-shell window of `size` (a zero side stretches between its anchors) on
/// `display`, with `build`'s view inside gpui-component's root.
pub fn open_layer(
    cx: &mut App,
    display: Option<DisplayId>,
    layer: LayerShellOptions,
    size: Size<Pixels>,
    background: WindowBackgroundAppearance,
    build: impl FnOnce(&mut Window, &mut App) -> AnyView,
) -> anyhow::Result<WindowHandle<Root>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(Pixels::ZERO, Pixels::ZERO),
            size,
        ))),
        titlebar: None,
        focus: false,
        kind: WindowKind::LayerShell(layer),
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: display,
        window_background: background,
        app_id: Some("shell".into()),
        ..WindowOptions::default()
    };
    cx.open_window(options, |window, cx| {
        let view = build(window, cx);
        cx.new(|cx| Root::new(view, window, cx))
    })
}
