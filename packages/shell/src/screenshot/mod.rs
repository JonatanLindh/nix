//! Screenshots and screen recordings (Screenshot.qml): the capture overlay's state, saving a shot
//! cut from the overlay's frozen frame or recording with wf-recorder, and the notification
//! afterwards (click opens it in Gradia; Copy text runs the shelf's OCR; Add to shelf
//! puts it on the shelf). The overlay itself, one window per monitor, is in [`overlay`].
//!
//! Opened by `shell msg screenshot` (or `screenshot-window`, `screenshot-screen`); while a
//! recording runs, those stop it instead, as does the bar's recording indicator.

pub mod geometry;
mod overlay;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::component::Root;
use gpui::{App, AppContext, Context, Entity, Global, Task, WindowHandle, div, prelude::*, px};

use crate::services::screenshot::{self as capture, Snapshot};
use crate::ui::{bar_button, hstack, label};
use crate::{runtime, theme};
use geometry::Rect;

/// What the capture takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Selection,
    Screen,
    Window,
}

/// What a recording records.
enum Target {
    Output(String),
    /// A global logical rectangle.
    Area(Rect),
}

/// How long the overlay gets to leave the screen before a recording starts, so it isn't in it.
const SETTLE: Duration = Duration::from_millis(180);

pub struct Screenshot {
    /// The overlay is up, or about to be (its stills are being taken).
    active: bool,
    /// Counts openings, so a slow snapshot doesn't show after the overlay closed or reopened,
    /// and so each has its own stills.
    opening: u64,
    pub mode: Mode,
    pub video: bool,
    pub pointer: bool,
    /// The monitor whose overlay last saw the pointer move; only it highlights a window, as
    /// leaving one overlay for another doesn't reliably tell the first.
    pub pointer_monitor: Option<String>,
    /// The last selection per monitor, in its logical coordinates, so the next capture starts
    /// from it (like GNOME's).
    selections: HashMap<String, Rect>,
    snapshot: Option<Arc<Snapshot>>,
    windows: Vec<WindowHandle<Root>>,
    recording: Option<Recording>,
}

struct Recording {
    pid: u32,
    since: Instant,
    /// Redraws the bar's indicator every second.
    _tick: Task<()>,
}

struct GlobalScreenshot(Entity<Screenshot>);

impl Global for GlobalScreenshot {}

impl Screenshot {
    /// The one instance, made on first use.
    pub fn global(cx: &mut App) -> Entity<Self> {
        if let Some(g) = cx.try_global::<GlobalScreenshot>() {
            return g.0.clone();
        }
        let entity = cx.new(|_| Self {
            active: false,
            opening: 0,
            mode: Mode::Selection,
            video: false,
            pointer: false,
            pointer_monitor: None,
            selections: HashMap::new(),
            snapshot: None,
            windows: Vec::new(),
            recording: None,
        });
        cx.set_global(GlobalScreenshot(entity.clone()));
        entity
    }

    /// From a keybind: opens the overlay in `mode`, or stops the recording if one runs.
    pub fn open(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            self.stop(cx);
            return;
        }
        self.mode = mode;
        cx.notify();
        if self.active {
            return;
        }
        self.active = true;
        self.opening += 1;
        let opening = self.opening;
        runtime::then(
            cx,
            capture::snapshot(capture::stills_dir(opening)),
            move |this, snapshot, cx| {
                if this.active && this.opening == opening {
                    this.show(snapshot, cx);
                } else {
                    runtime::detach(capture::forget_stills(opening));
                }
            },
        );
    }

    /// Opens an overlay window on every monitor grim took.
    fn show(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        if snapshot.stills.is_empty() {
            self.active = false;
            runtime::detach(capture::note(
                "Screenshot failed".into(),
                "Couldn't capture the screen".into(),
            ));
            return;
        }
        let snapshot = Arc::new(snapshot);
        self.snapshot = Some(snapshot.clone());
        let model = cx.entity();
        // After this update: the overlays read the model as they're built.
        cx.defer(move |cx| {
            let windows = overlay::open_all(&model, &snapshot, cx);
            model.update(cx, |this, cx| {
                // Closed again while they opened.
                if !this.active {
                    windows
                        .into_iter()
                        .for_each(|w| crate::shell::close_window(w, cx));
                    return;
                }
                this.windows = windows;
                cx.notify();
            });
        });
    }

    /// Closes the overlay without capturing.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.active {
            runtime::detach(capture::forget_stills(self.opening));
        }
        self.hide(cx);
    }

    /// Takes the overlay off the screen, leaving its stills.
    fn hide(&mut self, cx: &mut Context<Self>) {
        self.active = false;
        self.snapshot = None;
        for window in self.windows.drain(..) {
            crate::shell::close_window(window, cx);
        }
        cx.notify();
    }

    fn remember(&mut self, monitor: &str, sel: Option<Rect>) {
        match sel {
            Some(sel) => self.selections.insert(monitor.to_string(), sel),
            None => self.selections.remove(monitor),
        };
    }

    /// Saves the part `crop` (physical pixels) of the still at `still`, copies it to the
    /// clipboard, then announces it.
    fn save_frozen(&mut self, still: PathBuf, crop: (u32, u32, u32, u32), cx: &mut Context<Self>) {
        let opening = self.opening;
        self.hide(cx);
        let file = capture::pictures_dir().join(format!(
            "Screenshot from {}.png",
            geometry::stamp(chrono::Local::now())
        ));
        let saved = file.clone();
        let save = async move {
            let result = capture::save(still, crop, saved).await;
            capture::forget_stills(opening).await;
            result.map_err(|e| e.to_string())
        };
        runtime::then(cx, save, move |this, result, cx| match result {
            Ok(()) => this.announce(file, false, cx),
            Err(e) => runtime::detach(capture::note(
                "Screenshot failed".into(),
                format!("Saving the capture failed: {e}"),
            )),
        });
    }

    /// Records `target` once the overlay is off the screen.
    fn record(&mut self, target: Target, cx: &mut Context<Self>) {
        self.close(cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.start_recording(target, cx));
        })
        .detach();
    }

    fn start_recording(&mut self, target: Target, cx: &mut Context<Self>) {
        let file = capture::videos_dir().join(format!(
            "Screencast from {}.mp4",
            geometry::stamp(chrono::Local::now())
        ));
        let target = match target {
            Target::Output(name) => vec!["-o".to_string(), name],
            Target::Area(rect) => vec!["-g".to_string(), geometry::geometry(rect)],
        };
        let child = std::fs::create_dir_all(capture::videos_dir()).and_then(|()| {
            std::process::Command::new("wf-recorder")
                .arg("-f")
                .arg(&file)
                .args(&target)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        });
        let mut child = match child {
            Ok(child) => child,
            Err(e) => {
                runtime::detach(capture::note(
                    "Recording failed".into(),
                    format!("wf-recorder: {e}"),
                ));
                return;
            }
        };
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        self.recording = Some(Recording {
            pid: child.id(),
            since: Instant::now(),
            _tick: tick,
        });
        cx.notify();
        let wait = async move { tokio::task::spawn_blocking(move || child.wait()).await };
        runtime::then(cx, wait, move |this, _, cx| {
            this.recording = None;
            cx.notify();
            // wf-recorder finishes the file on SIGINT; it exits 0, 130 or 255 then.
            if file.exists() {
                this.announce(file, true, cx);
            } else {
                runtime::detach(capture::note(
                    "Recording failed".into(),
                    "wf-recorder didn't write a file".into(),
                ));
            }
        });
    }

    /// Stops the recording; wf-recorder finishes the file on SIGINT.
    pub fn stop(&mut self, _: &mut Context<Self>) {
        if let Some(recording) = &self.recording {
            let pid = recording.pid.to_string();
            runtime::detach(async move { crate::services::spawn("kill", &["-INT", &pid]).await });
        }
    }

    /// The notification about `file`, and whichever of its actions is picked.
    fn announce(&mut self, file: PathBuf, video: bool, cx: &mut Context<Self>) {
        let shown = file.clone();
        runtime::then(cx, capture::notify(shown, video), move |_, action, _| {
            if let Some(action) = action {
                act(&action, &file, video);
            }
        });
    }
}

fn act(action: &str, file: &Path, video: bool) {
    let path = file.to_string_lossy().into_owned();
    if action == "shelf" {
        runtime::detach(crate::services::shelf::add(vec![path]));
    } else if action == "ocr" {
        // The shelf's OCR tool: the text goes on the clipboard, and a notification says so.
        runtime::detach(async move {
            let outcome =
                crate::services::shelf::run_action("ocr".into(), String::new(), vec![path]).await;
            match outcome.copy {
                Some(text) => {
                    crate::services::picker::copy(text);
                    capture::note(outcome.message, String::new()).await;
                }
                None => capture::note("Copy text failed".into(), outcome.message).await,
            }
        });
    } else if let Some(command) = capture::action_command(action, file, video) {
        runtime::detach(async move {
            let args: Vec<&str> = command[1..].iter().map(String::as_str).collect();
            crate::services::spawn(&command[0], &args).await;
        });
    }
}

/// The bar's recording indicator while a recording runs: a red dot and the time so far. A click
/// stops it.
pub fn indicator(cx: &mut App) -> Option<impl IntoElement + use<>> {
    let model = Screenshot::global(cx);
    let since = model.read(cx).recording.as_ref()?.since;
    let dot = div().size(px(9.)).rounded(px(4.5)).bg(theme::WARN);
    let time =
        label(geometry::elapsed(since.elapsed().as_secs()), theme::TEXT).font_family(theme::MONO);
    Some(
        bar_button("recording", hstack(6.).child(dot).child(time))
            .rest(theme::alpha(theme::WARN, 0.22))
            .on_click(move |_, cx| model.update(cx, |s, cx| s.stop(cx))),
    )
}
