//! Reusable pieces shared by the bar, its panels and the other surfaces.

mod button;
pub mod controls;
pub mod icon;
pub mod slider;

use gpui::{Div, Rgba, SharedString, div, prelude::*, px};

pub use button::{BarButton, Width, bar_button};
pub use icon::icon;

use crate::theme;

/// Body text in `color`, at the theme's size.
pub fn label(text: impl Into<SharedString>, color: Rgba) -> Div {
    div().text_color(color).child(text.into())
}

/// Smaller dim text, for details under a title.
pub fn small(text: impl Into<SharedString>, color: Rgba) -> Div {
    label(text, color).text_size(px(theme::SMALL))
}

/// A horizontal row with its children centred vertically and `gap` between them.
pub fn hstack(gap: f32) -> Div {
    div().flex().flex_row().items_center().gap(px(gap))
}

/// A column with `gap` between its children.
pub fn vstack(gap: f32) -> Div {
    div().flex().flex_col().gap(px(gap))
}

/// The thin vertical divider between groups in the bar.
pub fn separator() -> Div {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(14.))
        .bg(theme::BAR_DIVIDER)
}

/// A hairline across a card.
pub fn divider() -> Div {
    div().w_full().h(px(1.)).bg(theme::DIVIDER)
}

/// A key cap for hints and shortcuts ("esc", "↵", "l"), in `ink` on `fill`.
pub fn key_cap(key: impl Into<SharedString>, ink: Rgba, fill: Rgba) -> Div {
    div()
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(5.))
        .rounded(px(5.))
        .bg(fill)
        .font_family(theme::MONO)
        .text_size(px(11.))
        .text_color(ink)
        .child(key.into())
}

/// "← → ↵ Pick" (KeyHint.qml): key caps, then what they do.
pub fn key_hint(keys: &[&'static str], what: &'static str) -> Div {
    hstack(6.)
        .child(
            hstack(4.).children(
                keys.iter()
                    .map(|k| key_cap(*k, theme::TEXT, theme::alpha(theme::WHITE, 0.1))),
            ),
        )
        .child(small(what, theme::TEXT_DIM))
}

/// A glass surface: translucent fill (blurred by Hyprland behind it) and a hairline border.
pub fn glass(radius: f32) -> Div {
    div()
        .bg(theme::GLASS)
        .border_1()
        .border_color(theme::GLASS_BORDER)
        .rounded(px(radius))
}

/// The root of every window: the theme's font and text colour.
pub fn root() -> Div {
    div()
        .size_full()
        .font_family(theme::FONT)
        .font_weight(theme::WEIGHT)
        .text_size(px(theme::FONT_SIZE))
        .text_color(theme::TEXT)
}

/// A click handler that runs `f` on the entity behind `cx`: `cx.listener` for [`BarButton`]'s
/// handlers, which take no event.
pub fn update<E: 'static>(
    cx: &gpui::Context<E>,
    f: impl Fn(&mut E, &mut gpui::Context<E>) + 'static,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) + 'static {
    let entity = cx.entity();
    move |_, cx| entity.update(cx, |this, cx| f(this, cx))
}

/// An observer that redraws the observing view when the observed entity changes.
pub fn redraw<V: 'static, E>(_: &mut V, _: gpui::Entity<E>, cx: &mut gpui::Context<V>) {
    cx.notify();
}

/// `items` in rows of `columns` equal cells, `gap` apart both ways; a short last row keeps the
/// cell width. (gpui's CSS grid doesn't stretch its items to the columns here.)
pub fn columns(columns: usize, gap: f32, items: impl IntoIterator<Item = gpui::AnyElement>) -> Div {
    let mut items: Vec<_> = items.into_iter().collect();
    let mut rows = vstack(gap).w_full();
    while !items.is_empty() {
        let rest = items.split_off(columns.min(items.len()));
        let cells = std::mem::replace(&mut items, rest);
        let filled = cells.len();
        rows = rows.child(
            hstack(gap)
                .w_full()
                .children(cells.into_iter().map(|c| div().flex_1().min_w_0().child(c)))
                .children((filled..columns).map(|_| div().flex_1())),
        );
    }
    rows
}
