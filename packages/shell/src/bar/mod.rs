//! One bar per display: workspaces on the left, the clock centred, status icons on the right.
//! Panels hang from it as popups, one at a time.

mod battery;
mod clock;
mod picker;
mod privacy;
mod status;
mod tray;
mod workspaces;

use std::time::{Duration, Instant};

use gpui::component::Root;
use gpui::{
    AnyWindowHandle, App, Context, FocusHandle, KeyDownEvent, Subscription, Window, WindowHandle,
    div,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    prelude::*,
    px,
};
use uuid::Uuid;

use crate::panel::{self, PanelKind};
use crate::services::Services;
use crate::shell::{Surfaces, monitor_named};
use crate::wayland::{self, FocusGrab};
use crate::{theme, ui};

/// The bars' layer namespace.
pub const NAMESPACE: &str = "shell";

/// The bar's surface: across the top, reserving its height, never taking the keyboard.
pub fn layer() -> LayerShellOptions {
    LayerShellOptions {
        namespace: NAMESPACE.into(),
        layer: Layer::Top,
        anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: Some(px(theme::BAR_HEIGHT)),
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    }
}

pub struct Bar {
    /// The display's UUID, which names the monitor it is to Hyprland (see `shell::monitor_uuid`).
    pub uuid: Option<Uuid>,
    /// The bar's own window, which its panels hang from.
    window: AnyWindowHandle,
    panel: Option<OpenPanel>,
    /// The panel that closed last, and when (see `toggle_panel`).
    last_closed: Option<(PanelKind, Instant)>,
    services: Services,
    /// The shell's other surfaces, which the bar has buttons for.
    pub surfaces: Surfaces,
    /// Focused while a panel holds a focus grab: the keyboard then comes to the bar, not the
    /// panel (a popup only gets it through its own grab), and the bar passes it on.
    focus: FocusHandle,
    _observers: Vec<Subscription>,
}

struct OpenPanel {
    kind: PanelKind,
    window: WindowHandle<Root>,
    /// Opened from a keybind: keeps input on the panel and the bar, and closes the panel on a
    /// click anywhere else. Dropping it ends it.
    _focus_grab: Option<FocusGrab>,
    _released: Subscription,
}

impl Bar {
    pub fn new(
        uuid: Option<Uuid>,
        window: AnyWindowHandle,
        surfaces: Surfaces,
        cx: &mut Context<Self>,
    ) -> Self {
        let services = Services::get(cx);
        let Surfaces {
            shelf,
            notifications,
            session,
            ..
        } = surfaces.clone();
        let screenshot = crate::screenshot::Screenshot::global(cx);
        let observers = vec![
            cx.observe(&services.hypr, ui::redraw),
            cx.observe(&services.clock, ui::redraw),
            cx.observe(&services.audio, ui::redraw),
            cx.observe(&services.network, ui::redraw),
            cx.observe(&services.bluetooth, ui::redraw),
            cx.observe(&services.battery, ui::redraw),
            cx.observe(&services.tray, ui::redraw),
            cx.observe(&services.caffeine, ui::redraw),
            cx.observe(&services.picker, ui::redraw),
            cx.observe(&services.privacy, ui::redraw),
            cx.observe(&services.tailscale, ui::redraw),
            cx.observe(&services.power_profiles, ui::redraw),
            cx.observe(&shelf, ui::redraw),
            cx.observe(&notifications, ui::redraw),
            cx.observe(&session, ui::redraw),
            cx.observe(&screenshot, ui::redraw),
        ];
        Self {
            uuid,
            window,
            panel: None,
            last_closed: None,
            services,
            surfaces,
            focus: cx.focus_handle(),
            _observers: observers,
        }
    }

    /// The monitor's name ("DP-2"), once Hyprland has reported it.
    pub fn monitor(&self, cx: &App) -> Option<String> {
        let hypr = self.services.hypr.read(cx);
        monitor_named(self.uuid?, &hypr.value).map(|m| m.name.clone())
    }

    pub fn is_open(&self, kind: &PanelKind) -> bool {
        self.panel.as_ref().is_some_and(|p| p.kind == *kind)
    }

    pub fn panel_open(&self) -> bool {
        self.panel.is_some()
    }

    /// The panel open from this bar, if any.
    pub fn open_panel_kind(&self) -> Option<&PanelKind> {
        self.panel.as_ref().map(|p| &p.kind)
    }

    /// One panel open at a time: opens `kind`, closes it if it's open, or swaps to it. Called from
    /// a press on the bar (`window` is the bar's), so the popup can take its input grab.
    pub fn toggle_panel(&mut self, kind: PanelKind, window: &mut Window, cx: &mut Context<Self>) {
        let width = f32::from(window.viewport_size().width);
        self.toggle(kind, width, Some(window), cx);
    }

    /// Like `toggle_panel`, for a keyboard shortcut: there's no press for the popup to grab with,
    /// so a Hyprland focus grab closes it on a click outside instead.
    pub fn toggle_panel_from_keyboard(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        let width = self
            .window
            .update(cx, |_, window, _| f32::from(window.viewport_size().width))
            .unwrap_or(0.);
        self.toggle(kind, width, None, cx);
    }

    fn toggle(
        &mut self,
        kind: PanelKind,
        bar_width: f32,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let Some(open) = self.panel.take() {
            let same = open.kind == kind;
            crate::shell::close_window(open.window, cx);
            if same {
                return;
            }
        }

        // A click on the bar while a panel is open can first make the compositor dismiss the
        // panel; if that click also reaches us, it mustn't open the same panel straight back up.
        if self
            .last_closed
            .as_ref()
            .is_some_and(|(k, at)| *k == kind && at.elapsed() < Duration::from_millis(300))
        {
            return;
        }
        self.open_panel(kind, None, bar_width, window, cx);
    }

    /// Swaps the open panel for `kind`, from a click inside it; `from` gets a back arrow to it.
    /// Done once that click has been handled, since the panel's window can't close inside its
    /// own handlers.
    pub fn swap_panel(&mut self, kind: PanelKind, from: Option<PanelKind>, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = this.update(cx, |bar, cx| {
                if let Some(open) = bar.panel.take() {
                    // Gone before the next one opens, so that one's grab has no popup above it.
                    let _ = open
                        .window
                        .update(cx, |_, window, _| window.remove_window());
                }
                let width = bar
                    .window
                    .update(cx, |_, window, _| f32::from(window.viewport_size().width))
                    .unwrap_or(0.);
                bar.open_panel(kind, from, width, None, cx);
            });
        });
    }

    fn open_panel(
        &mut self,
        kind: PanelKind,
        from: Option<PanelKind>,
        bar_width: f32,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        // Without a press on the bar (a keybind, or a swap from inside a panel) the popup can't
        // take its own grab, so a focus grab stands in for it.
        let from_keyboard = window.is_none();
        if kind == PanelKind::Notifications {
            let notifications = self.surfaces.notifications.clone();
            notifications.update(cx, |n, cx| n.seen(cx));
        }
        // A panel with a text field needs the keyboard, which popups get through their bar.
        if kind.takes_keyboard() {
            match window {
                Some(window) => window.set_keyboard_interactivity(KeyboardInteractivity::OnDemand),
                None => self.set_keyboard(KeyboardInteractivity::OnDemand, cx),
            }
        }

        let host = panel::Host {
            bar: cx.entity().downgrade(),
            from,
        };
        match panel::open(
            kind.clone(),
            host,
            self.window,
            bar_width,
            !from_keyboard,
            cx,
        ) {
            Ok((window, view)) => {
                let released = cx.observe_release(&view, {
                    let kind = kind.clone();
                    move |bar, _, cx| bar.panel_closed(kind, window, cx)
                });
                let focus_grab = from_keyboard.then(|| self.focus_grab(window, cx)).flatten();
                self.panel = Some(OpenPanel {
                    kind,
                    window,
                    _focus_grab: focus_grab,
                    _released: released,
                });
            }
            Err(e) => eprintln!("panel: {e:#}"),
        }
        cx.notify();
    }

    /// The panel's window went away: closed by us, or dismissed by the compositor.
    fn panel_closed(
        &mut self,
        kind: PanelKind,
        window: WindowHandle<Root>,
        cx: &mut Context<Self>,
    ) {
        if self.panel.as_ref().is_some_and(|p| p.window == window) {
            self.panel = None;
        }
        // Unless it was swapped for another panel that types too.
        if kind.takes_keyboard() && !self.panel.as_ref().is_some_and(|p| p.kind.takes_keyboard()) {
            self.set_keyboard(KeyboardInteractivity::None, cx);
        }
        self.last_closed = Some((kind, Instant::now()));
        cx.notify();
    }

    /// Keeps input on `panel` and the bar (so its buttons still work), and closes `panel` when a
    /// click elsewhere clears the grab.
    fn focus_grab(&self, panel: WindowHandle<Root>, cx: &mut Context<Self>) -> Option<FocusGrab> {
        let (grab, cleared) = panel
            .update(cx, |_, window, _| wayland::focus_grab(window))
            .ok()
            .flatten()?;
        let _ = panel.update(cx, |_, window, _| grab.add(window));
        let focus = self.focus.clone();
        let _ = self.window.update(cx, |_, window, cx| {
            grab.add(window);
            window.focus(&focus, cx);
        });
        grab.commit();
        cx.spawn(async move |bar, cx| {
            // Cancelled instead when the grab is dropped first, with the panel.
            if cleared.await.is_ok() {
                let _ = bar.update(cx, |bar, cx| {
                    if bar.panel.as_ref().is_some_and(|p| p.window == panel) {
                        bar.close_panel(cx);
                    }
                });
            }
        })
        .detach();
        Some(grab)
    }

    /// A key that reached the bar while its panel holds a focus grab: the panel's.
    fn forward_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(open) = self.panel.as_ref().filter(|p| p._focus_grab.is_some()) else {
            return;
        };
        let keystroke = event.keystroke.clone();
        // Untyped, so the update doesn't hold the panel's root view while its handlers run.
        let window = AnyWindowHandle::from(open.window);
        cx.stop_propagation();
        // Once this event is over: the panel's handlers may close their own window.
        App::defer(cx, move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            });
        });
    }

    pub fn close_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(open) = self.panel.take() {
            crate::shell::close_window(open.window, cx);
            cx.notify();
        }
    }

    fn set_keyboard(&self, mode: KeyboardInteractivity, cx: &mut App) {
        let _ = self
            .window
            .update(cx, |_, window, _| window.set_keyboard_interactivity(mode));
    }
}

impl Render for Bar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let monitor = self.monitor(cx);
        let shelf = self.surfaces.shelf.clone();

        let picker = picker::slot(self, cx);
        let mut left = ui::hstack(theme::GAP)
            .flex_1()
            .min_w_0()
            .child(crate::shelf::button(&shelf, monitor.clone(), cx))
            .children(picker)
            .child(ui::separator());
        if let Some(monitor) = &monitor {
            left = left.child(workspaces::view(
                &self.services.hypr.read(cx).value,
                monitor,
            ));
        }
        // Last, since it grows and shrinks as apps come and go.
        let tray = self.services.tray.read(cx).value.clone();
        if !tray.is_empty() {
            let menu = self.tray_menu().map(str::to_string);
            left = left
                .child(ui::separator())
                .child(tray::view(&tray, menu.as_deref(), cx));
        }

        // Centred on the bar itself: the sides take equal shares around it.
        let now = self.services.clock.read(cx).value;
        let clock = clock::view(&now, self.is_open(&PanelKind::Calendar), cx);
        let right = ui::hstack(2.)
            .flex_1()
            .justify_end()
            .child(status::view(self, monitor, cx));

        ui::root()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|bar, event, _, cx| bar.forward_key(event, cx)))
            .flex()
            .items_center()
            .px(px(theme::BAR_PADDING))
            .bg(theme::BAR_BG)
            .child(left)
            .child(div().flex_none().child(clock))
            .child(right)
    }
}

impl Bar {
    /// The tray item whose menu is open from this bar.
    fn tray_menu(&self) -> Option<&str> {
        match self.panel.as_ref().map(|p| &p.kind) {
            Some(PanelKind::Tray { address, .. }) => Some(address),
            _ => None,
        }
    }
}
