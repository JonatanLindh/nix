//! The privacy indicators (PrivacyIndicators.qml): one pill in the bar while something records
//! the camera, microphone or screen, with an icon for each. Each in its own colour, so they
//! stand out from the settings the user turned on (which use the accent) and from each other.

use gpui::{div, prelude::*, px};

use crate::services::privacy::State;
use crate::theme;
use crate::ui::{hstack, icon};

pub fn view(state: &State) -> Option<impl IntoElement + use<>> {
    if !state.any() {
        return None;
    }
    let icons = [
        (state.camera, "camera", 17., theme::CAMERA),
        (state.mic, "mic", 14., theme::MIC),
        (state.screen, "monitor", 15., theme::SCREEN),
    ];
    let shown = icons.iter().filter(|(on, ..)| *on).count();
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(22.))
            // A lone icon gets a little more room either side.
            .px(px(if shown > 1 { 8. } else { 9. }))
            .rounded(px(11.))
            .bg(theme::alpha(theme::WHITE, 0.12))
            .child(
                hstack(7.).children(
                    icons
                        .into_iter()
                        .filter(|(on, ..)| *on)
                        .map(|(_, name, size, color)| icon(name, size, color)),
                ),
            ),
    )
}
