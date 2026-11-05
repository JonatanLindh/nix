//! The colour picker's state (ColorPicker.qml): hyprpicker picks (and copies the hex), this keeps
//! the history and whether the bar shows the slot. The slot appears the first time the picker is
//! used and stays until removed from its panel. Saved in the state dir.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Stdio;

use gpui::{Context, Rgba};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::runtime;

/// How many colours the history keeps.
const HISTORY: usize = 16;
/// Shown before anything has been picked.
const FALLBACK: &str = "#6fd3c9";

#[derive(Default)]
pub struct Picker {
    pub slot_shown: bool,
    /// Newest first, "#rrggbb".
    pub history: Vec<String>,
    pub current: usize,
    /// `current` was picked this session (not chosen from history, nor picked before).
    pub fresh: bool,
    pub error: Option<String>,
    /// hyprpicker is running.
    picking: bool,
}

/// What's kept across restarts.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Saved {
    slot_shown: bool,
    history: Vec<String>,
}

impl Picker {
    pub fn new() -> Self {
        let saved = load();
        Self {
            slot_shown: saved.slot_shown,
            history: saved.history.into_iter().filter(|c| is_hex(c)).collect(),
            ..Self::default()
        }
    }

    /// The current colour, "#rrggbb".
    pub fn hex(&self) -> &str {
        self.history
            .get(self.current)
            .map_or(FALLBACK, String::as_str)
    }

    /// Runs hyprpicker, which copies the hex itself (`-a`) and prints it, or nothing when
    /// cancelled with escape.
    pub fn pick(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
        if self.picking {
            return;
        }
        self.picking = true;
        let run = async {
            // With -a, hyprpicker leaves a wl-copy serving the clipboard, which inherits its
            // stderr; tokio's `output()` always pipes stderr, which would then not reach EOF
            // until something else took the clipboard. So only stdout is read.
            let mut child = tokio::process::Command::new("hyprpicker")
                .args(["-a", "-f", "hex"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            let mut out = String::new();
            if let Some(mut stdout) = child.stdout.take() {
                stdout.read_to_string(&mut out).await?;
            }
            child.wait().await?;
            std::io::Result::Ok(out)
        };
        runtime::then(cx, run, |this, output, cx| {
            this.picking = false;
            match output {
                Ok(out) => {
                    if let Some(hex) = find_hex(&out) {
                        this.record(hex);
                    }
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    this.error = Some("hyprpicker isn't installed".into());
                }
                Err(e) => this.error = Some(format!("hyprpicker: {e}")),
            }
            cx.notify();
        });
    }

    fn record(&mut self, hex: String) {
        self.history.retain(|c| *c != hex);
        self.history.insert(0, hex);
        self.history.truncate(HISTORY);
        self.current = 0;
        self.fresh = true;
        self.slot_shown = true;
        self.save();
    }

    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.current = index;
        self.fresh = false;
        cx.notify();
    }

    /// Empties the history but for the current colour.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.history = vec![self.hex().to_string()];
        self.current = 0;
        self.save();
        cx.notify();
    }

    /// Takes the slot out of the bar, until the next pick.
    pub fn remove(&mut self, cx: &mut Context<Self>) {
        self.slot_shown = false;
        self.save();
        cx.notify();
    }

    fn save(&self) {
        let Some(path) = path() else { return };
        let saved = Saved {
            slot_shown: self.slot_shown,
            history: self.history.clone(),
        };
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                std::fs::write(&path, serde_json::to_string(&saved).unwrap_or_default())
            });
        if let Err(e) = result {
            eprintln!("picker: saving {}: {e}", path.display());
        }
    }
}

/// Puts `text` on the clipboard.
pub fn copy(text: String) {
    runtime::detach(async move { super::spawn("wl-copy", &["--", &text]).await });
}

fn path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(state.join("shell/picker.json"))
}

/// Missing until the picker is first used.
fn load() -> Saved {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn is_hex(c: &str) -> bool {
    c.len() == 7
        && c.starts_with('#')
        && c[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The first "#rrggbb" in hyprpicker's output, lowercased.
fn find_hex(text: &str) -> Option<String> {
    text.match_indices('#').find_map(|(i, _)| {
        let hex = text.get(i..i + 7)?.to_ascii_lowercase();
        is_hex(&hex).then_some(hex)
    })
}

/// "#rrggbb" as a colour.
pub fn color(hex: &str) -> Rgba {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    gpui::rgb(value)
}

/// The colour in the formats offered for copying, as (name, value).
pub fn formats(hex: &str) -> [(&'static str, String); 4] {
    let c = color(hex);
    let (rn, gn, bn) = (c.r, c.g, c.b);
    let [r, g, b] = [rn, gn, bn].map(|x| (x * 255.).round() as u8);
    // JavaScript's Math.round, which the QML used: halves round up.
    let round = |x: f32| (x + 0.5).floor();
    let (mx, mn) = (rn.max(gn).max(bn), rn.min(gn).min(bn));
    let (l, d) = ((mx + mn) / 2., mx - mn);
    let (mut h, mut s) = (0., 0.);
    if d > 0. {
        s = d / (1. - (2. * l - 1.).abs());
        h = if mx == rn {
            ((gn - bn) / d) % 6.
        } else if mx == gn {
            (bn - rn) / d + 2.
        } else {
            (rn - gn) / d + 4.
        };
        h = round(h * 60.);
        if h < 0. {
            h += 360.;
        }
    }
    let pct = |x: f32| format!("{}%", round(x * 100.));
    [
        ("HEX", hex.to_uppercase()),
        ("RGB", format!("rgb({r}, {g}, {b})")),
        ("HSL", format!("hsl({h}, {}, {})", pct(s), pct(l))),
        ("QML", format!("Qt.rgba({rn:.3}, {gn:.3}, {bn:.3}, 1)")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_hex() {
        assert_eq!(find_hex("#FF00aa\n").as_deref(), Some("#ff00aa"));
        assert_eq!(find_hex("x # #12345g #abcdef").as_deref(), Some("#abcdef"));
        assert_eq!(find_hex(""), None);
    }

    #[test]
    fn hsl() {
        assert_eq!(formats("#ff0000")[2].1, "hsl(0, 100%, 50%)");
        assert_eq!(formats("#6fd3c9")[2].1, "hsl(174, 53%, 63%)");
        assert_eq!(formats("#808080")[2].1, "hsl(0, 0%, 50%)");
    }
}
