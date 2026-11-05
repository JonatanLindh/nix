//! Controls used inside panels: tiles, headings, round and pill buttons, switches, list rows,
//! chips and dropdowns. Their QML counterparts are named on each.

use gpui::{AnyElement, App, Div, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

use super::{BarButton, Width, bar_button, hstack, icon, label, small, vstack};
use crate::theme;

/// A rounded fill grouping related controls in a panel (Tile.qml).
pub fn tile() -> Div {
    vstack(10.)
        .w_full()
        .p(px(12.))
        .rounded(px(16.))
        .bg(theme::FILL)
}

/// A small dim heading over a group, with controls on the right (SectionLabel.qml).
pub fn section(title: impl Into<SharedString>) -> Div {
    hstack(8.)
        .pl(px(10.))
        .pr(px(4.))
        .pt(px(4.))
        .child(small(title, theme::TEXT_DIM).flex_1())
}

/// A round icon button (CircleButton.qml).
pub fn circle_button(id: impl Into<ElementId>, name: &str, color: Rgba) -> BarButton {
    bar_button(id, icon(name, 15., color))
        .height(28.)
        .radius(14.)
        .padding(6.)
}

/// A pill-shaped text button (PillButton.qml).
pub fn pill(id: impl Into<ElementId>, text: impl Into<SharedString>) -> BarButton {
    bar_button(id, small(text, theme::TEXT))
        .height(28.)
        .radius(14.)
        .padding(12.)
        .rest(theme::FILL)
}

/// An on/off switch (Switch.qml).
pub fn switch(
    id: impl Into<ElementId>,
    on: bool,
    toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> BarButton {
    let knob = div()
        .size(px(18.))
        .rounded(px(9.))
        .bg(if on { theme::INK } else { theme::WHITE });
    let track = div()
        .flex()
        .items_center()
        .w(px(40.))
        .h(px(24.))
        .p(px(3.))
        .rounded(px(12.))
        .bg(if on { theme::WHITE } else { theme::TRACK })
        .when(on, |d| d.justify_end())
        .child(knob);
    bar_button(id, track)
        .height(24.)
        .radius(12.)
        .padding(0.)
        .on_click(toggle)
}

/// A clickable row in a panel list (ListRow.qml): an icon, text, dim detail, and whatever goes
/// at the end.
pub fn list_row(
    id: impl Into<ElementId>,
    name: Option<&str>,
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
    trailing: Vec<AnyElement>,
) -> BarButton {
    let content = hstack(10.)
        .w_full()
        .when_some(name, |row, name| {
            row.child(icon(name, 17., theme::alpha(theme::TEXT, 0.85)))
        })
        .child(label(title, theme::TEXT))
        .when_some(detail, |row, detail| {
            row.child(small(detail, theme::TEXT_DIM))
        })
        .child(div().flex_1())
        .children(trailing);
    bar_button(id, content)
        .height(40.)
        .width(Width::Full)
        .radius(10.)
        .padding(10.)
}

/// A small rounded label, lit or not (the sound panel's signal chain).
pub fn chip(text: impl Into<SharedString>, lit: bool, dim: bool) -> Div {
    div()
        .flex()
        .items_center()
        .h(px(22.))
        .px(px(9.))
        .rounded(px(11.))
        .bg(if lit {
            theme::alpha(theme::ACCENT, 0.32)
        } else {
            theme::alpha(theme::WHITE, 0.1)
        })
        .text_size(px(11.))
        .text_color(if dim {
            theme::alpha(theme::TEXT, 0.62)
        } else {
            theme::TEXT
        })
        .child(text.into())
}

/// A dim text link for a panel's footer ("Sound settings…").
pub fn footer_link(id: impl Into<ElementId>, text: impl Into<SharedString>) -> BarButton {
    bar_button(id, label(text, theme::alpha(theme::TEXT, 0.8)))
        .height(32.)
        .radius(10.)
        .padding(10.)
}

/// One choice in a [`dropdown`].
pub struct Choice {
    pub label: SharedString,
    pub icon: Option<&'static str>,
}

/// A picker that unfolds in place (Dropdown.qml): the current choice with a chevron, and while
/// `open` every choice under it. Which dropdown is open is the panel's state; `on_toggle` flips
/// it and `on_pick` gets the index picked.
pub fn dropdown(
    id: impl Into<SharedString>,
    choices: Vec<Choice>,
    current: Option<usize>,
    open: bool,
    enabled: bool,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let id: SharedString = id.into();
    let ink = if enabled {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    let shown = current.and_then(|i| choices.get(i));
    let head = hstack(10.)
        .w_full()
        .when_some(shown.and_then(|c| c.icon), |row, name| {
            row.child(icon(name, 16., ink))
        })
        .child(label(shown.map_or("—".into(), |c| c.label.clone()), ink).flex_1())
        .child(icon(
            if open {
                "chevron-down"
            } else {
                "chevron-right"
            },
            13.,
            theme::TEXT_DIM,
        ));
    let mut header = bar_button(SharedString::from(format!("{id}-head")), head)
        .height(38.)
        .width(Width::Full)
        .radius(10.)
        .padding(12.)
        .rest(theme::FILL)
        .active(open);
    if enabled {
        header = header.on_click(on_toggle);
    }

    let mut list = vstack(2.).w_full().child(header);
    if open {
        let on_pick = std::rc::Rc::new(on_pick);
        for (i, choice) in choices.into_iter().enumerate() {
            let check = if Some(i) == current {
                icon("check", 14., theme::TEXT).into_any_element()
            } else {
                div().w(px(14.)).into_any_element()
            };
            let content = hstack(10.)
                .w_full()
                .when_some(choice.icon, |row, name| {
                    row.child(icon(name, 16., theme::alpha(theme::TEXT, 0.85)))
                })
                .child(label(choice.label, theme::TEXT).flex_1())
                .child(check);
            let on_pick = on_pick.clone();
            list = list.child(
                bar_button(SharedString::from(format!("{id}-{i}")), content)
                    .height(34.)
                    .width(Width::Full)
                    .radius(9.)
                    .padding(12.)
                    .on_click(move |window, cx| on_pick(i, window, cx)),
            );
        }
    }
    list
}

/// Text in the theme's monospace face, for numbers and addresses.
pub fn mono(text: impl Into<SharedString>) -> Div {
    div()
        .font_family(theme::MONO)
        .text_size(px(11.))
        .text_color(theme::alpha(theme::TEXT, 0.62))
        .child(text.into())
}
