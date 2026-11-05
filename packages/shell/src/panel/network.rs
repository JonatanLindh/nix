//! The network panel (NetPanel.qml): wired links, the Wi-Fi network in use, the others in range
//! (with a password field for a new secured one), and Tailscale: its switch, and the way into its
//! own panel.

use gpui::component::input::{Input, InputEvent, InputState};
use gpui::{Context, Div, Entity, SharedString, Window, div, prelude::*, px};

use super::{Host, PanelKind, frame};
use crate::services::network::{Network, Wired};
use crate::services::{Services, network, tailscale};
use crate::ui::controls::{
    circle_button, footer_link, list_row, mono, pill, section, switch, tile,
};
use crate::ui::{Width, bar_button, hstack, icon, label, small, vstack};
use crate::{runtime, theme};

pub struct NetworkPanel {
    services: Services,
    host: Host,
    /// The network asking for a password.
    asking: Option<String>,
    password: Entity<InputState>,
    connecting: Option<String>,
    failed: Option<String>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl NetworkPanel {
    pub fn new(host: Host, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Password")
        });
        let subscriptions = vec![
            cx.observe(&services.network, |_, _, cx| cx.notify()),
            cx.observe(&services.tailscale, |_, _, cx| cx.notify()),
            cx.subscribe(&password, |this, _, event: &InputEvent, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.submit(cx);
                }
            }),
        ];
        Self {
            services,
            host,
            asking: None,
            password,
            connecting: None,
            failed: None,
            _subscriptions: subscriptions,
        }
    }

    /// Shows the password field under `ssid`, or hides it if it's showing there.
    fn ask(&mut self, ssid: String, window: &mut Window, cx: &mut Context<Self>) {
        self.failed = None;
        self.password
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.asking = (self.asking.as_deref() != Some(&ssid)).then_some(ssid);
        if self.asking.is_some() {
            self.password
                .update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    /// Joins the network asking for a password, with what's typed.
    fn submit(&mut self, cx: &mut Context<Self>) {
        let password = self.password.read(cx).value().to_string();
        if let Some(ssid) = self.asking.clone()
            && !password.is_empty()
        {
            self.connect(ssid, Some(password), cx);
        }
    }

    /// Connects, with a password for a new secured network.
    fn connect(&mut self, ssid: String, password: Option<String>, cx: &mut Context<Self>) {
        self.asking = None;
        self.failed = None;
        self.connecting = Some(ssid.clone());
        cx.notify();
        runtime::then(
            cx,
            network::connect(ssid.clone(), password),
            move |this, result, cx| {
                if this.connecting.as_deref() == Some(&ssid) {
                    this.connecting = None;
                }
                if let Err(e) = result {
                    eprintln!("network: connecting to {ssid}: {e}");
                    this.failed = Some(ssid);
                }
                cx.notify();
            },
        );
    }
}

impl Render for NetworkPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let net = self
            .services
            .network
            .read(cx)
            .value
            .clone()
            .unwrap_or_default();
        let this = cx.entity();
        let mut frame = frame(if net.wifi.is_some() {
            "Wi-Fi"
        } else {
            "Network"
        });
        if let Some(wifi) = &net.wifi {
            let enabled = wifi.enabled;
            frame = frame.action(switch("wifi", enabled, move |_, _| {
                runtime::detach(network::set_wifi(!enabled))
            }));
        }

        let mut body = vstack(10.);
        // Wired links are shown when up, plugged in, or when there's no Wi-Fi to show instead.
        for link in net
            .wired
            .iter()
            .filter(|w| w.connected || w.has_link || net.wifi.is_none())
        {
            body = body.child(wired(link));
        }

        if let Some(wifi) = &net.wifi {
            if !wifi.enabled {
                body = body.child(
                    div()
                        .flex()
                        .justify_center()
                        .py(px(18.))
                        .child(label("Wi-Fi is off", theme::TEXT_DIM)),
                );
            } else {
                if let Some(current) = &wifi.current {
                    body = body.child(connected(current, wifi.ip.as_deref(), &wifi.interface));
                }
                let interface = wifi.interface.clone();
                body = body.child(
                    section(if wifi.current.is_some() {
                        "Other networks"
                    } else {
                        "Networks"
                    })
                    .child(
                        circle_button("rescan", "refresh", theme::TEXT_DIM).on_click(
                            move |_, _| runtime::detach(network::rescan(interface.clone())),
                        ),
                    ),
                );
                let mut list = vstack(2.);
                for network in &wifi.others {
                    list = list.child(self.other(network, &this));
                }
                if wifi.others.is_empty() {
                    list = list.child(
                        div()
                            .px(px(10.))
                            .py(px(6.))
                            .child(small("Looking for networks…", theme::TEXT_DIM)),
                    );
                }
                body = body.child(list);
            }
        }

        let ts = self.services.tailscale.read(cx).value.clone();
        if ts.available {
            body = body.child(self.tailscale(&ts));
        }

        frame
            .footer(
                footer_link("network-settings", "Network settings…").on_click(|window, _| {
                    runtime::detach(crate::services::spawn("nm-connection-editor", &[]));
                    window.remove_window();
                }),
            )
            .body(body)
    }
}

impl NetworkPanel {
    /// Tailscale's switch and state; its name opens its own panel, with a way back here.
    fn tailscale(&self, ts: &tailscale::State) -> Div {
        let up = ts.up();
        let host = self.host.clone();
        let words = vstack(1.)
            .child(
                hstack(2.)
                    .child(label("Tailscale", theme::TEXT).font_weight(theme::SEMIBOLD))
                    .child(icon("chevron-right", 14., theme::TEXT_DIM)),
            )
            .child(small(
                ts.summary(),
                if ts.needs_login() {
                    theme::WARN_TEXT
                } else {
                    theme::TEXT_DIM
                },
            ));
        tile().p(px(10.)).child(
            hstack(12.)
                .child(badge("tailscale", up))
                .child(
                    // Its hover fill reaches a little past the words.
                    div().flex_1().min_w_0().ml(px(-6.)).child(
                        bar_button("tailscale-open", words)
                            .height(38.)
                            .width(Width::Full)
                            .padding(6.)
                            .on_click(move |_, cx| {
                                host.swap(PanelKind::Tailscale, Some(PanelKind::Network), cx)
                            }),
                    ),
                )
                .child(switch("tailscale", up, move |_, _| {
                    runtime::detach(async move {
                        let _ = tailscale::run_command(tailscale::up_args(!up)).await;
                    })
                })),
        )
    }

    /// A network in range: saved or open ones connect straight away, a new secured one asks for
    /// its password first.
    fn other(&self, network: &Network, this: &Entity<Self>) -> Div {
        let ssid = network.ssid.clone();
        let note = if self.connecting.as_deref() == Some(&ssid) {
            Some(("Connecting…", theme::TEXT_DIM))
        } else if self.failed.as_deref() == Some(&ssid) {
            Some(("Failed", theme::WARN_TEXT))
        } else if network.known {
            Some(("Saved", theme::TEXT_DIM))
        } else {
            None
        };
        let mut trailing = Vec::new();
        if let Some((note, color)) = note {
            trailing.push(
                div()
                    .text_size(px(11.))
                    .text_color(color)
                    .child(note)
                    .into_any_element(),
            );
        }
        if network.secured {
            trailing.push(icon("lock", 14., theme::TEXT_DIM).into_any_element());
        }
        let asks = !(network.known || !network.secured);
        let click = {
            let (this, ssid) = (this.clone(), ssid.clone());
            move |window: &mut Window, cx: &mut gpui::App| {
                this.update(cx, |this, cx| {
                    if asks {
                        this.ask(ssid.clone(), window, cx);
                    } else {
                        this.connect(ssid.clone(), None, cx);
                    }
                });
            }
        };
        let row = list_row(
            SharedString::from(format!("net-{ssid}")),
            Some(network.icon()),
            ssid.clone(),
            None,
            trailing,
        )
        .on_click(click);
        let mut column = vstack(0.).child(row);
        if self.asking.as_deref() == Some(&ssid) {
            // Its password, under it.
            let submit = this.clone();
            column = column.child(
                hstack(8.)
                    .pl(px(40.))
                    .pr(px(10.))
                    .pt(px(4.))
                    .pb(px(8.))
                    .child(div().flex_1().child(Input::new(&self.password)))
                    .child(
                        bar_button("join", icon("check", 16., theme::INK))
                            .height(32.)
                            .radius(9.)
                            .rest(theme::ACCENT)
                            .on_click(move |_, cx| submit.update(cx, |this, cx| this.submit(cx))),
                    ),
            );
        }
        column
    }
}

/// An icon in a circle, white when it's on.
fn badge(name: &str, on: bool) -> Div {
    div()
        .flex_none()
        .size(px(34.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(17.))
        .bg(if on { theme::ACCENT } else { theme::FILL_HOVER })
        .child(icon(name, 17., if on { theme::INK } else { theme::TEXT }))
}

/// A title over a dim status line, next to its badge.
fn titled(
    name: &str,
    on: bool,
    title: impl Into<SharedString>,
    status: impl Into<SharedString>,
) -> Div {
    hstack(12.).child(badge(name, on)).child(
        vstack(1.)
            .flex_1()
            .min_w_0()
            .child(
                label(title, theme::TEXT)
                    .font_weight(theme::SEMIBOLD)
                    .truncate(),
            )
            .child(small(status, theme::TEXT_DIM)),
    )
}

fn wired(link: &Wired) -> Div {
    let status = if link.connected {
        match link.speed_mbps {
            Some(s) if s >= 1000 => format!("Connected · {} Gb/s", s as f32 / 1000.),
            Some(s) => format!("Connected · {s} Mb/s"),
            None => "Connected".into(),
        }
    } else if link.has_link {
        "Not connected".into()
    } else {
        "Cable unplugged".into()
    };
    tile()
        .child(titled("ethernet", link.connected, "Wired", status))
        .when(link.connected, |t| {
            t.child(details(link.ip.clone(), link.interface.clone()))
        })
}

/// The address and something more under a connected link.
fn details(ip: Option<String>, extra: String) -> Div {
    hstack(16.)
        .pl(px(46.))
        .children(ip.map(mono))
        .child(mono(extra))
}

fn connected(network: &Network, ip: Option<&str>, interface: &str) -> Div {
    let mut status = vec!["Connected".to_string()];
    status.extend(network.band().map(str::to_string));
    if network.secured {
        status.push("Secured".into());
    }
    let interface = interface.to_string();
    tile()
        .child(
            titled(
                network.icon(),
                true,
                network.ssid.clone(),
                status.join(" · "),
            )
            .child(
                pill("disconnect", "Disconnect")
                    .on_click(move |_, _| runtime::detach(network::disconnect(interface.clone()))),
            ),
        )
        .child(details(
            ip.map(str::to_string),
            format!("Signal {}%", network.strength),
        ))
}
