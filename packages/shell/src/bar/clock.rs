//! "Mon 19 Oct  14:21" in the middle of the bar (Clock.qml); a button for the calendar.

use chrono::{DateTime, Local};
use gpui::{Context, prelude::*};

use super::Bar;
use crate::panel::PanelKind;
use crate::theme;
use crate::ui::{bar_button, hstack, label};

pub fn view(now: &DateTime<Local>, open: bool, cx: &mut Context<Bar>) -> impl IntoElement + use<> {
    let bar = cx.entity();
    let date = label(
        now.format("%a %-d %b").to_string(),
        theme::alpha(theme::TEXT, 0.72),
    );
    let time = label(now.format("%H:%M").to_string(), theme::TEXT).font_weight(theme::SEMIBOLD);
    bar_button("clock", hstack(10.).child(date).child(time))
        .padding(12.)
        .active(open)
        .on_press(move |_, window, cx| {
            bar.update(cx, |bar, cx| {
                bar.toggle_panel(PanelKind::Calendar, window, cx)
            });
        })
}
