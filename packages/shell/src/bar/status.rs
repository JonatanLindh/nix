//! The status icons on the right of the bar.

use gpui::{Context, div, prelude::*, px};

use super::{Bar, battery, privacy};
use crate::panel::PanelKind;
use crate::services::power_profiles::Profile;
use crate::theme;
use crate::ui::{bar_button, hstack, icon, label, separator};

pub fn view(bar: &Bar, monitor: Option<String>, cx: &mut Context<Bar>) -> impl IntoElement + use<> {
    let this = cx.entity();
    let services = bar.services.clone();
    let mut items = hstack(2.);

    // While a screen recording runs: red dot and elapsed time; click stops it.
    items = items.children(crate::screenshot::indicator(cx));

    // Only while something records the mic, camera or screen.
    if let Some(pill) = privacy::view(&services.privacy.read(cx).value) {
        items = items.child(pill).child(div().w(px(4.)));
    }

    // The power profile, only off Balanced; a click goes back to it.
    let profiles = services.power_profiles.clone();
    if let Some(profile) = profiles.read(cx).profile()
        && profile != Profile::Balanced
    {
        let open = this.clone();
        items = items.child(
            bar_button("power-profile", icon(profile.icon(), 16., theme::ACCENT))
                .on_click(move |_, cx| profiles.update(cx, |p, cx| p.set(Profile::Balanced, cx)))
                .on_right(move |_, window, cx| {
                    open.update(cx, |bar, cx| bar.toggle_panel(PanelKind::Power, window, cx));
                }),
        );
    }

    // Only while a funnel makes something reachable from the public internet; the rest of
    // Tailscale is in the network panel.
    if services.tailscale.read(cx).value.funnel_active() {
        let open = this.clone();
        items = items.child(
            bar_button("tailscale", icon("tailscale", 16., theme::ACCENT))
                .active(bar.is_open(&PanelKind::Tailscale))
                .on_press(move |_, window, cx| {
                    open.update(cx, |bar, cx| {
                        bar.toggle_panel(PanelKind::Tailscale, window, cx)
                    });
                }),
        );
    }

    let audio = services.audio.read(cx);
    if let Some(state) = &audio.state {
        let color = if state.muted() {
            theme::alpha(theme::TEXT, 0.5)
        } else {
            theme::TEXT
        };
        let output = state.output.as_ref().map(|d| d.id);
        let (open, mute, nudge) = (this.clone(), services.audio.clone(), services.audio.clone());
        items = items.child(
            bar_button("audio", icon(state.icon(), 18., color))
                .active(bar.is_open(&PanelKind::Audio))
                .on_press(move |_, window, cx| {
                    open.update(cx, |bar, cx| bar.toggle_panel(PanelKind::Audio, window, cx));
                })
                .on_right(move |_, _, cx| {
                    if let Some(id) = output {
                        mute.update(cx, |audio, cx| audio.toggle_mute(id, cx));
                    }
                })
                .on_scroll(move |up, _, cx| nudge.update(cx, |audio, cx| audio.nudge(up, cx))),
        );
    }

    if let Some(bt) = &services.bluetooth.read(cx).value {
        let open = this.clone();
        items = items.child(
            bar_button("bluetooth", icon("bluetooth", 17., bt.color()))
                .active(bar.is_open(&PanelKind::Bluetooth))
                .on_press(move |_, window, cx| {
                    open.update(cx, |bar, cx| {
                        bar.toggle_panel(PanelKind::Bluetooth, window, cx)
                    });
                }),
        );
    }

    if let Some(net) = &services.network.read(cx).value {
        let open = this.clone();
        items = items.child(
            bar_button("network", icon(net.icon(), 18., net.color()))
                .active(bar.is_open(&PanelKind::Network))
                .on_press(move |_, window, cx| {
                    open.update(cx, |bar, cx| {
                        bar.toggle_panel(PanelKind::Network, window, cx)
                    });
                }),
        );
    }

    let has_battery = services.battery.read(cx).value.is_some();
    if let Some(b) = &services.battery.read(cx).value {
        let open = this.clone();
        let color = if b.low() {
            theme::WARN_TEXT
        } else {
            theme::TEXT
        };
        items = items.child(
            bar_button(
                "battery",
                hstack(6.)
                    .child(battery::glyph(b))
                    .child(label(format!("{}%", b.percent), color)),
            )
            .active(bar.is_open(&PanelKind::Power))
            .on_press(move |_, window, cx| {
                open.update(cx, |bar, cx| bar.toggle_panel(PanelKind::Power, window, cx));
            }),
        );
    }

    items = items.child(div().px(px(4.)).child(separator()));

    // Keeps the screen awake. Right click opens the power panel, the only way to it on a machine
    // without a battery.
    let on = services.caffeine.read(cx).on();
    let caffeine = services.caffeine.clone();
    let open = this.clone();
    items = items.child(
        bar_button(
            "caffeine",
            icon("coffee", 17., if on { theme::ACCENT } else { theme::TEXT }),
        )
        .rest(if on {
            theme::alpha(theme::ACCENT, 0.22)
        } else {
            theme::TRANSPARENT
        })
        .active(!has_battery && bar.is_open(&PanelKind::Power))
        .on_click(move |_, cx| caffeine.update(cx, |c, cx| c.toggle(cx)))
        .on_right(move |_, window, cx| {
            open.update(cx, |bar, cx| bar.toggle_panel(PanelKind::Power, window, cx));
        }),
    );

    let surfaces = bar.surfaces.clone();
    let notifications = surfaces.notifications.clone();
    items = items.child(crate::notifications::indicator(
        &notifications,
        bar.is_open(&PanelKind::Notifications),
        this.clone(),
        cx,
    ));

    // The session menu: lock, suspend, log out, shut down, ...
    let session_open = surfaces.session.read(cx).is_open();
    let shell = surfaces.shell.clone();
    items.child(
        bar_button("power", icon("power", 18., theme::TEXT))
            .active(session_open)
            .on_click(move |_, cx| {
                let monitor = monitor.clone();
                shell.update(cx, |shell, cx| shell.toggle_session(monitor, cx));
            }),
    )
}
