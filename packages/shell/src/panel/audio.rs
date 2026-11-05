//! The sound panel (AudioPanel.qml): what's playing, the output with its signal chain,
//! EasyEffects, and the output and input devices.

mod now_playing;

use gpui::{Context, Div, Entity, SharedString, Window, div, prelude::*, px};

use self::now_playing::NowPlaying;
use super::frame;
use crate::services::audio::{Audio, Device};
use crate::services::{Services, easyeffects};
use crate::ui::controls::{
    Choice, chip, circle_button, dropdown, footer_link, section, switch, tile,
};
use crate::ui::slider::split_slider;
use crate::ui::{hstack, icon, label, small, vstack};
use crate::{runtime, theme};

/// The dropdown that's unfolded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picker {
    Output,
    Input,
    Preset,
}

pub struct AudioPanel {
    services: Services,
    now_playing: Entity<NowPlaying>,
    picker: Option<Picker>,
    _observers: Vec<gpui::Subscription>,
}

impl AudioPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        let observers = vec![
            cx.observe(&services.audio, crate::ui::redraw),
            cx.observe(&services.media, crate::ui::redraw),
            cx.observe(&services.easyeffects, crate::ui::redraw),
        ];
        // Opens with the state last read; this catches changes made in EasyEffects itself.
        services
            .easyeffects
            .update(cx, |easyeffects, cx| easyeffects.refresh(cx));
        Self {
            services,
            now_playing: cx.new(NowPlaying::new),
            picker: None,
            _observers: observers,
        }
    }

    fn toggle_picker(&mut self, which: Picker, cx: &mut Context<Self>) {
        self.picker = (self.picker != Some(which)).then_some(which);
        cx.notify();
    }
}

impl Render for AudioPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let audio = self.services.audio.clone();
        let Some(state) = audio.read(cx).state.clone() else {
            return frame("Sound")
                .body(label("No sound server", theme::TEXT_DIM))
                .into_any_element();
        };
        let this = cx.entity();
        let effects = self.services.easyeffects.read(cx).state.clone();

        let mut body = vstack(12.);
        if self.services.media.read(cx).value.is_some() {
            body = body.child(self.now_playing.clone());
        }

        // Output: level, mute, and where sound goes on its way to the device.
        if let Some(output) = &state.output {
            let mut chain = hstack(6.).flex_wrap().child(chip("Apps", false, true));
            for filter in &state.filters {
                // A bypassed EasyEffects passes sound through untouched.
                let lit = filter != "EasyEffects" || effects.as_ref().is_some_and(|e| !e.bypass);
                chain = chain.child(arrow()).child(chip(filter.clone(), lit, false));
            }
            chain = chain
                .child(arrow())
                .child(chip(output.label.clone(), false, false));
            body = body.child(
                tile()
                    .child(level_header(
                        "output",
                        "Output",
                        output,
                        "volume",
                        "volume-mute",
                        &audio,
                    ))
                    .child(level(
                        "output-level",
                        output,
                        if output.muted {
                            "volume-mute"
                        } else {
                            "volume-low"
                        },
                        &audio,
                    ))
                    .child(chain),
            );
        }

        if state.easyeffects {
            body = body.child(self.easyeffects_tile(effects.as_ref(), &this));
        }

        body = body.child(
            vstack(6.)
                .child(section("Output device"))
                .child(self.device_picker(
                    &state.sinks,
                    state.output.as_ref(),
                    Picker::Output,
                    &this,
                )),
        );

        if !state.sources.is_empty() {
            let mut input = vstack(8.);
            input = match &state.input {
                Some(source) => input.child(level_header(
                    "input", "Input", source, "mic", "mic-off", &audio,
                )),
                None => input.child(section("Input")),
            };
            input = input.child(self.device_picker(
                &state.sources,
                state.input.as_ref(),
                Picker::Input,
                &this,
            ));
            if let Some(source) = &state.input {
                input = input.child(level(
                    "input-level",
                    source,
                    if source.muted { "mic-off" } else { "mic" },
                    &audio,
                ));
            }
            body = body.child(input);
        }

        let mut frame = frame("Sound");
        if state.easyeffects {
            frame = frame.footer(launch(
                footer_link("easyeffects-app", "EasyEffects…"),
                &["easyeffects"],
            ));
        }
        frame
            .footer(div().flex_1())
            .footer(launch(
                footer_link("sound-settings", "Sound settings…"),
                &["pavucontrol"],
            ))
            .body(body)
            .into_any_element()
    }
}

impl AudioPanel {
    fn device_picker(
        &self,
        devices: &[Device],
        current: Option<&Device>,
        which: Picker,
        this: &Entity<Self>,
    ) -> Div {
        let choices = devices
            .iter()
            .map(|d| Choice {
                label: d.label.clone().into(),
                icon: Some(d.icon),
            })
            .collect();
        let current = current.and_then(|c| devices.iter().position(|d| d.id == c.id));
        let ids: Vec<_> = devices.iter().map(|d| d.id).collect();
        let (toggle, pick) = (this.clone(), this.clone());
        let audio = self.services.audio.clone();
        dropdown(
            format!("{which:?}"),
            choices,
            current,
            self.picker == Some(which),
            true,
            move |_, cx| toggle.update(cx, |this, cx| this.toggle_picker(which, cx)),
            move |i, _, cx| {
                audio.update(cx, |audio, _| audio.set_default(ids[i]));
                pick.update(cx, |this, cx| {
                    this.picker = None;
                    cx.notify();
                });
            },
        )
    }

    fn easyeffects_tile(&self, state: Option<&easyeffects::State>, this: &Entity<Self>) -> Div {
        let on = state.is_some_and(|s| !s.bypass);
        let flip = self.services.easyeffects.clone();
        let mut content = tile().child(
            hstack(0.)
                .pl(px(4.))
                .child(
                    label("EasyEffects", theme::TEXT)
                        .flex_1()
                        .font_weight(theme::SEMIBOLD),
                )
                // On means effects apply; switching it off bypasses them.
                .child(switch("easyeffects", on, move |_, cx| {
                    flip.update(cx, |easyeffects, cx| easyeffects.set_bypass(on, cx));
                })),
        );
        if let Some(state) = state.filter(|s| !s.presets.is_empty()) {
            let choices = state
                .presets
                .iter()
                .map(|p| Choice {
                    label: p.clone().into(),
                    icon: None,
                })
                .collect();
            let current = state.presets.iter().position(|p| *p == state.preset);
            let presets = state.presets.clone();
            let (toggle, close) = (this.clone(), this.clone());
            let pick = self.services.easyeffects.clone();
            content = content.child(dropdown(
                "preset",
                choices,
                current,
                self.picker == Some(Picker::Preset),
                on,
                move |_, cx| toggle.update(cx, |this, cx| this.toggle_picker(Picker::Preset, cx)),
                move |i, _, cx| {
                    let preset = presets[i].clone();
                    close.update(cx, |this, cx| {
                        this.picker = None;
                        cx.notify();
                    });
                    pick.update(cx, |easyeffects, cx| easyeffects.load(preset, cx));
                },
            ));
        }
        content
    }
}

/// Runs a program and closes the panel ("Sound settings…").
fn launch(button: crate::ui::BarButton, command: &'static [&'static str]) -> crate::ui::BarButton {
    button.on_click(move |window, _| {
        if let [program, args @ ..] = command {
            runtime::detach(crate::services::spawn(program, args));
        }
        window.remove_window();
    })
}

/// "Output   65  🔊": the level as a number, and a mute button showing the state.
fn level_header(
    id: &'static str,
    title: &'static str,
    device: &Device,
    on_icon: &'static str,
    muted_icon: &'static str,
    audio: &Entity<Audio>,
) -> Div {
    let value: SharedString = if device.muted {
        "Muted".into()
    } else {
        ((device.volume * 100.).round() as u32).to_string().into()
    };
    let (device_id, audio) = (device.id, audio.clone());
    hstack(8.)
        .pl(px(6.))
        .child(small(title, theme::TEXT_DIM).flex_1())
        .child(
            div()
                .font_family(theme::MONO)
                .text_size(px(theme::SMALL))
                .child(value),
        )
        .child(
            circle_button(
                SharedString::from(format!("{id}-mute")),
                if device.muted { muted_icon } else { on_icon },
                if device.muted {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                },
            )
            .on_click(move |_, cx| audio.update(cx, |audio, cx| audio.toggle_mute(device_id, cx))),
        )
}

fn level(
    id: &'static str,
    device: &Device,
    icon: &'static str,
    audio: &Entity<Audio>,
) -> impl IntoElement {
    let (device_id, audio) = (device.id, audio.clone());
    split_slider(id, device.volume, icon, move |value, _, cx| {
        audio.update(cx, |audio, cx| audio.set_volume(device_id, value, cx));
    })
    .fill(if device.muted {
        theme::alpha(theme::WHITE, 0.35)
    } else {
        theme::WHITE
    })
}

fn arrow() -> impl IntoElement {
    div().h(px(22.)).flex().items_center().child(icon(
        "chevron-right",
        12.,
        theme::alpha(theme::TEXT, 0.62),
    ))
}
