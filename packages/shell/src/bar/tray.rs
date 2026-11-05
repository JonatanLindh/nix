//! Tray icons (Tray.qml): a click activates the item (or shows its menu, for items that are only
//! a menu), a right click shows its menu under the icon, a middle click is the secondary action.

use gpui::{AnyElement, Bounds, Context, Pixels, Window, img, prelude::*, px, svg};

use super::Bar;
use crate::panel::PanelKind;
use crate::services::tray::{self, Icon, Item, Status};
use crate::ui::{bar_button, hstack, icon};
use crate::{runtime, theme};

const SIZE: f32 = 15.;

pub fn view(
    items: &[Item],
    menu_open: Option<&str>,
    cx: &mut Context<Bar>,
) -> impl IntoElement + use<> {
    let bar = cx.entity();
    hstack(2.).children(items.iter().map(|item| {
        let opacity = if item.status == Status::Passive {
            0.5
        } else {
            0.85
        };
        let attention = item.status == Status::NeedsAttention;
        let glyph: AnyElement = match &item.icon {
            // Theme icons are drawn in the bar's colour, unless the item wants attention.
            Some(Icon::Svg(path)) if attention => img(path.clone())
                .size(px(SIZE))
                .opacity(opacity)
                .into_any_element(),
            Some(Icon::Svg(path)) => svg()
                .external_path(path.to_string_lossy().into_owned())
                .size(px(SIZE))
                .text_color(theme::alpha(theme::TEXT, opacity))
                .into_any_element(),
            Some(Icon::Raster { grey, color }) => img(if attention {
                color.clone()
            } else {
                grey.clone()
            })
            .size(px(SIZE))
            .opacity(opacity)
            .into_any_element(),
            // Something to click while its icon is missing.
            None => icon("more", SIZE, theme::alpha(theme::TEXT, opacity)).into_any_element(),
        };

        let address = item.address.clone();
        let menu = {
            let (bar, address) = (bar.clone(), address.clone());
            let path = item.menu.as_ref().map(|m| m.path.clone());
            move |bounds: Bounds<Pixels>, window: &mut Window, cx: &mut gpui::App| {
                if let Some(path) = path.clone() {
                    // Menus built lazily fill in when told they're about to show.
                    runtime::detach(tray::menu_opening(address.clone(), path));
                }
                let kind = PanelKind::Tray {
                    address: address.clone(),
                    x: f32::from(bounds.origin.x),
                };
                bar.update(cx, |bar, cx| bar.toggle_panel(kind, window, cx));
            }
        };
        let button = bar_button(gpui::SharedString::from(format!("tray-{address}")), glyph)
            .active(menu_open == Some(item.address.as_str()))
            .on_right(menu.clone())
            .on_middle({
                let address = address.clone();
                move |_, _| runtime::detach(tray::secondary(address.clone()))
            });
        if item.only_menu {
            button.on_press(menu)
        } else {
            button.on_click(move |_, _| runtime::detach(tray::activate(address.clone())))
        }
    }))
}
