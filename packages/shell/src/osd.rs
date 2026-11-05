//! The on-screen display (Osd.qml): a glass pill (icon, 5% steps, number) at the bottom of the
//! focused monitor for a moment after the volume, mute or brightness changes, whatever changed
//! it (keys, the bar's wheel, another app).
//!
//! Only changes of a value it has already seen count, so startup and switching the output device
//! stay quiet. It stays out of the way while the bar's panel with the matching slider (sound,
//! power) is open there, and takes no input at all.

use std::time::Duration;

use gpui::component::Root;
use gpui::{
    Bounds, Context, Entity, Pixels, Size, WindowBackgroundAppearance, WindowHandle, canvas, div,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    prelude::*,
    px,
};

use crate::panel::PanelKind;
use crate::services::Services;
use crate::services::audio::DeviceId;
use crate::shell::{display_named, open_layer};
use crate::ui::{glass, hstack, icon};
use crate::{theme, wayland};

const WIDTH: f32 = 380.;
const HEIGHT: f32 = 58.;
const SEGMENTS: usize = 20;
/// Changes are ignored this long after startup or a device change: those jump the value too.
const SETTLE: Duration = Duration::from_millis(1500);
/// On screen this long after the last change.
const SHOWN: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Volume,
    Brightness,
}

/// Where it goes.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Place {
    /// The focused monitor.
    pub monitor: Option<String>,
    /// The panel open on its bar.
    pub panel: Option<PanelKind>,
}

/// What it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Reading {
    kind: Kind,
    percent: u32,
    muted: bool,
}

pub struct Osd {
    services: Services,
    shown: Option<Kind>,
    armed: bool,
    /// Bumped by each settle and each show, so only the latest timer acts.
    settles: u64,
    shows: u64,
    /// The output as last seen: device, percent, muted.
    output: Option<(DeviceId, u32, bool)>,
    /// The backlight as last seen: device, percent.
    backlight: Option<(String, u32)>,
    place: Place,
    /// The window (`None` while it opens) and the monitor it's on.
    window: Option<(Option<WindowHandle<Root>>, String)>,
    _observers: Vec<gpui::Subscription>,
}

impl Osd {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let services = Services::get(cx);
        let observers = vec![
            cx.observe(&services.audio, |osd, _, cx| osd.audio_changed(cx)),
            cx.observe(&services.brightness, |osd, _, cx| {
                osd.brightness_changed(cx)
            }),
        ];
        let mut osd = Self {
            services,
            shown: None,
            armed: false,
            settles: 0,
            shows: 0,
            output: None,
            backlight: None,
            place: Place::default(),
            window: None,
            _observers: observers,
        };
        osd.settle(cx);
        osd
    }

    fn audio_changed(&mut self, cx: &mut Context<Self>) {
        let now = self
            .services
            .audio
            .read(cx)
            .state
            .as_ref()
            .and_then(|s| Some((s.output.as_ref()?.id, s.percent(), s.muted())));
        let before = std::mem::replace(&mut self.output, now);
        match (before, now) {
            (Some((was, ..)), Some((id, ..))) if was != id => self.settle(cx),
            (Some(before), Some(now)) if before != now => self.show(Kind::Volume, cx),
            (None, Some(_)) => self.settle(cx),
            _ => {}
        }
    }

    fn brightness_changed(&mut self, cx: &mut Context<Self>) {
        let now = self
            .services
            .brightness
            .read(cx)
            .state
            .as_ref()
            // Without its maximum the percentage means nothing.
            .filter(|s| s.max > 1)
            .map(|s| (s.device.clone(), s.percent()));
        let before = std::mem::replace(&mut self.backlight, now.clone());
        match (before, now) {
            (Some((was, _)), Some((device, _))) if was != device => self.settle(cx),
            (Some(before), Some(now)) if before != now => self.show(Kind::Brightness, cx),
            _ => {}
        }
    }

    /// Ignores changes for a moment.
    fn settle(&mut self, cx: &mut Context<Self>) {
        self.armed = false;
        self.settles += 1;
        let settle = self.settles;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |osd, _| {
                if osd.settles == settle {
                    osd.armed = true;
                }
            });
        })
        .detach();
    }

    /// The real slider is on screen already.
    fn covered(&self, kind: Kind, cx: &gpui::App) -> bool {
        match kind {
            Kind::Volume => self.place.panel == Some(PanelKind::Audio),
            Kind::Brightness => {
                self.place.panel == Some(PanelKind::Power)
                    && self.services.brightness.read(cx).state.is_some()
            }
        }
    }

    fn show(&mut self, kind: Kind, cx: &mut Context<Self>) {
        if !self.armed || self.covered(kind, cx) {
            return;
        }
        self.shown = Some(kind);
        self.shows += 1;
        let show = self.shows;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SHOWN).await;
            let _ = this.update(cx, |osd, cx| {
                if osd.shows == show {
                    osd.shown = None;
                    osd.sync_window(cx);
                }
            });
        })
        .detach();
        self.sync_window(cx);
        cx.notify();
    }

    fn reading(&self, cx: &gpui::App) -> Option<Reading> {
        let kind = self.shown?;
        Some(match kind {
            Kind::Volume => {
                let audio = self.services.audio.read(cx).state.as_ref()?;
                Reading {
                    kind,
                    percent: audio.percent(),
                    muted: audio.muted(),
                }
            }
            Kind::Brightness => Reading {
                kind,
                percent: self.services.brightness.read(cx).state.as_ref()?.percent(),
                muted: false,
            },
        })
    }

    /// Where it goes, when focus moves or a panel opens or closes.
    pub fn set_place(&mut self, place: Place, cx: &mut Context<Self>) {
        if self.place != place {
            self.place = place;
            if self.shown.is_some_and(|kind| self.covered(kind, cx)) {
                self.shown = None;
            }
            self.sync_window(cx);
        }
    }

    /// On the focused monitor while something is shown.
    fn sync_window(&mut self, cx: &mut Context<Self>) {
        let want = self.shown.and(self.place.monitor.clone());
        if self.window.as_ref().map(|(_, m)| m) == want.as_ref() {
            return;
        }
        if let Some((Some(window), _)) = self.window.take() {
            let _ = window.update(cx, |_, window, _| wayland::forget(window));
            crate::shell::close_window(window, cx);
        }
        let Some(monitor) = want else {
            return;
        };
        // Opened once this update is over: the view reads this as it opens.
        self.window = Some((None, monitor.clone()));
        let model = cx.entity();
        cx.defer(move |cx| {
            let display = display_named(&monitor, cx);
            let opened = open_layer(
                cx,
                display,
                layer(),
                Size::new(px(WIDTH), px(HEIGHT)),
                WindowBackgroundAppearance::Transparent,
                |_, cx| {
                    let model = model.clone();
                    cx.new(|cx| View::new(model, cx)).into()
                },
            );
            let window = match opened {
                Ok(window) => window,
                Err(e) => return eprintln!("osd: opening its window: {e:#}"),
            };
            model.update(cx, |osd, cx| match &mut osd.window {
                Some((slot @ None, on)) if *on == monitor => *slot = Some(window),
                // Moved on while it opened.
                _ => crate::shell::close_window(window, cx),
            });
        });
    }
}

/// Centred at the bottom, over everything, ignoring exclusive zones; never takes the keyboard.
fn layer() -> LayerShellOptions {
    LayerShellOptions {
        namespace: "shell:osd".into(),
        layer: Layer::Overlay,
        // Anchored to the bottom edge only, so the compositor centres it horizontally.
        anchor: Anchor::BOTTOM,
        exclusive_zone: Some(px(-1.)),
        margin: Some((px(0.), px(0.), px(96.), px(0.))),
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    }
}

/// How many of the meter's segments are lit for `percent`.
fn lit(percent: u32) -> usize {
    let value = (percent.min(100) as f32 / 100.).clamp(0., 1.);
    (0..SEGMENTS)
        .filter(|i| (i + 1) as f32 / SEGMENTS as f32 <= value + 0.0001)
        .count()
}

struct View {
    model: Entity<Osd>,
    /// Bounds last given to the compositor for the blur.
    blurred: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    _observer: gpui::Subscription,
}

impl View {
    fn new(model: Entity<Osd>, cx: &mut Context<Self>) -> Self {
        Self {
            _observer: cx.observe(&model, crate::ui::redraw),
            model,
            blurred: Default::default(),
        }
    }
}

impl Render for View {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(reading) = self.model.read(cx).reading(cx) else {
            return div().into_any_element();
        };
        let blurred = self.blurred.clone();
        // Blur behind the pill; clicks go straight through.
        let apply = canvas(
            move |bounds, window, _| {
                if blurred.get() != Some(bounds) {
                    window.set_input_region(Some(&[]));
                    wayland::set_blur(window, &[(bounds, HEIGHT / 2.)]);
                    blurred.set(Some(bounds));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let ink = if reading.muted {
            theme::TEXT_DIM
        } else {
            theme::TEXT
        };
        let name = match reading.kind {
            _ if reading.muted => "volume-mute",
            Kind::Volume => "volume",
            Kind::Brightness => "sun",
        };
        let on = lit(reading.percent);
        // 20 segments in 5% steps (StepMeter.qml); muted dims the lit ones.
        let meter = hstack(3.).flex_1().children((0..SEGMENTS).map(|i| {
            let radius = if i == 0 || i == SEGMENTS - 1 { 7. } else { 3. };
            let color = match (i < on, reading.muted) {
                (true, false) => theme::WHITE,
                (true, true) => theme::alpha(theme::WHITE, 0.32),
                (false, _) => theme::TRACK,
            };
            div().flex_1().h(px(18.)).rounded(px(radius)).bg(color)
        }));
        let number = div()
            .flex_none()
            .w(px(48.))
            .flex()
            .justify_end()
            .text_color(ink)
            .child(if reading.muted {
                "Muted".to_string()
            } else {
                reading.percent.to_string()
            });

        crate::ui::root()
            .child(
                glass(HEIGHT / 2.)
                    .relative()
                    .size_full()
                    .child(
                        hstack(14.)
                            .size_full()
                            .pl(px(18.))
                            .pr(px(20.))
                            .child(icon(name, 20., ink))
                            .child(meter)
                            .child(number),
                    )
                    .child(apply),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lights_segments() {
        assert_eq!(lit(0), 0);
        assert_eq!(lit(4), 0);
        assert_eq!(lit(5), 1);
        assert_eq!(lit(52), 10);
        assert_eq!(lit(100), 20);
        assert_eq!(lit(150), 20);
    }
}
