//! EasyEffects, through its CLI (it has no D-Bus API). Careful: with no instance running, the
//! CLI starts a whole new EasyEffects instead of failing, so every call first checks that the
//! service is active and its socket exists.

/// What the sound panel shows. Read when the panel opens and after each change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// Effects are bypassed (sound passes through untouched).
    pub bypass: bool,
    pub presets: Vec<String>,
    /// The output preset last loaded.
    pub preset: String,
}

/// Runs the CLI against the running instance only; `None` when it isn't running.
async fn cli(args: &[&str]) -> Option<String> {
    let guarded = r#"systemctl --user is-active --quiet easyeffects.service && [ -S "$XDG_RUNTIME_DIR/EasyEffectsServer" ] && exec easyeffects "$@""#;
    let output = tokio::process::Command::new("sh")
        .args(["-c", guarded, "sh"])
        .args(args)
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn read() -> Option<State> {
    let (bypass, presets, last) = tokio::join!(cli(&["-b", "3"]), cli(&["-p"]), cli(&["-s"]));
    // "Output presets:\n1\tName\n...\n\nInput presets..."
    let presets = presets
        .as_deref()
        .and_then(|p| p.split("Output presets:").nth(1))
        .and_then(|p| p.split("\n\n").next())
        .map(|p| {
            p.lines()
                .map(|l| {
                    l.trim()
                        .split_once('\t')
                        .map_or(l.trim(), |(_, name)| name)
                        .trim()
                })
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let preset = last
        .as_deref()
        .and_then(|s| s.lines().find_map(|l| l.trim().strip_prefix("output:")))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Some(State {
        bypass: bypass?.trim() == "1",
        presets,
        preset,
    })
}

/// Bypasses effects (or stops bypassing), then reads the result.
pub async fn set_bypass(bypass: bool) -> Option<State> {
    // -b: 1 enables bypass, 2 disables it.
    cli(&["-b", if bypass { "1" } else { "2" }]).await;
    read().await
}

pub async fn load(preset: String) -> Option<State> {
    cli(&["-l", &preset]).await;
    read().await
}

/// The last state read, kept for the life of the shell so the sound panel opens with it instead
/// of starting blank (EasyEffects off, no presets) and jumping once a read comes back. Read at
/// startup, again each time the panel opens, and after each change made here.
pub struct EasyEffects {
    /// `None` while EasyEffects isn't running (or hasn't been read yet).
    pub state: Option<State>,
}

impl EasyEffects {
    pub fn new(cx: &mut gpui::App) -> gpui::Entity<Self> {
        use gpui::AppContext;
        cx.new(|cx| {
            let mut this = Self { state: None };
            this.refresh(cx);
            this
        })
    }

    /// Reads the state again; what's shown meanwhile is the last one.
    pub fn refresh(&mut self, cx: &mut gpui::Context<Self>) {
        crate::runtime::then(cx, read(), Self::apply);
    }

    /// Shows the change right away; the read after it confirms it.
    pub fn set_bypass(&mut self, bypass: bool, cx: &mut gpui::Context<Self>) {
        if let Some(state) = &mut self.state {
            state.bypass = bypass;
            cx.notify();
        }
        crate::runtime::then(cx, set_bypass(bypass), Self::apply);
    }

    pub fn load(&mut self, preset: String, cx: &mut gpui::Context<Self>) {
        if let Some(state) = &mut self.state {
            state.preset.clone_from(&preset);
            cx.notify();
        }
        crate::runtime::then(cx, load(preset), Self::apply);
    }

    fn apply(&mut self, state: Option<State>, cx: &mut gpui::Context<Self>) {
        if self.state != state {
            self.state = state;
            cx.notify();
        }
    }
}
