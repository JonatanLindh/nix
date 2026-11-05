//! The capture overlay on one monitor (ScreenshotOverlay.qml), GNOME-style: a frozen frame of the
//! screen, dimmed outside what will be captured, and a toolbar (selection / screen / window,
//! photo / video, pointer, the capture button). State shared between monitors and the capture
//! itself live in the [`Screenshot`] model.
//!
//! Keys: esc closes, enter/space captures, s/c/w pick the mode, v toggles video, p the pointer.

use std::sync::Arc;

use gpui::component::Root;
use gpui::{
    App, Context, CursorStyle, Entity, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, Rgba, Size, Subscription, Window,
    WindowBackgroundAppearance, WindowHandle, div, img,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    prelude::*,
    px, rgba,
};

use super::geometry::{self, Drag, Rect};
use super::{Mode, Screenshot, Target};
use crate::services::screenshot::{Monitor, Snapshot, Still};
use crate::shell::{display_named, open_layer};
use crate::theme;
use crate::ui::{Width, bar_button, icon, small, vstack};

/// The dark glass of the toolbar and labels: more solid than panels, as there's no live blur
/// over the frozen frame.
const TOOLBAR: Rgba = Rgba {
    r: 14. / 255.,
    g: 26. / 255.,
    b: 30. / 255.,
    a: 0.88,
};
const LABEL: Rgba = Rgba { a: 0.85, ..TOOLBAR };
const DIM: Rgba = Rgba {
    r: 0.,
    g: 0.,
    b: 0.,
    a: 0.45,
};

const MODES: [(Mode, &str, &str); 3] = [
    (Mode::Selection, "select", "Selection"),
    (Mode::Screen, "monitor", "Screen"),
    (Mode::Window, "windows", "Window"),
];

/// An overlay window on each monitor in `snapshot` that has a still; the focused one takes the
/// keyboard.
pub(super) fn open_all(
    model: &Entity<Screenshot>,
    snapshot: &Arc<Snapshot>,
    cx: &mut App,
) -> Vec<WindowHandle<Root>> {
    let mut windows = Vec::new();
    for monitor in &snapshot.monitors {
        if !snapshot.stills.contains_key(&monitor.name) {
            continue;
        }
        let Some(display) = display_named(&monitor.name, cx) else {
            continue;
        };
        let window = open_layer(
            cx,
            Some(display),
            layer(monitor.focused),
            Size::default(),
            WindowBackgroundAppearance::Transparent,
            |window, cx| {
                let (model, snapshot, monitor) = (model.clone(), snapshot.clone(), monitor.clone());
                cx.new(|cx| Overlay::new(model, snapshot, monitor, window, cx))
                    .into()
            },
        );
        match window {
            Ok(window) => windows.push(window),
            Err(e) => eprintln!("screenshot: opening the overlay on {}: {e:#}", monitor.name),
        }
    }
    windows
}

/// A whole monitor, over everything, ignoring the bar's space.
fn layer(focused: bool) -> LayerShellOptions {
    LayerShellOptions {
        // Not "shell:…": the blur rule would blur the whole screen behind an opaque frame.
        namespace: "shell-screenshot".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: Some(px(-1.)),
        keyboard_interactivity: if focused {
            KeyboardInteractivity::Exclusive
        } else {
            KeyboardInteractivity::None
        },
        ..Default::default()
    }
}

/// A press on the frame, and what it's doing to the selection.
struct Press {
    start: (f32, f32),
    /// `None` outside selection mode.
    drag: Option<(Drag, Rect)>,
}

struct Overlay {
    model: Entity<Screenshot>,
    snapshot: Arc<Snapshot>,
    monitor: Monitor,
    focus: FocusHandle,
    /// The monitor's size in logical pixels, as of the last frame.
    size: (f32, f32),
    /// The selection, in this monitor's logical coordinates.
    sel: Option<Rect>,
    /// The window under the pointer (window mode), local coordinates.
    hovered: Option<Rect>,
    press: Option<Press>,
    _observer: Subscription,
}

impl Overlay {
    fn new(
        model: Entity<Screenshot>,
        snapshot: Arc<Snapshot>,
        monitor: Monitor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        if monitor.focused {
            window.focus(&focus, cx);
        }
        let sel = model.read(cx).selections.get(&monitor.name).copied();
        let observer = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            snapshot,
            monitor,
            focus,
            size: (0., 0.),
            sel,
            hovered: None,
            press: None,
            _observer: observer,
        }
    }

    fn still(&self) -> &Still {
        &self.snapshot.stills[&self.monitor.name]
    }

    /// What a capture would take right now, local coordinates.
    fn target(&self, cx: &App) -> Option<Rect> {
        let model = self.model.read(cx);
        match model.mode {
            Mode::Screen => Some(Rect::new(0., 0., self.size.0, self.size.1)),
            Mode::Window if model.pointer_monitor.as_deref() == Some(&self.monitor.name) => {
                self.hovered
            }
            Mode::Window => None,
            Mode::Selection => self.sel,
        }
    }

    /// The windows on show, in this monitor's coordinates.
    fn window_at(&self, x: f32, y: f32) -> Option<Rect> {
        let local: Vec<Rect> = self
            .snapshot
            .windows
            .iter()
            .map(|w| w.offset(-self.monitor.x, -self.monitor.y))
            .collect();
        geometry::window_at(&local, x, y)
    }

    fn capture(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.target(cx) else {
            return;
        };
        let (mode, video, pointer) = {
            let m = self.model.read(cx);
            (m.mode, m.video, m.pointer)
        };
        let (name, sel) = (self.monitor.name.clone(), self.sel);
        if !video {
            // Cut from the still taken as the overlay opened rather than grabbed again once
            // we're gone, so menus and hovers that close when the overlay takes focus still make
            // it in. The still's own scale: fractional scales make it 1.5 and the like.
            let still = self.still();
            let scale = still.width as f32 / self.size.0.max(1.);
            let height = (self.size.1 * scale).round() as u32;
            let crop = geometry::crop(target, scale, (still.width, height));
            let path = if pointer {
                still.pointer_path.clone()
            } else {
                still.plain_path.clone()
            };
            self.model.update(cx, |m, cx| {
                if mode == Mode::Selection {
                    m.remember(&name, sel);
                }
                m.save_frozen(path, crop, cx);
            });
            return;
        }
        let record = match mode {
            Mode::Screen => Target::Output(name.clone()),
            _ => Target::Area(target.offset(self.monitor.x, self.monitor.y)),
        };
        self.model.update(cx, |m, cx| {
            if mode == Mode::Selection {
                m.remember(&name, sel);
            }
            m.record(record, cx);
        });
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let model = self.model.clone();
        let set = |cx: &mut Context<Self>, f: &dyn Fn(&mut Screenshot)| {
            model.update(cx, |m, cx| {
                f(m);
                cx.notify();
            });
        };
        match event.keystroke.key.as_str() {
            "escape" => self.model.update(cx, |m, cx| m.close(cx)),
            "enter" | "space" => self.capture(cx),
            "s" => set(cx, &|m| m.mode = Mode::Selection),
            "c" => set(cx, &|m| m.mode = Mode::Screen),
            "w" => set(cx, &|m| m.mode = Mode::Window),
            "v" => set(cx, &|m| m.video = !m.video),
            "p" => set(cx, &|m| m.pointer = !m.pointer),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let start = (f32::from(event.position.x), f32::from(event.position.y));
        let drag =
            (self.model.read(cx).mode == Mode::Selection).then(|| {
                match geometry::handle_at(self.sel, start.0, start.1) {
                    Some(drag) => (drag, self.sel.unwrap_or_default()),
                    None => (Drag::New, Rect::new(start.0, start.1, 0., 0.)),
                }
            });
        self.press = Some(Press { start, drag });
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
        let (mode, here) = {
            let m = self.model.read(cx);
            (
                m.mode,
                m.pointer_monitor.as_deref() == Some(&self.monitor.name),
            )
        };
        if !here {
            let name = self.monitor.name.clone();
            self.model.update(cx, |m, cx| {
                m.pointer_monitor = Some(name);
                cx.notify();
            });
        }
        if mode == Mode::Window {
            let hovered = self.window_at(x, y);
            if hovered != self.hovered {
                self.hovered = hovered;
                cx.notify();
            }
            return;
        }
        if let Some(Press {
            start,
            drag: Some((drag, from)),
        }) = self.press
        {
            self.sel = Some(geometry::dragged(drag, start, from, (x, y), self.size));
            cx.notify();
        }
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        // Only for presses that began on the frame, not on the toolbar.
        if self.press.take().is_none() {
            return;
        }
        match self.model.read(cx).mode {
            Mode::Selection => self.sel = geometry::settled(self.sel),
            // Screen and window modes capture on click.
            Mode::Window => {
                let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
                self.hovered = self.window_at(x, y);
                // A click with no move before it still counts as the pointer being here.
                let name = self.monitor.name.clone();
                self.model
                    .update(cx, |m, _| m.pointer_monitor = Some(name));
                self.capture(cx);
            }
            Mode::Screen => self.capture(cx),
        }
        cx.notify();
    }
}

impl Render for Overlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        self.size = (f32::from(viewport.width), f32::from(viewport.height));
        let (width, height) = self.size;
        let target = self.target(cx);
        let model = self.model.read(cx);
        let (mode, video, pointer) = (model.mode, model.video, model.pointer);
        let still = self.still();
        let frame = if pointer {
            still.pointer.clone()
        } else {
            still.plain.clone()
        };

        // Dim everything outside the target (all of it when there's none yet).
        let cut = target.unwrap_or_default();
        let band = |r: Rect| at(r).bg(DIM);
        let bands = geometry::dim_bands(cut, width, height).map(band);

        let mut root = crate::ui::root()
            .id("overlay")
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus)
            .cursor(if mode == Mode::Selection {
                CursorStyle::Crosshair
            } else {
                CursorStyle::PointingHand
            })
            .on_key_down(cx.listener(|this, e, _, cx| this.key(e, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.mouse_down(e, cx)),
            )
            .on_mouse_move(cx.listener(|this, e, _, cx| this.mouse_move(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.mouse_up(e, cx)),
            )
            .child(
                img(frame)
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .children(bands);

        // Outline of the target (not for the whole screen).
        if target.is_some() && mode != Mode::Screen {
            root = root.child(
                at(Rect::new(cut.x - 1., cut.y - 1., cut.w + 2., cut.h + 2.))
                    .border_2()
                    .border_color(rgba(0xffffffff)),
            );
        }

        if mode == Mode::Selection
            && let Some(sel) = self.sel
        {
            // Size of the selection, under it, while it isn't being dragged.
            if self.press.is_none() {
                root = root.child(
                    div()
                        .absolute()
                        .left(px(sel.x))
                        .top(px(geometry::label_top(sel, 24., height)))
                        .h(px(24.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(12.))
                        .bg(LABEL)
                        .font_family(theme::MONO)
                        .child(small(geometry::size_label(sel), theme::TEXT)),
                );
            }
            // Corner handles.
            for (fx, fy) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
                let (cx_, cy) = (sel.x + sel.w * fx, sel.y + sel.h * fy);
                root = root.child(
                    at(Rect::new(cx_ - 8., cy - 8., 16., 16.))
                        .rounded(px(8.))
                        .bg(rgba(0xffffffff))
                        .border_1()
                        .border_color(rgba(0x00000040)),
                );
            }
        }

        // The controls are on the focused monitor only.
        if self.monitor.focused {
            root = root.child(self.close_button()).child(self.toolbar(
                mode,
                video,
                pointer,
                target.is_some(),
                cx,
            ));
        }
        root
    }
}

/// An absolutely placed box over `r`.
fn at(r: Rect) -> gpui::Div {
    div()
        .absolute()
        .left(px(r.x))
        .top(px(r.y))
        .w(px(r.w.max(0.)))
        .h(px(r.h.max(0.)))
}

/// Presses here don't start a selection underneath.
fn shield(el: gpui::Div) -> gpui::Div {
    el.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl Overlay {
    fn close_button(&self) -> impl IntoElement {
        let model = self.model.clone();
        shield(div().absolute().top(px(24.)).right(px(24.))).child(
            bar_button("close", icon("close", 20., theme::TEXT))
                .height(48.)
                .width(Width::Px(48.))
                .radius(24.)
                .padding(0.)
                .centered()
                .rest(LABEL)
                .outline(Some(theme::GLASS_BORDER))
                .on_click(move |_, cx| model.update(cx, |m, cx| m.close(cx))),
        )
    }

    fn toolbar(
        &self,
        mode: Mode,
        video: bool,
        pointer: bool,
        can_capture: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let model = &self.model;
        let set = |f: fn(&mut Screenshot)| {
            let model = model.clone();
            move |_: &mut Window, cx: &mut App| {
                model.update(cx, |m, cx| {
                    f(m);
                    cx.notify();
                })
            }
        };

        let modes = crate::ui::hstack(8.).children(MODES.iter().map(|&(m, name, text)| {
            let content = vstack(8.)
                .items_center()
                .child(icon(name, 26., theme::TEXT))
                .child(small(text, theme::TEXT));
            let pick: fn(&mut Screenshot) = match m {
                Mode::Selection => |s| s.mode = Mode::Selection,
                Mode::Screen => |s| s.mode = Mode::Screen,
                Mode::Window => |s| s.mode = Mode::Window,
            };
            bar_button(text, content)
                .height(72.)
                .width(Width::Px(92.))
                .radius(16.)
                .padding(0.)
                .centered()
                .active(mode == m)
                .on_click(set(pick))
        }));

        // Photo / video.
        let kind = |id: &'static str, on: bool, name: &str, pick: fn(&mut Screenshot)| {
            let ink = if on { theme::INK } else { theme::TEXT };
            bar_button(id, icon(name, 17., ink))
                .height(30.)
                .width(Width::Px(40.))
                .radius(15.)
                .padding(0.)
                .centered()
                .rest(if on { theme::WHITE } else { theme::TRANSPARENT })
                .on_click(set(pick))
        };
        let kinds = crate::ui::hstack(2.)
            .h(px(36.))
            .px(px(2.))
            .rounded(px(18.))
            .bg(theme::FILL)
            .child(kind("photo", !video, "photo", |s| s.video = false))
            .child(kind("video", video, "video", |s| s.video = true));

        // The capture button: a white ring around a disc (red for video).
        let shutter = div()
            .id("shutter")
            .group("shutter")
            .size(px(64.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(32.))
            .border_4()
            .border_color(rgba(0xffffffff))
            .when(!can_capture, |d| d.opacity(0.5))
            .when(can_capture, |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.capture(cx)))
            })
            .child(
                div()
                    .id("disc")
                    .size(px(48.))
                    .rounded(px(24.))
                    .bg(if video { theme::WARN } else { theme::WHITE })
                    .when(can_capture, |d| {
                        d.group_active("shutter", |s| s.size(px(42.)).rounded(px(21.)))
                    }),
            );

        // Include the pointer (screenshots only; recordings always show it).
        let ink = if pointer { theme::INK } else { theme::TEXT };
        let mut pointer_button = bar_button("pointer", icon("pointer", 17., ink))
            .height(40.)
            .width(Width::Px(40.))
            .radius(20.)
            .padding(0.)
            .centered()
            .rest(if pointer { theme::WHITE } else { theme::FILL });
        if !video {
            pointer_button = pointer_button.on_click(set(|s| s.pointer = !s.pointer));
        }
        let pointer_button = div().when(video, |d| d.opacity(0.4)).child(pointer_button);

        let controls = div()
            .relative()
            .w_full()
            .h(px(64.))
            .flex()
            .items_center()
            .justify_center()
            .child(div().absolute().left_0().top(px(14.)).child(kinds))
            .child(shutter)
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top(px(12.))
                    .child(pointer_button),
            );

        let bar = shield(
            div()
                .p(px(16.))
                .rounded(px(28.))
                .bg(TOOLBAR)
                .border_1()
                .border_color(theme::GLASS_BORDER)
                .cursor(CursorStyle::Arrow),
        )
        .child(vstack(14.).child(modes).child(controls));

        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(48.))
            .flex()
            .justify_center()
            .child(bar)
    }
}
