//! The split slider (SplitSlider.qml): the track breaks around a bar-shaped handle, with an icon
//! riding in the fill (or just past the handle, once the fill is too short for it). Drag, click
//! or scroll.

use std::rc::Rc;

use gpui::{
    App, Bounds, Corners, ElementId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Rgba, ScrollDelta, ScrollEvent, TransformationMatrix, Window, canvas, div, fill, point,
    prelude::*, px, size,
};

use super::icon;
use crate::theme;

const GAP: f32 = 6.;
const HANDLE: f32 = 4.;
const HEIGHT: f32 = 46.;
const BAR: f32 = 36.;
const ICON: f32 = 16.;
/// The fill has room for the icon from this width.
const ICON_ROOM: f32 = 34.;

type OnChange = Rc<dyn Fn(f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SplitSlider {
    id: ElementId,
    value: f32,
    icon: &'static str,
    fill: Rgba,
    on_change: OnChange,
}

/// A 0–1 slider showing `value`, with `icon` riding in it.
pub fn split_slider(
    id: impl Into<ElementId>,
    value: f32,
    icon: &'static str,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> SplitSlider {
    SplitSlider {
        id: id.into(),
        value: value.clamp(0., 1.),
        icon,
        fill: theme::WHITE,
        on_change: Rc::new(on_change),
    }
}

impl SplitSlider {
    /// The fill's colour, e.g. dimmed while muted.
    pub fn fill(mut self, color: Rgba) -> Self {
        self.fill = color;
        self
    }
}

fn usable(bounds: Bounds<Pixels>) -> f32 {
    (f32::from(bounds.size.width) - GAP * 2. - HANDLE).max(1.)
}

fn value_at(bounds: Bounds<Pixels>, x: Pixels) -> f32 {
    ((f32::from(x - bounds.origin.x) - GAP) / usable(bounds)).clamp(0., 1.)
}

impl RenderOnce for SplitSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Whether a drag that started on the slider is going on, across frames.
        let dragging = window.use_keyed_state(self.id.clone(), cx, |_, _| false);
        let SplitSlider {
            id,
            value,
            icon: icon_name,
            fill: fill_color,
            on_change,
        } = self;

        let paint = move |b: Bounds<Pixels>, (): (), window: &mut Window, cx: &mut App| {
            let (x, y) = (f32::from(b.origin.x), f32::from(b.origin.y));
            let width = f32::from(b.size.width);
            let bar_y = y + (HEIGHT - BAR) / 2.;
            let fill_w = usable(b) * value;
            let rect = |x: f32, y: f32, w: f32, h: f32| {
                Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
            };
            // Round outer ends, nearly square inner ends against the handle.
            let corners = |tl: f32, tr: f32, br: f32, bl: f32| Corners {
                top_left: px(tl),
                top_right: px(tr),
                bottom_right: px(br),
                bottom_left: px(bl),
            };
            if fill_w > 0. {
                window.paint_quad(
                    fill(rect(x, bar_y, fill_w, BAR), fill_color)
                        .corner_radii(corners(12., 4., 4., 12.)),
                );
            }
            let handle_x = x + fill_w + GAP;
            window.paint_quad(
                fill(rect(handle_x, y, HANDLE, HEIGHT), theme::WHITE).corner_radii(px(2.)),
            );
            let track_x = handle_x + HANDLE + GAP;
            let track_w = (x + width - track_x).max(0.);
            if track_w > 0. {
                window.paint_quad(
                    fill(rect(track_x, bar_y, track_w, BAR), theme::TRACK)
                        .corner_radii(corners(4., 12., 12., 4.)),
                );
            }

            // The icon rides in the fill while it fits there, else just past the handle.
            let (icon_x, ink) = if fill_w >= ICON_ROOM {
                (x + 10., theme::INK)
            } else {
                (track_x + 10., theme::TEXT)
            };
            let _ = window.paint_svg(
                rect(icon_x, y + (HEIGHT - ICON) / 2., ICON, ICON),
                icon::path(icon_name),
                None,
                TransformationMatrix::unit(),
                ink.into(),
                cx,
            );

            // A drag carries on outside the slider, so its moves are watched window-wide.
            {
                let on_change = on_change.clone();
                let dragging = dragging.clone();
                window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                    if phase.bubble() && e.button == MouseButton::Left && b.contains(&e.position) {
                        cx.stop_propagation();
                        dragging.update(cx, |d, _| *d = true);
                        on_change(value_at(b, e.position.x), window, cx);
                    }
                });
            }
            {
                let on_change = on_change.clone();
                let dragging = dragging.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if phase.bubble() && *dragging.read(cx) {
                        if e.dragging() {
                            on_change(value_at(b, e.position.x), window, cx);
                        } else {
                            dragging.update(cx, |d, _| *d = false);
                        }
                    }
                });
            }
            {
                let dragging = dragging.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase.bubble() && *dragging.read(cx) {
                        dragging.update(cx, |d, _| *d = false);
                    }
                });
            }
            let on_change = on_change.clone();
            window.on_mouse_event(move |e: &ScrollEvent, phase, window, cx| {
                if !phase.bubble() || !b.contains(&e.position) {
                    return;
                }
                let y = match e.delta {
                    ScrollDelta::Pixels(p) => f32::from(p.y),
                    ScrollDelta::Lines(l) => l.y,
                };
                if y != 0. {
                    cx.stop_propagation();
                    let step = if y > 0. { 0.02 } else { -0.02 };
                    on_change((value + step).clamp(0., 1.), window, cx);
                }
            });
        };

        div()
            .id(id)
            .w_full()
            .h(px(HEIGHT))
            .cursor_pointer()
            .child(canvas(|_, _, _| (), paint).size_full())
    }
}
