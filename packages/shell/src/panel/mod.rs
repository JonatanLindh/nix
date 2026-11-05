//! The panels that drop down from the bar, each a popup window of its own: the window is the
//! glass card, blurred behind by the compositor, `PANEL_GAP` under the bar.
//!
//! A panel's width is fixed per kind; its height follows its content (see [`Popup`]). Clicking
//! anywhere outside closes it, and so does pressing its bar button again. Opened from the bar,
//! that's the popup's own grab; opened from a keybind there's no press to grab with, so the bar
//! holds a Hyprland focus grab instead (see `Bar::toggle`).

mod audio;
mod bluetooth;
mod calendar;
mod network;
mod picker;
mod power;
mod tailscale;
mod tray;

use gpui::component::Root;
use gpui::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext, Bounds, Context, Corners, Entity,
    SharedString, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, canvas, div, point,
    popup::{PopupAnchor, PopupConstraintAdjustment, PopupGravity, PopupOptions},
    prelude::*,
    px, size,
};

use crate::bar::Bar;
use crate::theme;
use crate::ui::controls::circle_button;
use crate::ui::{self, divider, hstack, label, vstack};

#[derive(Debug, Clone, PartialEq)]
pub enum PanelKind {
    Audio,
    Bluetooth,
    Calendar,
    Network,
    Notifications,
    Picker,
    /// Battery, brightness and the power profile.
    Power,
    Tailscale,
    /// A tray item's menu, hanging under its icon at `x` on the bar.
    Tray {
        address: String,
        x: f32,
    },
}

/// Where a panel hangs along the bar.
enum Side {
    Left,
    Center,
    Right,
    At(f32),
}

/// Distance from the bar's edge for panels at its sides.
const SIDE_MARGIN: f32 = 8.;
/// A popup opens this tall, then fits its card once that has been laid out.
const INITIAL_HEIGHT: f32 = 480.;

impl PanelKind {
    fn width(&self) -> f32 {
        match self {
            PanelKind::Audio
            | PanelKind::Bluetooth
            | PanelKind::Network
            | PanelKind::Notifications
            | PanelKind::Power
            | PanelKind::Tailscale => theme::PANEL_WIDTH,
            PanelKind::Calendar => calendar::WIDTH,
            PanelKind::Picker => picker::WIDTH,
            PanelKind::Tray { .. } => tray::WIDTH,
        }
    }

    /// It has a text field or keys of its own, so its bar takes the keyboard while it's open.
    pub fn takes_keyboard(&self) -> bool {
        matches!(
            self,
            PanelKind::Network | PanelKind::Notifications | PanelKind::Tailscale
        )
    }

    fn radius(&self) -> f32 {
        match self {
            PanelKind::Tray { .. } => tray::RADIUS,
            _ => theme::PANEL_RADIUS,
        }
    }

    fn side(&self) -> Side {
        match self {
            PanelKind::Audio
            | PanelKind::Bluetooth
            | PanelKind::Network
            | PanelKind::Notifications
            | PanelKind::Power
            | PanelKind::Tailscale => Side::Right,
            PanelKind::Calendar => Side::Center,
            PanelKind::Picker => Side::Left,
            PanelKind::Tray { x, .. } => Side::At(*x),
        }
    }

    fn view(&self, host: Host, window: &mut Window, cx: &mut App) -> AnyView {
        match self {
            PanelKind::Audio => cx.new(audio::AudioPanel::new).into(),
            PanelKind::Bluetooth => cx.new(bluetooth::BluetoothPanel::new).into(),
            PanelKind::Calendar => cx.new(calendar::CalendarPanel::new).into(),
            PanelKind::Network => cx
                .new(|cx| network::NetworkPanel::new(host, window, cx))
                .into(),
            PanelKind::Notifications => cx
                .new(|cx| crate::notifications::Center::new(window, cx))
                .into(),
            PanelKind::Picker => cx.new(picker::PickerPanel::new).into(),
            PanelKind::Power => cx.new(|cx| power::PowerPanel::new(host, cx)).into(),
            PanelKind::Tailscale => cx
                .new(|cx| tailscale::TailscalePanel::new(host, window, cx))
                .into(),
            PanelKind::Tray { address, .. } => {
                let address = address.clone();
                cx.new(|cx| tray::TrayMenu::new(address, cx)).into()
            }
        }
    }
}

/// What a panel knows of the bar it hangs from.
#[derive(Clone)]
pub struct Host {
    pub bar: WeakEntity<Bar>,
    /// The panel this one was opened from, which its back arrow returns to.
    pub from: Option<PanelKind>,
}

impl Host {
    /// Swaps this panel for `kind`; `from` gets a back arrow to it.
    pub fn swap(&self, kind: PanelKind, from: Option<PanelKind>, cx: &mut App) {
        let _ = self
            .bar
            .update(cx, |bar, cx| bar.swap_panel(kind, from, cx));
    }
}

/// Opens `kind` under the bar in `parent`, which is `bar_width` wide, taking the popup grab if
/// `grab` (only right after a press on the bar). Returns the window and its popup view, which is
/// released when the window closes.
pub fn open(
    kind: PanelKind,
    host: Host,
    parent: AnyWindowHandle,
    bar_width: f32,
    grab: bool,
    cx: &mut App,
) -> anyhow::Result<(WindowHandle<Root>, Entity<Popup>)> {
    let width = kind.width();
    let x = match kind.side() {
        Side::Left => SIDE_MARGIN,
        Side::Center => (bar_width - width) / 2.,
        Side::Right => bar_width - width - SIDE_MARGIN,
        // Under its icon, but kept on the bar.
        Side::At(x) => x.min(bar_width - width - SIDE_MARGIN),
    }
    .max(0.);
    let popup = PopupOptions {
        parent,
        // A sliver of the bar's bottom edge at `x`; the panel hangs from its left end.
        anchor_rect: Bounds::new(point(px(x), px(0.)), size(px(1.), px(theme::BAR_HEIGHT))),
        anchor: PopupAnchor::BottomLeft,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment: PopupConstraintAdjustment::SLIDE_X
            | PopupConstraintAdjustment::RESIZE_Y,
        offset: point(px(0.), px(theme::PANEL_GAP)),
        grab,
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(width), px(INITIAL_HEIGHT)),
        ))),
        titlebar: None,
        kind: WindowKind::AnchoredPopup(popup),
        is_movable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Blurred,
        app_id: Some("shell".into()),
        ..WindowOptions::default()
    };
    let radius = kind.radius();
    let mut popup = None;
    let window = cx.open_window(options, |window, cx| {
        // The blur behind follows the card's corners.
        window.set_client_corner_radii(Corners::all(px(radius)));
        let content = kind.view(host, window, cx);
        let view = cx.new(|_| Popup { content });
        popup = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    Ok((window, popup.expect("open_window ran its build closure")))
}

/// A panel's window: its content at the content's own height, with the window fitted to it.
///
/// The content is laid out free of the window's height (it doesn't shrink to fit), measured,
/// and the window resized to match, so a panel that grows (a dropdown unfolding) grows its
/// window too.
pub struct Popup {
    content: AnyView,
}

impl Render for Popup {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let fit = canvas(
            |bounds, window, cx| {
                let height = bounds.size.height.ceil();
                let viewport = window.viewport_size();
                if height > px(0.) && (height - viewport.height).abs() >= px(1.) {
                    window.defer(cx, move |window, _| {
                        let width = window.viewport_size().width;
                        window.resize(size(width, height));
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        ui::root().child(
            div()
                .relative()
                .flex_none()
                .w_full()
                .child(self.content.clone())
                .child(fit),
        )
    }
}

/// The glass card every panel sits in (PanelFrame.qml): a header with the title and actions on
/// the right, the body, and an optional footer under a hairline.
pub struct Frame {
    icon: Option<&'static str>,
    title: SharedString,
    back: Option<AnyElement>,
    actions: Vec<AnyElement>,
    footer: Vec<AnyElement>,
}

pub fn frame(title: impl Into<SharedString>) -> Frame {
    Frame {
        icon: None,
        title: title.into(),
        back: None,
        actions: Vec::new(),
        footer: Vec::new(),
    }
}

impl Frame {
    /// An icon before the title.
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }

    /// A back arrow before the title, when the panel was opened from another one (Tailscale from
    /// the network panel), which returns there.
    pub fn back(mut self, host: &Host) -> Self {
        if let Some(from) = host.from.clone() {
            let host = host.clone();
            self.back = Some(
                circle_button("back", "chevron-left", theme::TEXT)
                    .on_click(move |_, cx| host.swap(from.clone(), None, cx))
                    .into_any_element(),
            );
        }
        self
    }

    /// Something at the right of the header (a switch, buttons).
    pub fn action(mut self, element: impl IntoElement) -> Self {
        self.actions.push(element.into_any_element());
        self
    }

    /// Something in the footer row; a `div().flex_1()` pushes the rest right.
    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer.push(element.into_any_element());
        self
    }

    /// The card around `body`.
    pub fn body(self, body: impl IntoElement) -> impl IntoElement {
        let header = hstack(8.)
            .pl(px(if self.back.is_some() { 0. } else { 6. }))
            .pr(px(2.))
            .children(self.back)
            .children(
                self.icon
                    .map(|name| ui::icon(name, 17., theme::alpha(theme::TEXT, 0.75))),
            )
            .child(
                label(self.title, theme::TEXT)
                    .flex_1()
                    .font_weight(theme::SEMIBOLD)
                    .text_size(px(theme::TITLE)),
            )
            .children(self.actions);
        let mut content = vstack(10.).child(header).child(body);
        if !self.footer.is_empty() {
            content = content
                .child(div().px(px(4.)).child(divider()))
                .child(hstack(8.).children(self.footer));
        }
        ui::glass(theme::PANEL_RADIUS)
            .w_full()
            .p(px(theme::PANEL_PADDING))
            .child(content)
    }
}
