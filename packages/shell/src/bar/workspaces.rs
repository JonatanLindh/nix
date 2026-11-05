//! Workspace pills (Workspaces.qml): the one shown on this monitor is a filled pill, the rest are
//! numbers, bright when they hold windows and faint when empty.

use gpui::{Div, SharedString, prelude::*, px};

use crate::services::hyprland::{self, State, Workspace};
use crate::ui::{Width, bar_button, hstack, small};
use crate::{runtime, theme};

pub fn view(hypr: &State, monitor: &str) -> Div {
    let Some(shown) = hypr
        .monitors
        .iter()
        .find(|m| m.name == monitor)
        .map(|m| m.active_workspace)
    else {
        return hstack(4.);
    };
    let here: Vec<&Workspace> = hypr
        .workspaces
        .iter()
        .filter(|w| w.id > 0 && w.monitor == monitor)
        .collect();

    hstack(4.)
        .px(px(4.))
        .children(slots(hypr, monitor, shown, &here).into_iter().map(|id| {
            let ws = here.iter().find(|w| w.id == id).copied();
            pill(id, ws, id == shown, hypr.urgent.contains(&id))
        }))
}

/// The block of five around the shown workspace (1–5, 6–10: the desktop pins one block per
/// output), empty ones included so there's somewhere to click, plus any other workspace on this
/// monitor. Empty slots that live on another monitor are left out.
fn slots(hypr: &State, monitor: &str, shown: i32, here: &[&Workspace]) -> Vec<i32> {
    let current = if shown > 0 {
        shown
    } else {
        here.first().map_or(1, |w| w.id)
    };
    let first = (current - 1) / 5 * 5 + 1;
    let elsewhere = |id: i32| {
        hypr.workspaces
            .iter()
            .any(|w| w.id == id && w.monitor != monitor)
    };
    let mut ids: Vec<i32> = (first..first + 5).filter(|&id| !elsewhere(id)).collect();
    ids.extend(
        here.iter()
            .map(|w| w.id)
            .filter(|id| !(first..first + 5).contains(id)),
    );
    ids.sort_unstable();
    ids
}

fn pill(id: i32, ws: Option<&Workspace>, shown: bool, urgent: bool) -> impl IntoElement {
    let name: SharedString = ws.map_or_else(|| id.to_string(), |w| w.name.clone()).into();
    let occupied = ws.is_some_and(|w| w.windows > 0);
    let color = if shown {
        theme::TEXT
    } else if urgent {
        theme::WARN
    } else {
        theme::alpha(theme::TEXT, if occupied { 0.92 } else { 0.42 })
    };
    let button = bar_button(("workspace", id as usize), small(name, color))
        .height(20.)
        .radius(10.)
        .padding(6.)
        .centered()
        .active(shown)
        .on_click(move |_, _| runtime::detach(hyprland::focus_workspace(id)));
    // The shown one is a wider pill; the others fit their number, at least round.
    if shown {
        button.width(Width::Px(34.))
    } else {
        button
    }
}
