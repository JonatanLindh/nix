//! Data sources: the compositor, sound, network, power, the tray, what's playing.
//!
//! Each source runs on the tokio runtime (`crate::runtime`) and reports its whole state on every
//! change. On the UI side that state lives in an entity, so views `observe` the sources they
//! show and redraw when one changes. Read-only sources are a [`Feed`]; those with actions or
//! local state of their own (sound, caffeine, the colour picker) are entities with methods.
//!
//! Every source is reachable through the [`Services`] global.

pub mod agenda;
pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod brightness;
pub mod caffeine;
pub mod clock;
pub mod easyeffects;
pub mod hyprland;
pub mod icons;
pub mod ipc;
pub mod launcher;
pub mod media;
pub mod network;
pub mod notifications;
pub mod picker;
pub mod power_profiles;
pub mod privacy;
pub mod screenshot;
pub mod shelf;
pub mod tailscale;
pub mod tray;
mod wirehose;

use std::time::Duration;

use chrono::{DateTime, Local};
use futures::channel::mpsc::Sender;
use futures::{Stream, StreamExt};
use gpui::{App, AppContext, Context, Entity, Global};

/// A source's latest state, kept current from its stream.
pub struct Feed<T> {
    pub value: T,
}

impl<T: PartialEq + 'static> Feed<T> {
    /// Starts at `initial`, then takes each value `stream` reports. Views observing the entity
    /// are notified when the value actually changes.
    pub fn new(initial: T, stream: impl Stream<Item = T> + 'static, cx: &mut App) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            cx.spawn(async move |this, cx| {
                let mut stream = std::pin::pin!(stream);
                while let Some(value) = stream.next().await {
                    let alive = this.update(cx, |feed, cx| {
                        if feed.value != value {
                            feed.value = value;
                            cx.notify();
                        }
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
            Self { value: initial }
        })
    }
}

/// Every source, for views to read and observe.
#[derive(Clone)]
pub struct Services {
    pub hypr: Entity<Feed<hyprland::State>>,
    pub clock: Entity<Feed<DateTime<Local>>>,
    pub audio: Entity<audio::Audio>,
    pub network: Entity<Feed<Option<network::State>>>,
    /// `None` while there is no adapter.
    pub bluetooth: Entity<Feed<Option<bluetooth::State>>>,
    /// `None` on a machine without a laptop battery.
    pub battery: Entity<Feed<Option<battery::State>>>,
    pub tray: Entity<Feed<Vec<tray::Item>>>,
    pub media: Entity<Feed<Option<media::Track>>>,
    pub caffeine: Entity<caffeine::Caffeine>,
    pub picker: Entity<picker::Picker>,
    /// What records the microphone, camera or screen.
    pub privacy: Entity<Feed<privacy::State>>,
    pub agenda: Entity<agenda::Agenda>,
    pub tailscale: Entity<Feed<tailscale::State>>,
    pub power_profiles: Entity<power_profiles::PowerProfiles>,
    pub brightness: Entity<brightness::Brightness>,
    pub easyeffects: Entity<easyeffects::EasyEffects>,
}

impl Global for Services {}

impl Services {
    /// Starts every source.
    pub fn init(cx: &mut App) {
        let services = Services {
            hypr: Feed::new(hyprland::State::default(), hyprland::watch(), cx),
            clock: Feed::new(Local::now(), clock::watch(), cx),
            audio: audio::Audio::new(cx),
            network: Feed::new(None, network::watch().map(Some), cx),
            bluetooth: Feed::new(None, bluetooth::watch(), cx),
            battery: Feed::new(None, battery::watch(), cx),
            tray: Feed::new(Vec::new(), tray::watch(), cx),
            media: Feed::new(None, media::watch(), cx),
            caffeine: cx.new(|_| caffeine::Caffeine::default()),
            picker: cx.new(|_| picker::Picker::new()),
            privacy: Feed::new(privacy::State::default(), privacy::watch(), cx),
            agenda: agenda::Agenda::new(cx),
            tailscale: Feed::new(tailscale::State::default(), tailscale::watch(), cx),
            power_profiles: power_profiles::PowerProfiles::new(cx),
            brightness: brightness::Brightness::new(cx),
            easyeffects: easyeffects::EasyEffects::new(cx),
        };
        cx.set_global(services);
        shelf::start();
    }

    pub fn get(cx: &App) -> Self {
        cx.global::<Self>().clone()
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T = ()> = std::result::Result<T, Error>;

/// Runs `run` for as long as anyone listens on `out`, restarting it `retry` after it fails, so a
/// daemon that restarts or starts late is picked up again. Logs each distinct failure once.
pub async fn supervise<T, Fut>(
    name: &str,
    retry: Duration,
    out: Sender<T>,
    run: impl Fn(Sender<T>) -> Fut,
) where
    Fut: Future<Output = Result>,
{
    let mut last_error = String::new();
    while !out.is_closed() {
        if let Err(e) = run(out.clone()).await {
            let e = e.to_string();
            if e != last_error {
                eprintln!("{name}: {e}");
                last_error = e;
            }
        }
        tokio::time::sleep(retry).await;
    }
}

/// Runs a command for its side effect, logging a failure.
pub async fn spawn(program: &str, args: &[&str]) {
    match tokio::process::Command::new(program)
        .args(args)
        .status()
        .await
    {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("{program} {args:?}: {s}"),
        Err(e) => eprintln!("{program}: {e}"),
    }
}
