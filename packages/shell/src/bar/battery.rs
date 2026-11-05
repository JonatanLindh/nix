//! The battery glyph (after Bar.qml): a 28x13 outline, a fill as long as the charge, the nub, and
//! a bolt over it all while charging.

use gpui::{Div, div, prelude::*, px};

use crate::services::battery::State;
use crate::theme;

pub fn glyph(battery: &State) -> Div {
    let fill = if battery.low() {
        theme::WARN
    } else if battery.charging {
        theme::ONLINE
    } else {
        theme::TEXT
    };
    let faint = theme::alpha(theme::TEXT, 0.5);
    let level = (19. * battery.percent as f32 / 100.).round().max(2.);
    let at =
        |x: f32, y: f32, w: f32, h: f32| div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h));

    div()
        .relative()
        .flex_none()
        .w(px(28.))
        .h(px(13.))
        // Whole-pixel offsets throughout: gpui snaps each edge to the device grid from the
        // window's origin, so fractional ones (Bar.qml's 0.75 and 2.5) round apart and pull
        // the fill off centre wherever the glyph lands on a fractional position.
        .child(
            at(0., 0., 23., 13.)
                .rounded(px(3.5))
                .border(px(1.))
                .border_color(faint),
        )
        .child(at(2., 2., level, 9.).rounded(px(2.)).bg(fill))
        .child(at(24., 4., 2., 5.).rounded(px(1.)).bg(faint))
        .when(battery.charging, |d| {
            d.child(
                div().absolute().inset_0().child(
                    gpui::svg()
                        .path(crate::ui::icon::path("battery-bolt"))
                        .w(px(28.))
                        .h(px(13.))
                        .text_color(theme::WHITE),
                ),
            )
        })
}
