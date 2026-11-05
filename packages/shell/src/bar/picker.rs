//! The colour picker's slot (PickerSlot.qml): a swatch of the last colour. Left click opens the
//! picker panel, right click picks. Hidden until the picker is first used (super+shift+c), and
//! again after "Remove from bar".

use gpui::{Context, div, prelude::*, px};

use super::Bar;
use crate::panel::PanelKind;
use crate::services::picker;
use crate::theme;
use crate::ui::{Width, bar_button};

/// The slot, if the bar shows it.
pub fn slot(bar: &Bar, cx: &mut Context<Bar>) -> Option<impl IntoElement + use<>> {
    let model = bar.services.picker.clone();
    let state = model.read(cx);
    if !state.slot_shown {
        return None;
    }
    // The colour in a white ring (one bordered circle, so the ring stays even at fractional
    // scales). Nudged up half a pixel to sit on the line of the bar's icon glyphs.
    let swatch = div()
        .size(px(18.))
        .mt(px(-1.))
        .rounded_full()
        .bg(picker::color(state.hex()))
        .border(px(1.5))
        .border_color(theme::alpha(theme::WHITE, 0.85));
    let open = cx.entity();
    Some(
        bar_button("picker", swatch)
            .width(Width::Px(31.))
            .padding(0.)
            .centered()
            .active(bar.is_open(&PanelKind::Picker))
            .on_press(move |_, window, cx| {
                open.update(cx, |bar, cx| {
                    bar.toggle_panel(PanelKind::Picker, window, cx)
                });
            })
            .on_right(move |_, _, cx| model.update(cx, |p, cx| p.pick(cx))),
    )
}
