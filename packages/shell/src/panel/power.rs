//! The power panel (PowerPanel.qml): the battery, screen brightness, the power profile, and
//! caffeine. Opened from the battery, or from caffeine's right click on a machine without one.

use gpui::{App, Context, Div, SharedString, Window, div, prelude::*, px};

use super::{Host, frame};
use crate::services::Services;
use crate::services::battery;
use crate::shell::Shell;
use crate::theme;
use crate::ui::controls::{circle_button, switch, tile};
use crate::ui::slider::split_slider;
use crate::ui::{Width, bar_button, hstack, icon, label, small, vstack};

pub struct PowerPanel {
    services: Services,
    host: Host,
    _observers: Vec<gpui::Subscription>,
}

impl PowerPanel {
    pub fn new(host: Host, cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        let observers = vec![
            cx.observe(&services.battery, crate::ui::redraw),
            cx.observe(&services.brightness, crate::ui::redraw),
            cx.observe(&services.power_profiles, crate::ui::redraw),
            cx.observe(&services.caffeine, crate::ui::redraw),
        ];
        Self {
            services,
            host,
            _observers: observers,
        }
    }
}

impl Render for PowerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let battery = self.services.battery.read(cx).value.clone();
        let mut frame = frame(if battery.is_some() {
            "Battery"
        } else {
            "Power"
        });
        if let Some(b) = &battery {
            let status = if b.charging {
                "Charging"
            } else if b.on_ac {
                "Plugged in"
            } else {
                "On battery"
            };
            frame = frame.action(small(status, theme::TEXT_DIM));
        }
        // The full session menu (super+m).
        let bar = self.host.bar.clone();
        frame = frame.action(circle_button("session", "power", theme::TEXT).on_click(
            move |window, cx: &mut App| {
                let monitor = bar.upgrade().and_then(|bar| bar.read(cx).monitor(cx));
                window.remove_window();
                Shell::global(cx).update(cx, |shell, cx| shell.toggle_session(monitor, cx));
            },
        ));

        let mut body = vstack(12.);
        if let Some(b) = &battery {
            body = body.child(battery_tile(b));
        }

        // Screen brightness (laptop).
        if let Some(fraction) = self
            .services
            .brightness
            .read(cx)
            .state
            .as_ref()
            .map(|s| s.fraction())
        {
            let brightness = self.services.brightness.clone();
            body = body.child(
                tile()
                    .gap(px(8.))
                    .child(small("Display", theme::TEXT_DIM))
                    .child(split_slider(
                        "brightness",
                        fraction,
                        "sun",
                        move |value, _, cx| brightness.update(cx, |b, cx| b.set(value, cx)),
                    )),
            );
        }

        let profiles = self.services.power_profiles.clone();
        let (current, available) = {
            let p = profiles.read(cx);
            (p.profile(), p.state.as_ref().map(|s| s.profiles.clone()))
        };
        if let Some(available) = available {
            let mut modes = hstack(8.).w_full();
            for profile in available {
                let on = current == Some(profile);
                let ink = if on { theme::INK } else { theme::TEXT };
                let content = vstack(8.)
                    .items_center()
                    .child(icon(profile.icon(), 20., ink))
                    .child(small(profile.label(), ink));
                let mut mode = bar_button(SharedString::from(profile.name()), content)
                    .height(78.)
                    .width(Width::Full)
                    .radius(14.)
                    .centered()
                    .rest(if on { theme::ACCENT } else { theme::FILL });
                // The current one takes no clicks, so hovering doesn't grey out its white.
                if !on {
                    let profiles = profiles.clone();
                    mode =
                        mode.on_click(move |_, cx| profiles.update(cx, |p, cx| p.set(profile, cx)));
                }
                modes = modes.child(div().flex_1().child(mode));
            }
            body = body.child(
                vstack(8.)
                    .child(div().pl(px(4.)).child(small("Power mode", theme::TEXT_DIM)))
                    .child(modes),
            );
        }

        let caffeine = self.services.caffeine.clone();
        let on = caffeine.read(cx).on();
        body = body.child(
            tile().child(
                hstack(12.)
                    .child(badge("coffee", on))
                    .child(
                        vstack(1.)
                            .flex_1()
                            .min_w_0()
                            .child(label("Caffeine", theme::TEXT).font_weight(theme::SEMIBOLD))
                            .child(small("Keep the screen on, skip auto-lock", theme::TEXT_DIM)),
                    )
                    .child(switch("caffeine", on, move |_, cx| {
                        caffeine.update(cx, |c, cx| c.toggle(cx))
                    })),
            ),
        );

        frame.body(body)
    }
}

/// An icon in a circle, white when it's on.
fn badge(name: &str, on: bool) -> Div {
    div()
        .flex_none()
        .size(px(34.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(17.))
        .bg(if on { theme::ACCENT } else { theme::FILL_HOVER })
        .child(icon(name, 17., if on { theme::INK } else { theme::TEXT }))
}

/// The charge, large, with what's left and a big battery glyph.
fn battery_tile(b: &battery::State) -> Div {
    let left = b
        .time_left()
        .or_else(|| b.on_ac.then(|| "Fully charged".into()));
    let words = vstack(2.)
        .flex_1()
        .child(
            div()
                .text_size(px(34.))
                .font_weight(theme::SEMIBOLD)
                .text_color(if b.low() {
                    theme::WARN_TEXT
                } else {
                    theme::TEXT
                })
                .child(format!("{}%", b.percent)),
        )
        .children(left.map(|l| small(l, theme::TEXT_DIM)));
    tile()
        .p(px(16.))
        .child(hstack(16.).child(words).child(big_battery(b)))
}

/// A 74x34 battery: outline, a fill as long as the charge, the nub, and a bolt while charging.
fn big_battery(b: &battery::State) -> Div {
    let faint = theme::alpha(theme::TEXT, 0.5);
    let level = (56. * b.percent as f32 / 100.).round().max(6.);
    let at =
        |x: f32, y: f32, w: f32, h: f32| div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h));
    div()
        .relative()
        .flex_none()
        .w(px(74.))
        .h(px(34.))
        .child(
            at(0., 0., 66., 34.)
                .rounded(px(10.))
                .border(px(2.))
                .border_color(faint),
        )
        .child(at(5., 5., level, 24.).rounded(px(5.5)).bg(if b.low() {
            theme::WARN
        } else {
            theme::TEXT
        }))
        .child(at(69., 12., 3., 10.).rounded(px(1.5)).bg(faint))
        .when(b.charging, |d| {
            d.child(
                at(0., 0., 66., 34.)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(
                        "bolt",
                        18.,
                        if b.percent > 55 {
                            theme::INK
                        } else {
                            theme::TEXT
                        },
                    )),
            )
        })
}
