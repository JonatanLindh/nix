//! The clickable item used across the bar and its panels (IconButton.qml): content on a rounded
//! target that lights up on hover and stays lit while `active`. Left press, left click, right
//! and middle press and the wheel each take a handler; handlers get the button's bounds in its
//! window, to hang a popup under it.
//!
//! Panels open on *press*, not click: a popup can only take its input grab while the button that
//! opened it is still held.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::base::{Easing, Transition, transition};
use gpui::{
    AnyElement, App, Bounds, ElementId, MouseButton, Pixels, Rgba, ScrollDelta, SharedString,
    Window, canvas, div, prelude::*, px,
};

use crate::theme;

/// How long the hover background takes to fade, as Theme.fast in QML.
const FADE: Duration = Duration::from_millis(120);

type Handler = Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App)>;
type ScrollHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(Clone, Copy)]
pub enum Width {
    /// As wide as the content and padding.
    Auto,
    /// The whole row.
    Full,
    Px(f32),
}

#[derive(IntoElement)]
pub struct BarButton {
    id: ElementId,
    content: AnyElement,
    on_press: Option<Handler>,
    on_click: Option<Handler>,
    on_right: Option<Handler>,
    on_middle: Option<Handler>,
    on_scroll: Option<ScrollHandler>,
    active: bool,
    rest: Rgba,
    radius: f32,
    height: f32,
    width: Width,
    padding: f32,
    centered: bool,
    outline: Option<Rgba>,
    badge: Option<String>,
}

/// A bar item around `content`. It shows hover only once something handles its clicks.
pub fn bar_button(id: impl Into<ElementId>, content: impl IntoElement) -> BarButton {
    BarButton {
        id: id.into(),
        content: content.into_any_element(),
        on_press: None,
        on_click: None,
        on_right: None,
        on_middle: None,
        on_scroll: None,
        active: false,
        rest: theme::TRANSPARENT,
        radius: theme::BUTTON_RADIUS,
        height: 26.,
        width: Width::Auto,
        padding: 8.,
        centered: false,
        outline: None,
        badge: None,
    }
}

impl BarButton {
    /// On left press, with the button's bounds. For buttons that open a popup.
    pub fn on_press(mut self, f: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(f));
        self
    }

    /// On left click (press and release over it).
    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(move |_, window, cx| f(window, cx)));
        self
    }

    pub fn on_right(mut self, f: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static) -> Self {
        self.on_right = Some(Rc::new(f));
        self
    }

    pub fn on_middle(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_middle = Some(Rc::new(move |_, window, cx| f(window, cx)));
        self
    }

    /// On the wheel: `true` for up.
    pub fn on_scroll(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_scroll = Some(Rc::new(f));
        self
    }

    /// Keeps the highlight on, e.g. while this item's panel is open.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Background when neither hovered nor active, e.g. a tinted "on" state.
    pub fn rest(mut self, color: Rgba) -> Self {
        self.rest = color;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn width(mut self, width: Width) -> Self {
        self.width = width;
        self
    }

    /// Horizontal padding.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Centres the content horizontally (it sits at the start by default).
    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    /// A hairline around it, e.g. for a selected day.
    pub fn outline(mut self, color: Option<Rgba>) -> Self {
        self.outline = color;
        self
    }

    /// A small count bubble in the top-right corner.
    pub fn badge(mut self, badge: Option<String>) -> Self {
        self.badge = badge;
        self
    }

    fn interactive(&self) -> bool {
        self.on_press.is_some()
            || self.on_click.is_some()
            || self.on_right.is_some()
            || self.on_middle.is_some()
            || self.on_scroll.is_some()
    }
}

impl RenderOnce for BarButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let interactive = self.interactive();
        // The hover background fades in and out rather than snapping.
        let hovered = window.use_keyed_state(self.id.clone(), cx, |_, _| false);
        let target = if interactive && *hovered.read(cx) {
            1.
        } else {
            0.
        };
        let fade = Transition::new(FADE).easing(Easing::EaseOut);
        let t: f32 = transition(
            SharedString::from(format!("{:?}/hover", self.id)),
            target,
            fade,
            window,
            cx,
        );
        // Where the button was laid out this frame, for the handlers.
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let measure = {
            let bounds = bounds.clone();
            canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                .absolute()
                .top_0()
                .left_0()
                .size_full()
        };

        let mut el = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(self.height))
            .px(px(self.padding))
            .rounded(px(self.radius))
            .bg(if self.active {
                theme::FILL_ACTIVE
            } else {
                mix(self.rest, theme::FILL_HOVER, t)
            })
            .when(self.centered, |d| d.justify_center())
            .child(self.content)
            .child(measure);
        el = match self.width {
            Width::Auto => el,
            Width::Full => el.w_full(),
            Width::Px(w) => el.w(px(w)),
        };
        if let Some(color) = self.outline {
            el = el.border(px(1.5)).border_color(color);
        }
        if interactive {
            el = el.cursor_pointer().on_hover(move |over, _, cx| {
                hovered.update(cx, |h, cx| {
                    *h = *over;
                    cx.notify();
                });
            });
        }

        let press = |el: gpui::Stateful<gpui::Div>, button: MouseButton, f: Option<Handler>| match f
        {
            Some(f) => {
                let bounds = bounds.clone();
                el.on_mouse_down(button, move |_, window, cx| {
                    cx.stop_propagation();
                    f(bounds.get(), window, cx);
                })
            }
            None => el,
        };
        el = press(el, MouseButton::Left, self.on_press);
        el = press(el, MouseButton::Right, self.on_right);
        el = press(el, MouseButton::Middle, self.on_middle);
        if let Some(f) = self.on_click {
            let bounds = bounds.clone();
            el = el.on_click(move |_, window, cx| {
                cx.stop_propagation();
                f(bounds.get(), window, cx);
            });
        }
        if let Some(f) = self.on_scroll {
            el = el.on_scroll(move |event, window, cx| {
                let y = match event.delta {
                    ScrollDelta::Pixels(p) => f32::from(p.y),
                    ScrollDelta::Lines(l) => l.y,
                };
                if y != 0. {
                    cx.stop_propagation();
                    f(y > 0., window, cx);
                }
            });
        }

        if let Some(count) = self.badge {
            el = el.child(
                div()
                    .absolute()
                    .top(px(1.))
                    .right(px(1.))
                    .h(px(14.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .rounded(px(7.))
                    .bg(theme::WHITE)
                    .text_color(theme::INK)
                    .text_size(px(10.))
                    .font_weight(theme::SEMIBOLD)
                    .child(count),
            );
        }
        el
    }
}

/// `from` → `to` by `t`, in straight (non-premultiplied) colour. A transparent end takes the
/// other end's colour, so fading in doesn't pass through black.
fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let from = if from.a == 0. {
        Rgba { a: 0., ..to }
    } else {
        from
    };
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Rgba {
        r: lerp(from.r, to.r),
        g: lerp(from.g, to.g),
        b: lerp(from.b, to.b),
        a: lerp(from.a, to.a),
    }
}
