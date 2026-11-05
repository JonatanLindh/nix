//! Screen brightness (Brightness.qml): the first backlight in sysfs, and setting it through
//! brightnessctl (which goes through logind, so needs no root). The desktop has no backlight,
//! which shows as `None`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::SinkExt;
use futures::channel::mpsc::Sender;

const BACKLIGHTS: &str = "/sys/class/backlight";
/// sysfs doesn't reliably tell when brightness changes (the brightness keys run brightnessctl,
/// the firmware may change it too), so it's read this often; the OSD wants it soon.
const POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// The backlight's name in sysfs ("intel_backlight").
    pub device: String,
    pub max: u32,
    pub current: u32,
}

impl State {
    pub fn fraction(&self) -> f32 {
        self.current as f32 / self.max.max(1) as f32
    }

    pub fn percent(&self) -> u32 {
        (self.fraction() * 100.).round() as u32
    }
}

fn read_number(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// The first backlight there is, by name.
fn find() -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(BACKLIGHTS)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    found.sort();
    found.into_iter().next()
}

fn read(dir: &Path) -> Option<State> {
    Some(State {
        device: dir.file_name()?.to_string_lossy().into_owned(),
        max: read_number(&dir.join("max_brightness"))?,
        current: read_number(&dir.join("brightness"))?,
    })
}

/// `None` without a backlight.
pub fn watch() -> futures::channel::mpsc::Receiver<Option<State>> {
    crate::runtime::stream(4, run)
}

async fn run(mut out: Sender<Option<State>>) {
    let mut last = None;
    let mut dir = None;
    loop {
        if dir.is_none() {
            dir = find();
        }
        let state = dir.as_deref().and_then(read);
        if state.is_none() {
            // Gone, or never there; look again, but rarely.
            dir = None;
        }
        if state != last {
            last = state.clone();
            if out.send(state).await.is_err() {
                return;
            }
        }
        tokio::time::sleep(if dir.is_some() {
            POLL
        } else {
            Duration::from_secs(30)
        })
        .await;
    }
}

/// The step brightnessctl is sent for `fraction` of the slider: whole percent, never fully dark.
pub fn percent_for(fraction: f32) -> u32 {
    ((fraction * 100.).round() as u32).clamp(1, 100)
}

/// Brightness on the UI side: as last read, or as just set here while the backlight catches up.
pub struct Brightness {
    pub state: Option<State>,
    /// When it was last set here: reads from before that mustn't move the slider back.
    held: Option<Instant>,
    /// The latest read while held, applied once the hold is over.
    stashed: Option<Option<State>>,
}

/// How long a value set here wins over what's read.
const HOLD: Duration = Duration::from_millis(600);

impl Brightness {
    pub fn new(cx: &mut gpui::App) -> gpui::Entity<Self> {
        use futures::StreamExt;
        use gpui::AppContext;
        cx.new(|cx: &mut gpui::Context<Self>| {
            cx.spawn(async move |this, cx| {
                let mut reports = watch();
                while let Some(state) = reports.next().await {
                    let alive = this.update(cx, |b, cx| b.reported(state, cx));
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
            Self {
                state: None,
                held: None,
                stashed: None,
            }
        })
    }

    fn reported(&mut self, state: Option<State>, cx: &mut gpui::Context<Self>) {
        if self.held.is_some_and(|at| at.elapsed() < HOLD) && state.is_some() {
            self.stashed = Some(state);
            return;
        }
        self.stashed = None;
        if self.state != state {
            self.state = state;
            cx.notify();
        }
    }

    /// Shows `fraction` (0–1) right away and sets it.
    pub fn set(&mut self, fraction: f32, cx: &mut gpui::Context<Self>) {
        let Some(state) = &mut self.state else { return };
        let percent = percent_for(fraction);
        state.current = (percent as f32 / 100. * state.max as f32).round() as u32;
        let at = Instant::now();
        self.held = Some(at);
        cx.notify();
        // Whatever was read meanwhile, once the hold is over.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOLD).await;
            let _ = this.update(cx, |b, cx| {
                if b.held == Some(at)
                    && let Some(state) = b.stashed.take()
                {
                    b.reported(state, cx);
                }
            });
        })
        .detach();
        let arg = format!("{percent}%");
        crate::runtime::detach(async move {
            super::spawn("brightnessctl", &["set", &arg]).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps() {
        assert_eq!(percent_for(0.), 1);
        assert_eq!(percent_for(0.424), 42);
        assert_eq!(percent_for(1.2), 100);
        let state = State {
            device: "x".into(),
            max: 400,
            current: 100,
        };
        assert_eq!(state.percent(), 25);
    }
}
