//! The Bluetooth panel (BtPanel.qml): connected devices with their battery, the paired ones to
//! connect to, and a scan for new ones nearby to pair.

use std::collections::HashSet;

use bluer::Address;
use gpui::{AnyElement, Context, Div, SharedString, Window, div, prelude::*, px};

use super::frame;
use crate::services::Services;
use crate::services::bluetooth::{self, Device, Scan};
use crate::ui::controls::{footer_link, pill, section, switch};
use crate::ui::{Width, bar_button, hstack, icon, label, small, vstack};
use crate::{runtime, theme};

/// The dim note at the end of a row ("Pair", "Connecting…").
const NOTE: gpui::Rgba = theme::alpha(theme::TEXT, 0.62);

pub struct BluetoothPanel {
    services: Services,
    /// Our scan, while one runs; it stops when the panel closes.
    scan: Option<Scan>,
    connecting: HashSet<Address>,
    pairing: HashSet<Address>,
    _observer: gpui::Subscription,
}

impl BluetoothPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        Self {
            _observer: cx.observe(&services.bluetooth, crate::ui::redraw),
            services,
            scan: None,
            connecting: HashSet::new(),
            pairing: HashSet::new(),
        }
    }

    fn toggle_scan(&mut self, scanning: bool, cx: &mut Context<Self>) {
        self.scan = if scanning {
            None
        } else {
            Some(bluetooth::scan())
        };
        cx.notify();
    }

    fn connect(&mut self, address: Address, cx: &mut Context<Self>) {
        if self.connecting.insert(address) {
            cx.notify();
            runtime::then(cx, bluetooth::connect(address), move |this, _, cx| {
                this.connecting.remove(&address);
                cx.notify();
            });
        }
    }

    fn pair(&mut self, address: Address, cx: &mut Context<Self>) {
        if self.pairing.insert(address) {
            cx.notify();
            runtime::then(cx, bluetooth::pair(address), move |this, _, cx| {
                this.pairing.remove(&address);
                cx.notify();
            });
        }
    }
}

impl Render for BluetoothPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.services.bluetooth.read(cx).value.clone();
        let this = cx.entity();
        let mut frame = frame("Bluetooth");
        if let Some(state) = &state {
            let on = state.powered;
            frame = frame.action(switch("bluetooth", on, move |_, _| {
                runtime::detach(bluetooth::set_powered(!on))
            }));
        }
        let footer =
            footer_link("bluetooth-settings", "Bluetooth settings…").on_click(|window, _| {
                runtime::detach(crate::services::spawn("blueman-manager", &[]));
                window.remove_window();
            });
        let frame = frame.footer(footer);

        let Some(state) = state.filter(|s| s.powered) else {
            let why = if self.services.bluetooth.read(cx).value.is_some() {
                "Bluetooth is off"
            } else {
                "No Bluetooth adapter"
            };
            return frame.body(
                div()
                    .flex()
                    .justify_center()
                    .py(px(18.))
                    .child(label(why, theme::TEXT_DIM)),
            );
        };

        let mut body = vstack(10.);
        let connected: Vec<_> = state.devices.iter().filter(|d| d.connected).collect();
        if !connected.is_empty() {
            body = body.child(vstack(8.).children(connected.into_iter().map(connected_tile)));
        }

        let paired: Vec<_> = state
            .devices
            .iter()
            .filter(|d| !d.connected && d.paired)
            .collect();
        if !paired.is_empty() {
            let mut list = vstack(2.).mt(px(-6.));
            for device in paired {
                let address = device.address;
                let note = self
                    .connecting
                    .contains(&address)
                    .then(|| small("Connecting…", NOTE).into_any_element());
                let this = this.clone();
                list =
                    list.child(device_row(device, note, false).on_click(move |_, cx| {
                        this.update(cx, |this, cx| this.connect(address, cx))
                    }));
            }
            body = body.child(section("Paired")).child(list);
        }

        let scanning = state.discovering;
        let scan = this.clone();
        body =
            body.child(
                section("Nearby").child(scan_button(scanning).on_click(move |_, cx| {
                    scan.update(cx, |this, cx| this.toggle_scan(scanning, cx))
                })),
            );
        if scanning {
            let mut list = vstack(2.).mt(px(-4.));
            for device in state.devices.iter().filter(|d| !d.connected && !d.paired) {
                let address = device.address;
                let pairing = self.pairing.contains(&address);
                let note = small(if pairing { "Pairing…" } else { "Pair" }, NOTE);
                let this = this.clone();
                list = list.child(
                    device_row(device, Some(note.into_any_element()), pairing)
                        .on_click(move |_, cx| this.update(cx, |this, cx| this.pair(address, cx))),
                );
            }
            body = body.child(list);
        }

        frame.body(body)
    }
}

/// An icon in a circle: white with dark ink while connected.
fn device_badge(name: &str, size: f32, on: bool) -> Div {
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(size / 2.))
        .bg(if on { theme::ACCENT } else { theme::FILL_HOVER })
        .child(icon(name, 15., if on { theme::INK } else { theme::TEXT }))
}

/// A connected device: its name, battery and a way to disconnect.
fn connected_tile(device: &Device) -> Div {
    let mut status = hstack(6.);
    if let Some(percent) = device.battery {
        status = status.child(battery_glyph(percent));
    }
    let detail = match device.battery {
        Some(percent) => format!("{percent}% · Connected"),
        None => "Connected".into(),
    };
    status = status.child(small(detail, theme::TEXT_DIM));
    let address = device.address;
    hstack(12.)
        .w_full()
        .h(px(54.))
        .px(px(12.))
        .rounded(px(16.))
        .bg(theme::FILL)
        .child(device_badge(device.icon, 34., true))
        .child(
            vstack(2.)
                .flex_1()
                .min_w_0()
                .child(
                    label(device.name.clone(), theme::TEXT)
                        .font_weight(theme::SEMIBOLD)
                        .truncate(),
                )
                .child(status),
        )
        .child(
            pill(
                SharedString::from(format!("disconnect-{address}")),
                "Disconnect",
            )
            .on_click(move |_, _| runtime::detach(bluetooth::disconnect(address))),
        )
}

/// A device in a list (ListRow.qml with its icon in a circle), with a note at the end.
fn device_row(device: &Device, note: Option<AnyElement>, dimmed: bool) -> crate::ui::BarButton {
    let content = hstack(12.)
        .w_full()
        .when(dimmed, |row| row.opacity(0.45))
        .child(device_badge(device.icon, 28., false))
        .child(label(device.name.clone(), theme::TEXT).flex_1().truncate())
        .children(note);
    bar_button(
        SharedString::from(format!("bt-{}", device.address)),
        content,
    )
    .height(40.)
    .width(Width::Full)
    .radius(10.)
    .padding(10.)
}

/// The scan toggle; a dot marks it while scanning.
fn scan_button(scanning: bool) -> crate::ui::BarButton {
    let dot = div()
        .size(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .bg(theme::alpha(theme::ACCENT, 0.3))
        .child(div().size(px(8.)).rounded(px(4.)).bg(theme::ACCENT));
    let content = hstack(6.).when(scanning, |row| row.child(dot)).child(small(
        if scanning { "Scanning" } else { "Scan" },
        theme::TEXT,
    ));
    bar_button("scan", content)
        .height(24.)
        .radius(12.)
        .padding(10.)
        .rest(theme::FILL)
}

/// A small battery, filled to the charge.
fn battery_glyph(percent: u8) -> Div {
    let level = (13. * percent as f32 / 100.).round().max(2.);
    div()
        .relative()
        .flex_none()
        .w(px(20.))
        .h(px(10.))
        .child(
            div()
                .absolute()
                .w(px(17.))
                .h(px(10.))
                .rounded(px(3.))
                .border(px(1.))
                .border_color(theme::TEXT_DIM),
        )
        .child(
            div()
                .absolute()
                .left(px(2.))
                .top(px(2.))
                .w(px(level))
                .h(px(6.))
                .rounded(px(1.5))
                .bg(theme::TEXT_DIM),
        )
        .child(
            div()
                .absolute()
                .left(px(18.))
                .top(px(3.5))
                .w(px(2.))
                .h(px(3.))
                .rounded(px(1.))
                .bg(theme::TEXT_DIM),
        )
}
