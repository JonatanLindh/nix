//! A tray item's menu (its DBusMenu), as a glass card under its icon. Submenus are shown inline,
//! indented under their label.

use gpui::{Context, Div, SharedString, Window, div, prelude::*, px};

use crate::services::Services;
use crate::services::tray::{self, Entry};
use crate::ui::{Width, bar_button, divider, glass, hstack, icon, label, small, vstack};
use crate::{runtime, theme};

pub const WIDTH: f32 = 260.;
pub const RADIUS: f32 = 14.;

pub struct TrayMenu {
    address: String,
    services: Services,
    _observer: gpui::Subscription,
}

impl TrayMenu {
    pub fn new(address: String, cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        let observer = cx.observe(&services.tray, |_, _, cx| cx.notify());
        Self {
            address,
            services,
            _observer: observer,
        }
    }
}

impl Render for TrayMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = &self.services.tray.read(cx).value;
        let mut list = vstack(1.);
        match items
            .iter()
            .find(|i| i.address == self.address)
            .and_then(|i| i.menu.as_ref())
        {
            Some(menu) if !menu.entries.is_empty() => {
                list = entries(list, &menu.entries, &self.address, &menu.path, 0);
            }
            _ => {
                list = list.child(
                    div()
                        .px(px(10.))
                        .py(px(6.))
                        .child(small("No menu", theme::TEXT_DIM)),
                )
            }
        }
        glass(RADIUS).w_full().p(px(6.)).child(list)
    }
}

fn entries(mut list: Div, items: &[Entry], address: &str, path: &str, depth: u16) -> Div {
    for entry in items {
        if entry.separator {
            list = list.child(div().px(px(8.)).py(px(4.)).child(divider()));
            continue;
        }
        let indent = f32::from(depth) * 14.;
        if !entry.children.is_empty() {
            // A submenu: its label as a heading, its entries under it.
            list = list.child(
                hstack(0.)
                    .py(px(6.))
                    .pr(px(10.))
                    .pl(px(10. + indent))
                    .child(small(entry.label.clone(), theme::TEXT_DIM).flex_1())
                    .child(icon("chevron-down", 12., theme::TEXT_FAINT)),
            );
            list = entries(list, &entry.children, address, path, depth + 1);
            continue;
        }
        list = list.child(entry_row(entry, address, path, indent));
    }
    list
}

fn entry_row(entry: &Entry, address: &str, path: &str, indent: f32) -> impl IntoElement {
    let ink = if entry.enabled {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    let mark = match entry.checked {
        Some(true) => Some(icon("check", 14., ink).into_any_element()),
        Some(false) => Some(div().w(px(14.)).into_any_element()),
        None => None,
    };
    let content = hstack(8.)
        .children(mark)
        .child(label(entry.label.clone(), ink));
    let mut button = bar_button(SharedString::from(format!("entry-{}", entry.id)), content)
        .height(28.)
        .width(Width::Full)
        .radius(8.)
        .padding(10. + indent);
    if entry.enabled {
        let (address, path, id) = (address.to_string(), path.to_string(), entry.id);
        button = button.on_click(move |window, _| {
            runtime::detach(tray::activate_entry(address.clone(), path.clone(), id));
            window.remove_window();
        });
    }
    button
}
