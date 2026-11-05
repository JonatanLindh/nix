//! The Tailscale panel (TsPanel.qml): the switch, this device, the exit node, what's served or
//! funnelled (and adding to it), and the other devices. Opened from the network panel (with a
//! way back) or from the bar's funnel icon. Clicking an address or URL copies it.

use std::rc::Rc;
use std::time::Duration;

use gpui::component::input::{Input, InputEvent, InputState};
use gpui::{
    AnyElement, App, Context, Div, Entity, Rgba, SharedString, Window, div, prelude::*, px,
};

use super::{Host, frame};
use crate::services::tailscale::{self, FUNNEL_PORTS, Peer, Serve};
use crate::services::{Services, spawn};
use crate::ui::controls::{circle_button, footer_link, pill, section, switch, tile};
use crate::ui::{BarButton, Width, bar_button, hstack, icon, label, small, vstack};
use crate::{runtime, theme};

/// Theme.mic: what's public is marked in orange.
const PUBLIC: Rgba = theme::alpha(
    Rgba {
        r: 1.,
        g: 0xa9 as f32 / 255.,
        b: 0x4d as f32 / 255.,
        a: 1.,
    },
    0.28,
);
const PUBLIC_TEXT: Rgba = Rgba {
    r: 1.,
    g: 0xc5 as f32 / 255.,
    b: 0x8a as f32 / 255.,
    a: 1.,
};
const FAINT: Rgba = theme::alpha(theme::TEXT, 0.6);
const TINY: f32 = 11.;
/// How long a copied value's row says "Copied".
const COPIED_FOR: Duration = Duration::from_millis(1500);
/// Devices listed, at most.
const PEERS: usize = 8;

pub struct TailscalePanel {
    services: Services,
    host: Host,
    /// The last command's failure.
    error: Option<String>,
    /// The form for a new entry is open.
    adding: bool,
    new_port: &'static str,
    /// Public is always an explicit choice.
    new_public: bool,
    target: Entity<InputState>,
    path: Entity<InputState>,
    exit_open: bool,
    /// Value most recently copied, and which copy it was (for the reset).
    copied: Option<(String, u64)>,
    copies: u64,
    _subscriptions: Vec<gpui::Subscription>,
}

impl TailscalePanel {
    pub fn new(host: Host, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        let target =
            cx.new(|cx| InputState::new(window, cx).placeholder("3000, localhost:8080, /some/dir"));
        let path = cx.new(|cx| InputState::new(window, cx).placeholder("/"));
        let enter = |this: &mut Self, _, event: &InputEvent, cx: &mut Context<Self>| {
            if let InputEvent::PressEnter { .. } = event {
                this.submit(cx);
            }
        };
        let subscriptions = vec![
            cx.observe(&services.tailscale, crate::ui::redraw),
            cx.subscribe(&target, enter),
            cx.subscribe(&path, enter),
        ];
        // State is pushed, so this is only for a fresh look.
        tailscale::refresh();
        Self {
            services,
            host,
            error: None,
            adding: false,
            new_port: FUNNEL_PORTS[0],
            new_public: false,
            target,
            path,
            exit_open: false,
            copied: None,
            copies: 0,
            _subscriptions: subscriptions,
        }
    }

    /// Runs `tailscale <args>`, showing its error if it fails.
    fn run(&mut self, args: Vec<String>, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
        runtime::then(cx, tailscale::run_command(args), |this, result, cx| {
            this.error = result.err();
            cx.notify();
        });
    }

    fn copy(&mut self, text: String, cx: &mut Context<Self>) {
        let arg = text.clone();
        runtime::detach(async move { spawn("wl-copy", &["--", &arg]).await });
        self.copies += 1;
        let copy = self.copies;
        self.copied = Some((text, copy));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            let _ = this.update(cx, |this, cx| {
                if this.copied.as_ref().is_some_and(|(_, c)| *c == copy) {
                    this.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn was_copied(&self, text: &str) -> bool {
        self.copied.as_ref().is_some_and(|(t, _)| t == text)
    }

    fn set_adding(&mut self, adding: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.adding = adding;
        if adding {
            self.new_port = FUNNEL_PORTS[0];
            self.new_public = false;
            for input in [&self.target, &self.path] {
                input.update(cx, |input, cx| input.set_value("", window, cx));
            }
            self.target.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let target = self.target.read(cx).value().trim().to_string();
        if target.is_empty() {
            return;
        }
        let path = self.path.read(cx).value().trim().to_string();
        let path = if path.is_empty() { "/".into() } else { path };
        let args = tailscale::serve_args(self.new_port, &target, self.new_public, &path);
        self.adding = false;
        self.run(args, cx);
    }
}

impl Render for TailscalePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ts = self.services.tailscale.read(cx).value.clone();
        let this = cx.entity();
        let mut frame = frame("Tailscale").back(&self.host);
        if ts.available {
            let up = ts.up();
            let flip = this.clone();
            frame = frame.action(switch("tailscale", up, move |_, cx| {
                flip.update(cx, |this, cx| this.run(tailscale::up_args(!up), cx))
            }));
        }
        let frame = frame.footer(footer_link("tailscale-admin", "Admin console…").on_click(
            |window, _| {
                runtime::detach(spawn(
                    "xdg-open",
                    &["https://login.tailscale.com/admin/machines"],
                ));
                window.remove_window();
            },
        ));

        let mut body = vstack(10.);
        if let Some(error) = &self.error {
            body = body.child(
                div()
                    .px(px(6.))
                    .child(small(error.clone(), theme::WARN_TEXT)),
            );
        }
        if !ts.up() {
            return frame.body(
                body.child(
                    div()
                        .flex()
                        .justify_center()
                        .py(px(18.))
                        .px(px(6.))
                        .text_center()
                        .child(label(ts.down_reason(), theme::TEXT_DIM)),
                ),
            );
        }

        if let Some(me) = &ts.me {
            let copied = self.was_copied(&me.ip);
            let ip = me.ip.clone();
            let copy = this.clone();
            body = body.child(
                tile().child(
                    hstack(12.)
                        .child(badge("tailscale"))
                        .child(
                            vstack(2.)
                                .flex_1()
                                .min_w_0()
                                .child(
                                    hstack(4.)
                                        .child(
                                            label(me.name.clone(), theme::TEXT)
                                                .font_weight(theme::SEMIBOLD)
                                                .truncate(),
                                        )
                                        .child(small("· this device", FAINT).flex_none()),
                                )
                                .child(if copied {
                                    small("Copied", theme::ACCENT)
                                } else {
                                    mono(me.ip.clone(), theme::alpha(theme::TEXT, 0.75))
                                }),
                        )
                        .child(
                            circle_button(
                                "copy-ip",
                                if copied { "check" } else { "copy" },
                                theme::TEXT,
                            )
                            .on_click(move |_, cx| {
                                copy.update(cx, |this, cx| this.copy(ip.clone(), cx))
                            }),
                        ),
                ),
            );
        }

        let exits: Vec<Peer> = ts.exit_options().cloned().collect();
        if !exits.is_empty() {
            body = body.child(section("Exit node")).child(
                self.exit_picker(exits, ts.exit_node().is_some(), &this)
                    .mt(px(-4.)),
            );
        }

        let add = this.clone();
        let adding = self.adding;
        body = body.child(
            section("Serving").child(
                pill("add", if adding { "Cancel" } else { "+ Add" })
                    .height(24.)
                    .when(adding, |p| p.rest(theme::FILL_ACTIVE))
                    .on_click(move |window, cx| {
                        add.update(cx, |this, cx| this.set_adding(!adding, window, cx))
                    }),
            ),
        );
        if !ts.serves.is_empty() {
            let mut list = vstack(2.).mt(px(-4.));
            for (i, serve) in ts.serves.iter().enumerate() {
                list = list.child(self.serve_row(i, serve, &this));
            }
            body = body.child(list);
        } else if !adding {
            body = body.child(
                div()
                    .pl(px(10.))
                    .mt(px(-4.))
                    .child(small("Nothing served", theme::TEXT_FAINT)),
            );
        }
        if adding {
            body = body.child(self.add_form(&this));
        }

        if !ts.peers.is_empty() {
            let mut list = vstack(2.).mt(px(-4.));
            for peer in ts.peers.iter().take(PEERS) {
                list = list.child(self.peer_row(peer, &this));
            }
            body = body.child(section("Devices")).child(list);
        }

        frame.body(body)
    }
}

impl TailscalePanel {
    /// The exit node in use, unfolding into the choices (Dropdown.qml with notes): none, or a
    /// device offering to be one. Offline ones can't be picked. Stays open after a pick, so
    /// neighbouring ones can be tried in turn.
    fn exit_picker(&self, exits: Vec<Peer>, routing: bool, this: &Entity<Self>) -> Div {
        let current = exits
            .iter()
            .position(|p| p.exit_active)
            .map_or(0, |i| i + 1);
        let mut choices = vec![("None".to_string(), "Direct", None, true)];
        choices.extend(exits.iter().map(|p| {
            let note = if p.online { "Online" } else { "Offline" };
            (p.node.name.clone(), note, Some(p.node.ip.clone()), p.online)
        }));

        let open = self.exit_open;
        let toggle = this.clone();
        let head = hstack(10.)
            .w_full()
            .child(
                label(choices[current].0.clone(), theme::TEXT)
                    .flex_1()
                    .truncate(),
            )
            .when(routing, |row| {
                row.child(small("Routing all traffic", theme::TEXT_DIM))
            })
            .child(icon(
                if open {
                    "chevron-down"
                } else {
                    "chevron-right"
                },
                13.,
                theme::TEXT_DIM,
            ));
        let mut picker = vstack(4.).w_full().child(
            bar_button("exit-head", head)
                .height(38.)
                .width(Width::Full)
                .radius(12.)
                .padding(12.)
                .rest(theme::FILL)
                .active(open)
                .on_click(move |_, cx| {
                    toggle.update(cx, |this, cx| {
                        this.exit_open = !this.exit_open;
                        cx.notify();
                    })
                }),
        );
        if open {
            let mut options = vstack(2.)
                .w_full()
                .p(px(4.))
                .rounded(px(14.))
                .bg(theme::alpha(theme::WHITE, 0.06))
                .border_1()
                .border_color(theme::GLASS_BORDER);
            for (i, (name, note, ip, enabled)) in choices.into_iter().enumerate() {
                let check = icon("check", 16., theme::ACCENT).when(i != current, |c| c.opacity(0.));
                let content = hstack(10.)
                    .w_full()
                    .when(!enabled, |row| row.opacity(0.45))
                    .child(check)
                    .child(label(name, theme::TEXT).flex_1().truncate())
                    .child(
                        div()
                            .text_size(px(TINY))
                            .text_color(theme::TEXT_FAINT)
                            .child(note),
                    );
                let mut option = bar_button(SharedString::from(format!("exit-{i}")), content)
                    .height(34.)
                    .width(Width::Full)
                    .radius(9.)
                    .padding(10.);
                if enabled {
                    let pick = this.clone();
                    option = option.on_click(move |_, cx| {
                        let args = tailscale::exit_node_args(ip.as_deref());
                        pick.update(cx, |this, cx| this.run(args, cx));
                    });
                }
                options = options.child(option);
            }
            picker = picker.child(options);
        }
        picker
    }

    /// Something served: click copies its URL; the chip flips it between the tailnet and the
    /// internet; the cross takes it down.
    fn serve_row(&self, i: usize, serve: &Serve, this: &Entity<Self>) -> BarButton {
        let copied = self.was_copied(&serve.url);
        let text: AnyElement = if copied {
            small("Copied", theme::ACCENT).into_any_element()
        } else {
            let path = if serve.path == "/" { "" } else { &serve.path };
            hstack(0.)
                .child(mono(format!(":{}{path} ", serve.port), theme::TEXT))
                .child(mono("→ ", theme::alpha(theme::TEXT, 0.5)))
                .child(mono(
                    tailscale::short_target(&serve.target).to_string(),
                    theme::TEXT,
                ))
                .into_any_element()
        };

        let who = if serve.funnel { "Public" } else { "Tailnet" };
        let flip = (this.clone(), serve.clone());
        let chip = bar_button(
            SharedString::from(format!("serve-who-{i}")),
            div()
                .text_size(px(TINY))
                .text_color(if serve.funnel {
                    PUBLIC_TEXT
                } else {
                    theme::TEXT
                })
                .child(who),
        )
        .height(22.)
        .radius(11.)
        .padding(9.)
        .rest(if serve.funnel {
            PUBLIC
        } else {
            theme::FILL_HOVER
        })
        .on_click(move |_, cx| {
            let (this, serve) = &flip;
            let args =
                tailscale::serve_args(&serve.port, &serve.target, !serve.funnel, &serve.path);
            this.update(cx, |this, cx| this.run(args, cx));
        });

        let remove = (this.clone(), serve.clone());
        let close = bar_button(
            SharedString::from(format!("serve-off-{i}")),
            icon("close", 12., theme::TEXT),
        )
        .height(24.)
        .radius(12.)
        .padding(6.)
        .on_click(move |_, cx| {
            let (this, serve) = &remove;
            let args = tailscale::remove_args(serve);
            this.update(cx, |this, cx| this.run(args, cx));
        });

        let (copy, url) = (this.clone(), serve.url.clone());
        bar_button(
            SharedString::from(format!("serve-{i}")),
            hstack(8.)
                .w_full()
                .child(div().flex_1().min_w_0().overflow_hidden().child(text))
                .child(chip)
                .child(close),
        )
        .height(40.)
        .width(Width::Full)
        .radius(10.)
        .padding(8.)
        .on_click(move |_, cx| copy.update(cx, |this, cx| this.copy(url.clone(), cx)))
    }

    /// A new entry: port, what to serve, at which path, and to whom.
    fn add_form(&self, this: &Entity<Self>) -> Div {
        let port = FUNNEL_PORTS
            .iter()
            .position(|p| *p == self.new_port)
            .unwrap_or(0);
        let ports = this.clone();
        let who = this.clone();
        let submit = this.clone();
        let field = |name: &'static str, control: AnyElement| {
            hstack(12.)
                .w_full()
                .child(div().w(px(36.)).child(small(name, theme::TEXT_DIM)))
                .child(div().flex_1().min_w_0().child(control))
        };
        tile()
            .gap(px(8.))
            .child(field(
                "Port",
                segmented("port", &FUNNEL_PORTS, port, move |i, _, cx| {
                    ports.update(cx, |this, cx| {
                        this.new_port = FUNNEL_PORTS[i];
                        cx.notify();
                    })
                })
                .into_any_element(),
            ))
            .child(field("To", Input::new(&self.target).into_any_element()))
            .child(field("Path", Input::new(&self.path).into_any_element()))
            .child(field(
                "Who",
                hstack(8.)
                    .child(
                        segmented(
                            "who",
                            &["Tailnet", "Public"],
                            usize::from(self.new_public),
                            move |i, _, cx| {
                                who.update(cx, |this, cx| {
                                    this.new_public = i == 1;
                                    cx.notify();
                                })
                            },
                        )
                        .flex_1(),
                    )
                    .child(
                        bar_button(
                            "add-serve",
                            hstack(6.)
                                .child(icon("check", 13., theme::INK))
                                .child(small("Add", theme::INK)),
                        )
                        .height(32.)
                        .radius(16.)
                        .padding(12.)
                        .rest(theme::ACCENT)
                        .on_click(move |_, cx| submit.update(cx, |this, cx| this.submit(cx))),
                    )
                    .into_any_element(),
            ))
    }

    /// Another device: click copies its MagicDNS name.
    fn peer_row(&self, peer: &Peer, this: &Entity<Self>) -> BarButton {
        let copied = self.was_copied(&peer.node.dns);
        let dot = div()
            .flex_none()
            .size(px(8.))
            .rounded(px(4.))
            .bg(if peer.online {
                theme::ONLINE
            } else {
                theme::alpha(theme::TEXT, 0.3)
            });
        let end = if copied {
            small("Copied", theme::ACCENT)
        } else if peer.online {
            mono(peer.node.ip.clone(), FAINT)
        } else {
            small("offline", FAINT)
        };
        let content = hstack(12.)
            .w_full()
            .child(dot)
            .child(
                label(
                    peer.node.name.clone(),
                    if peer.online {
                        theme::TEXT
                    } else {
                        theme::alpha(theme::TEXT, 0.55)
                    },
                )
                .flex_1()
                .truncate(),
            )
            .child(end);
        let (copy, dns) = (this.clone(), peer.node.dns.clone());
        bar_button(
            SharedString::from(format!("peer-{}", peer.node.dns)),
            content,
        )
        .height(40.)
        .width(Width::Full)
        .radius(10.)
        .padding(10.)
        .on_click(move |_, cx| copy.update(cx, |this, cx| this.copy(dns.clone(), cx)))
    }
}

/// The accent circle with an icon in it.
fn badge(name: &str) -> Div {
    div()
        .flex_none()
        .size(px(34.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(17.))
        .bg(theme::ACCENT)
        .child(icon(name, 17., theme::INK))
}

/// Small monospace text, for addresses and ports.
fn mono(text: impl Into<SharedString>, color: Rgba) -> Div {
    div()
        .font_family(theme::MONO)
        .text_size(px(theme::SMALL))
        .text_color(color)
        .child(text.into())
}

/// A segmented control (Segmented.qml): one of `options`, picked with a click.
fn segmented(
    id: &str,
    options: &[&str],
    current: usize,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let on_pick = Rc::new(on_pick);
    let mut row = hstack(2.)
        .h(px(32.))
        .p(px(3.))
        .rounded(px(12.))
        .bg(theme::FILL);
    for (i, option) in options.iter().enumerate() {
        let on = i == current;
        let mut segment = bar_button(
            SharedString::from(format!("{id}-{i}")),
            small(
                option.to_string(),
                if on { theme::WHITE } else { theme::TEXT_DIM },
            ),
        )
        .height(26.)
        .width(Width::Full)
        .radius(9.)
        .padding(6.)
        .centered();
        // The picked one takes no clicks, so it keeps its fill under the pointer.
        if on {
            segment = segment.rest(theme::FILL_ACTIVE);
        } else {
            let on_pick = on_pick.clone();
            segment = segment.on_click(move |window, cx| on_pick(i, window, cx));
        }
        row = row.child(div().flex_1().min_w_0().child(segment));
    }
    row
}
