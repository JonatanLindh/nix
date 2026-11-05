//! The now-playing card (NowPlaying.qml): cover art, title and artist, the transport buttons,
//! and under them the track's progress, which can be clicked or dragged to seek.
//!
//! The elapsed time is estimated (see [`media::Position`]), so the card redraws itself as it
//! ticks over to the next second, and only while the track plays and the panel is open.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Subscription, Task, Window, canvas, div, fill, img, point, prelude::*,
    px, size,
};

use crate::services::media::{self, Track};
use crate::services::{Feed, Services, icons};
use crate::ui::controls::{circle_button, mono};
use crate::ui::{hstack, icon, label, small, vstack};
use crate::{runtime, theme};

/// The progress bar's line, and the knob riding on it while it can seek.
const LINE: f32 = 4.;
const KNOB: f32 = 10.;
/// Its height, all of which takes clicks.
const HIT: f32 = 16.;

pub struct NowPlaying {
    media: Entity<Feed<Option<Track>>>,
    /// Where a drag on the progress bar is (0–1), shown until it's let go and the seek is made.
    drag: Option<f32>,
    /// Redraws as the elapsed time reaches each next second.
    ticker: Option<Task<()>>,
    _observer: Subscription,
}

impl NowPlaying {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let media = Services::get(cx).media;
        let observer = cx.observe(&media, |this, _, cx| {
            this.restart_ticker(cx);
            cx.notify();
        });
        let mut this = Self {
            media,
            drag: None,
            ticker: None,
            _observer: observer,
        };
        this.restart_ticker(cx);
        this
    }

    /// After any change to the track, so the ticks land on the new position's seconds; stops
    /// them while nothing moves.
    fn restart_ticker(&mut self, cx: &mut Context<Self>) {
        self.ticker = self.next_tick(cx).is_some().then(|| {
            cx.spawn(async move |this, cx| {
                while let Ok(Some(wait)) = this.update(cx, |this, cx| this.next_tick(cx)) {
                    cx.background_executor().timer(wait).await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
        });
    }

    fn next_tick(&self, cx: &App) -> Option<Duration> {
        let track = self.media.read(cx).value.as_ref()?;
        if !track.has_progress() {
            return None;
        }
        track.position?.until_next_second(Instant::now())
    }

    /// Seeks to `fraction` of the track, and shows it there until the player confirms.
    fn seek(&mut self, fraction: f32, cx: &mut Context<Self>) {
        self.drag = None;
        self.media.update(cx, |feed, cx| {
            let Some(track) = &mut feed.value else { return };
            let Some(length) = track.length.filter(|_| track.can_seek) else {
                return;
            };
            let to = length.mul_f32(fraction.clamp(0., 1.));
            runtime::detach(media::seek(track, to));
            track.assume_seeked(to, Instant::now());
            cx.notify();
        });
        cx.notify();
    }

    fn progress(
        &self,
        track: &Track,
        length: Duration,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let elapsed = match self.drag {
            Some(fraction) => length.mul_f32(fraction),
            None => track.position_at(Instant::now()).unwrap_or_default(),
        };
        let fraction = self
            .drag
            .unwrap_or_else(|| (elapsed.as_secs_f64() / length.as_secs_f64()) as f32)
            .clamp(0., 1.);
        let can_seek = track.can_seek;
        let this = cx.entity();
        let paint = move |b: Bounds<Pixels>, (): (), window: &mut Window, _: &mut App| {
            paint_bar(b, fraction, can_seek, window);
            if can_seek {
                listen(b, &this, window);
            }
        };
        hstack(8.)
            .w_full()
            .child(mono(clock(elapsed)))
            .child(
                div()
                    .id("seek")
                    .flex_1()
                    .h(px(HIT))
                    .when(can_seek, |d| d.cursor_pointer())
                    .child(canvas(|_, _, _| (), paint).size_full()),
            )
            .child(mono(clock(length)))
    }
}

fn paint_bar(b: Bounds<Pixels>, fraction: f32, can_seek: bool, window: &mut Window) {
    let (x, width) = (f32::from(b.origin.x), f32::from(b.size.width));
    let mid = f32::from(b.origin.y) + HIT / 2.;
    let rect =
        |x: f32, w: f32, h: f32| Bounds::new(point(px(x), px(mid - h / 2.)), size(px(w), px(h)));
    window.paint_quad(fill(rect(x, width, LINE), theme::TRACK).corner_radii(px(LINE / 2.)));
    let filled = width * fraction;
    let ink = if can_seek {
        theme::WHITE
    } else {
        theme::alpha(theme::WHITE, 0.6)
    };
    if filled > 0. {
        window.paint_quad(fill(rect(x, filled, LINE), ink).corner_radii(px(LINE / 2.)));
    }
    if can_seek {
        // Kept inside the bar at either end.
        let knob_x = (x + filled - KNOB / 2.).clamp(x, x + width - KNOB);
        window.paint_quad(fill(rect(knob_x, KNOB, KNOB), ink).corner_radii(px(KNOB / 2.)));
    }
}

/// Click or drag on the bar to seek. The seek is made on release, so a drag doesn't send the
/// player a stream of them; until then the bar follows the pointer.
fn listen(b: Bounds<Pixels>, this: &Entity<NowPlaying>, window: &mut Window) {
    let at = move |x: Pixels| {
        (f32::from(x - b.origin.x) / f32::from(b.size.width).max(1.)).clamp(0., 1.)
    };
    let dragging = |this: &Entity<NowPlaying>, cx: &App| this.read(cx).drag.is_some();
    {
        let this = this.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
            if phase.bubble() && e.button == MouseButton::Left && b.contains(&e.position) {
                cx.stop_propagation();
                this.update(cx, |this, cx| {
                    this.drag = Some(at(e.position.x));
                    cx.notify();
                });
            }
        });
    }
    // A drag carries on outside the bar, so its moves are watched window-wide.
    {
        let this = this.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
            if phase.bubble() && dragging(&this, cx) {
                let x = at(e.position.x);
                this.update(cx, |this, cx| {
                    if e.dragging() {
                        this.drag = Some(x);
                        cx.notify();
                    } else {
                        // The release happened where we didn't see it.
                        this.seek(x, cx);
                    }
                });
            }
        });
    }
    let this = this.clone();
    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
        if phase.bubble() && dragging(&this, cx) {
            this.update(cx, |this, cx| this.seek(at(e.position.x), cx));
        }
    });
}

impl Render for NowPlaying {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(track) = self.media.read(cx).value.clone() else {
            return div();
        };
        let card = vstack(10.)
            .w_full()
            .p(px(10.))
            .rounded(px(16.))
            .bg(theme::FILL)
            .child(header(&track));
        match track.length.filter(|_| track.has_progress()) {
            Some(length) => card.pb(px(8.)).child(self.progress(&track, length, cx)),
            None => card,
        }
    }
}

/// Cover art, title and artist, and the transport buttons.
fn header(track: &Track) -> impl IntoElement {
    let art: AnyElement = match &track.art {
        Some(icons::Image::Raster(image)) => img(image.clone())
            .size(px(46.))
            .rounded(px(10.))
            .into_any_element(),
        Some(icons::Image::Svg(path)) => img(path.clone())
            .size(px(46.))
            .rounded(px(10.))
            .into_any_element(),
        None => div()
            .size(px(46.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(10.))
            .bg(theme::alpha(theme::WHITE, 0.1))
            .child(icon("music", 20., theme::alpha(theme::TEXT, 0.6)))
            .into_any_element(),
    };
    let words = vstack(2.)
        .flex_1()
        .min_w_0()
        .child(
            label(track.title.clone(), theme::TEXT)
                .font_weight(theme::SEMIBOLD)
                .truncate(),
        )
        .when(!track.artist.is_empty(), |w| {
            w.child(small(track.artist.clone(), theme::TEXT_DIM).truncate())
        });
    let player = &track.player;
    hstack(12.).w_full().child(art).child(words).child(
        hstack(0.)
            .child(transport(
                "prev",
                "prev",
                track.can_go_previous,
                player,
                media::previous,
            ))
            .child(transport(
                "play",
                if track.playing { "pause" } else { "play" },
                true,
                player,
                media::play_pause,
            ))
            .child(transport(
                "next",
                "next",
                track.can_go_next,
                player,
                media::next,
            )),
    )
}

/// A big round transport button that sends `action` to `player`; dimmed and inert when the
/// player says it can't (as NowPlaying.qml dims them).
fn transport<F>(
    id: &'static str,
    name: &str,
    enabled: bool,
    player: &str,
    action: fn(String) -> F,
) -> impl IntoElement
where
    F: Future<Output = ()> + Send + 'static,
{
    let button = circle_button(id, name, theme::TEXT).height(32.).radius(16.);
    let player = player.to_string();
    div().when(!enabled, |d| d.opacity(0.4)).child(if enabled {
        button.on_click(move |_, _| runtime::detach(action(player.clone())))
    } else {
        button
    })
}

/// "3:07", or "1:02:09" past an hour.
fn clock(time: Duration) -> String {
    let s = time.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_format() {
        assert_eq!(clock(Duration::from_secs_f64(59.9)), "0:59");
        assert_eq!(clock(Duration::from_secs(187)), "3:07");
        assert_eq!(clock(Duration::from_secs(3729)), "1:02:09");
    }
}
