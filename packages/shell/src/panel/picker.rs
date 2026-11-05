//! The colour picker's panel (PickerPanel.qml): the current colour in four copyable formats, the
//! history, and a way to take the slot back out of the bar.

use gpui::{Context, Entity, SharedString, Window, div, prelude::*, px};

use super::frame;
use crate::services::Services;
use crate::services::picker::{self, Picker};
use crate::theme;
use crate::ui::controls::{pill, section, tile};
use crate::ui::{bar_button, columns, hstack, icon, key_cap, label, small, vstack};

pub const WIDTH: f32 = 340.;
const SWATCH_GAP: f32 = 6.;
/// The history grid's inset from the card's sides.
const GRID_INSET: f32 = 6.;
const TINY: f32 = 11.;
/// Eight to a row, however many there are.
const SWATCH: f32 =
    ((WIDTH - theme::PANEL_PADDING * 2. - GRID_INSET * 2. - SWATCH_GAP * 7.) / 8.) as u32 as f32;

pub struct PickerPanel {
    picker: Entity<Picker>,
    /// The format row just copied, and the colour it was copied for: a new colour clears it.
    copied: Option<(&'static str, String)>,
    _observer: gpui::Subscription,
}

impl PickerPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let picker = Services::get(cx).picker;
        let observer = cx.observe(&picker, crate::ui::redraw);
        Self {
            picker,
            copied: None,
            _observer: observer,
        }
    }
}

impl Render for PickerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.picker.read(cx);
        let hex = state.hex().to_string();
        let (history, selected) = (state.history.clone(), state.current);
        let copied = self
            .copied
            .as_ref()
            .filter(|(_, for_hex)| *for_hex == hex)
            .map(|(name, _)| *name);

        let status = match &state.error {
            Some(e) => small(e.clone(), theme::WARN_TEXT),
            None => small(
                if state.history.is_empty() {
                    "Nothing picked yet"
                } else if state.fresh {
                    "Picked just now · copied as hex"
                } else {
                    "From history"
                },
                theme::alpha(theme::TEXT, 0.62),
            ),
        };
        let current = hstack(14.)
            .child(
                div()
                    .flex_none()
                    .size(px(56.))
                    .rounded(px(14.))
                    .bg(picker::color(&hex))
                    .border_1()
                    .border_color(theme::alpha(theme::WHITE, 0.18)),
            )
            .child(
                vstack(2.)
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_family(theme::MONO)
                            .text_size(px(20.))
                            .child(hex.to_uppercase()),
                    )
                    .child(status),
            );

        let rows: Vec<_> = picker::formats(&hex)
            .into_iter()
            .map(|(name, value)| self.format_row(name, value, copied == Some(name), &hex, cx))
            .collect();
        let card = tile().child(current).child(vstack(2.).children(rows));

        let mut body = vstack(10.).child(card);
        if !history.is_empty() {
            let clear = self.picker.clone();
            body = body.child(
                section("History").child(
                    bar_button("clear", small("Clear", theme::alpha(theme::TEXT, 0.72)))
                        .height(20.)
                        .radius(6.)
                        .padding(6.)
                        .on_click(move |_, cx| clear.update(cx, |p, cx| p.clear(cx))),
                ),
            );
            let swatches = history.iter().enumerate().map(|(i, c)| {
                let select = self.picker.clone();
                swatch(i, c, i == selected)
                    .on_click(move |_, _, cx| select.update(cx, |p, cx| p.select(i, cx)))
                    .into_any_element()
            });
            body = body.child(
                div()
                    .px(px(GRID_INSET))
                    .child(columns(8, SWATCH_GAP, swatches)),
            );
        }

        let pick = self.picker.clone();
        let remove = self.picker.clone();
        let keys =
            ["super", "⇧", "C"].map(|k| key_cap(k, theme::TEXT, theme::alpha(theme::WHITE, 0.1)));
        let hint = vstack(4.)
            .flex_1()
            .child(
                label(
                    "Right-click the swatch to pick",
                    theme::alpha(theme::TEXT, 0.6),
                )
                .text_size(px(TINY)),
            )
            .child(hstack(4.).children(keys));
        frame("Colour picker")
            .icon("dropper")
            .action(
                bar_button(
                    "pick",
                    hstack(6.)
                        .child(icon("dropper", 13., theme::TEXT))
                        .child(small("Pick", theme::TEXT)),
                )
                .height(28.)
                .radius(14.)
                .padding(12.)
                .rest(theme::FILL)
                .on_click(move |_, cx| pick.update(cx, |p, cx| p.pick(cx))),
            )
            .footer(
                hstack(8.)
                    .flex_1()
                    .pl(px(10.))
                    .pr(px(4.))
                    .child(hint)
                    .child(
                        pill("remove", "Remove from bar").on_click(move |window, cx| {
                            window.remove_window();
                            remove.update(cx, |p, cx| p.remove(cx));
                        }),
                    ),
            )
            .body(body)
    }
}

impl PickerPanel {
    /// A format's row: click to copy its value.
    fn format_row(
        &self,
        name: &'static str,
        value: String,
        copied: bool,
        hex: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let dim = |a| theme::alpha(theme::TEXT, a);
        let hex = hex.to_string();
        let text = value.clone();
        hstack(12.)
            .id(SharedString::from(format!("format-{name}")))
            .group("format")
            .h(px(34.))
            .px(px(10.))
            .rounded(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(theme::alpha(theme::WHITE, 0.09)))
            .on_click(cx.listener(move |this, _, _, cx| {
                picker::copy(text.clone());
                this.copied = Some((name, hex.clone()));
                cx.notify();
            }))
            .child(
                label(name, dim(0.55))
                    .flex_none()
                    .w(px(44.))
                    .text_size(px(TINY))
                    .font_weight(theme::SEMIBOLD),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(theme::MONO)
                    .child(value),
            )
            .when(copied, |row| {
                row.child(label("Copied", dim(0.6)).text_size(px(TINY)))
            })
            .child(
                icon(
                    if copied { "check" } else { "copy" },
                    13.,
                    dim(if copied { 0.8 } else { 0.45 }),
                )
                .group_hover("format", |s| s.text_color(dim(0.8))),
            )
    }
}

/// A colour in the history; the selected one has a dark gap, then a light ring.
fn swatch(index: usize, hex: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    let ring = div()
        .absolute()
        .top(px(-4.))
        .left(px(-4.))
        .right(px(-4.))
        .bottom(px(-4.))
        .rounded(px(13.))
        .border_2()
        .border_color(theme::WHITE);
    div()
        .id(("swatch", index))
        .relative()
        .size(px(SWATCH))
        .cursor_pointer()
        .when(on, |d| d.child(ring))
        .child(
            div()
                .size_full()
                .rounded(px(9.))
                .bg(picker::color(hex))
                .when(!on, |d| {
                    d.border_1().border_color(theme::alpha(theme::WHITE, 0.18))
                }),
        )
}
