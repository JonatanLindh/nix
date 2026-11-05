//! A Wayland shell for Hyprland, in gpui: a bar on every display, its panels as popups, the
//! session menu, notifications and the drag shelf, all on layer-shell surfaces.

// GPUI Kit is the facade over GPUI, gpui-base and gpui-component. Named `gpui`, it's where GPUI's
// macro output (`gpui::IntoElement`, ...) resolves, and it reads as plain GPUI everywhere else.
extern crate gpui_kit as gpui;

mod bar;
mod launcher;
mod notifications;
mod osd;
mod panel;
mod runtime;
mod screenshot;
mod services;
mod session;
mod shelf;
mod shell;
mod theme;
mod ui;
mod wayland;

use gpui::component::{Theme, ThemeMode};

/// A desktop shell for Hyprland: a bar on every display, panels, launcher, notifications and the
/// drag shelf. Without a subcommand it runs the shell.
#[derive(clap::Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Tell the running shell COMMAND, e.g. from a Hyprland bind: launcher, clip, op, session,
    /// notifications, shelf, paste, picker, dnd, screenshot, screenshot-window,
    /// screenshot-screen, bluetooth, power, tailscale, or the media keys: play-pause, next,
    /// previous (to the player the now-playing card shows).
    Msg { command: String },
}

fn main() {
    let cli = <Cli as clap::Parser>::parse();
    if let Some(Command::Msg { command }) = cli.command {
        if let Err(e) = services::ipc::send(&command) {
            eprintln!("shell isn't running: {e}");
            std::process::exit(1);
        }
        return;
    }
    if services::ipc::running() {
        eprintln!("shell is already running");
        std::process::exit(1);
    }

    // gpui and the services log through `log`; `RUST_LOG=info` shows more than errors.
    env_logger::init();
    stop_on_signals();

    gpui::application().with_assets(ui::icon::Assets).run(|cx| {
        gpui::init(cx);
        // The text field's colours: light text on the dark glass.
        Theme::change(ThemeMode::Dark, None, cx);
        // gpui-component paints every window root in the theme background; ours draw their own glass.
        Theme::global_mut(cx).tokens.background = gpui::transparent_black().into();
        services::Services::init(cx);
        shell::Shell::init(cx);
        launcher::init(cx);
    });
}

/// On SIGTERM (systemd stopping the shell) or SIGINT: stop the shelf's shares first, so no
/// Funnel link stays public with nothing serving or stopping it, then exit.
fn stop_on_signals() {
    runtime::detach(async {
        use tokio::signal::unix::{SignalKind, signal};
        let (Ok(mut term), Ok(mut int)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) else {
            return eprintln!("can't listen for signals: shares won't be stopped on exit");
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
        services::shelf::shutdown().await;
        std::process::exit(0);
    });
}
