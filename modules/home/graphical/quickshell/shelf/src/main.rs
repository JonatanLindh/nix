//! `shelf`: the drag shelf's backend. `shelf daemon` serves org.jonatanlindh.Shelf on the
//! session bus, holds the shelf's items and does the work; the other subcommands are a thin
//! CLI (for Quickshell, scripts and keybinds) that print JSON on stdout.

mod actions;
mod clip;
mod daemon;
mod files;
mod hand;
mod http;
mod jobs;
mod peers;
mod qr;
mod store;
mod taildrop;
mod tools;
mod watch;

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use serde_json::json;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve org.jonatanlindh.Shelf on the session bus.
    Daemon,
    /// Put FILES on the shelf.
    Add {
        #[arg(required = true)]
        files: Vec<String>,
    },
    /// Take FILES off the shelf.
    Remove {
        #[arg(required = true)]
        files: Vec<String>,
    },
    /// Put the clipboard on the shelf: copied files as they are; a web address, text or
    /// an image as a new temp file. With --text, that text instead.
    Paste {
        #[arg(long)]
        text: Option<String>,
    },
    /// Empty the shelf.
    Clear,
    /// Print the shelf's items as JSON.
    List,
    /// Print the actions that apply to all of FILES, as JSON.
    Actions { files: Vec<String> },
    /// Run ACTION on FILES (through the daemon) and print the result as JSON.
    Run {
        action: String,
        /// The chosen option, e.g. "png" for convert or a peer for taildrop.
        #[arg(long, default_value = "")]
        opt: String,
        files: Vec<String>,
    },
    /// Stop a running share (funnel link or wormhole).
    Cancel { job: String },
    /// Print the running shares as JSON.
    Jobs,
    /// Print Taildrop targets (your own tailnet devices) as JSON.
    Peers,
    /// Print the items, then item and share updates, as JSON lines until killed.
    Watch,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match run(cli.command).await {
        Ok(()) => 0,
        Err(e) => {
            let mut message = format!("{e:#}");
            if message.contains("ServiceUnknown") || message.contains("NameHasNoOwner") {
                message = "The shelf daemon isn't running".into();
            }
            // Same shape as a failed action, so the UI has one error path.
            println!("{}", json!({ "ok": false, "message": message }));
            1
        }
    };
    std::process::exit(code);
}

async fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Daemon => daemon::serve().await,
        Command::Actions { files } => {
            println!("{}", serde_json::to_string(&actions::for_files(&files))?);
            Ok(())
        }
        Command::Peers => {
            println!("{}", serde_json::to_string(&peers::list().await?)?);
            Ok(())
        }
        Command::Add { files } => {
            daemon::proxy().await?.add(absolute(&files)?).await?;
            println!("{}", json!({ "ok": true }));
            Ok(())
        }
        Command::Remove { files } => {
            daemon::proxy().await?.remove(absolute(&files)?).await?;
            println!("{}", json!({ "ok": true }));
            Ok(())
        }
        Command::Paste { text } => paste(text).await,
        Command::Clear => {
            daemon::proxy().await?.clear().await?;
            println!("{}", json!({ "ok": true }));
            Ok(())
        }
        Command::List => {
            println!("{}", daemon::proxy().await?.items().await?);
            Ok(())
        }
        Command::Run { action, opt, files } => {
            let proxy = daemon::proxy().await?;
            println!("{}", proxy.run(&action, &opt, absolute(&files)?).await?);
            Ok(())
        }
        Command::Cancel { job } => {
            daemon::proxy().await?.cancel(&job).await?;
            println!("{}", json!({ "ok": true }));
            Ok(())
        }
        Command::Jobs => {
            println!("{}", daemon::proxy().await?.jobs().await?);
            Ok(())
        }
        Command::Watch => {
            let proxy = daemon::proxy().await?;
            // Subscribed before reading the items, so no change falls in between.
            let mut jobs = proxy.receive_job_changed().await?;
            let mut items = proxy.receive_items_changed().await?;
            let current: serde_json::Value = serde_json::from_str(&proxy.items().await?)?;
            println!("{}", json!({ "items": current, "reason": "load" }));
            loop {
                tokio::select! {
                    Some(signal) = jobs.next() => {
                        let args = signal.args()?;
                        println!(
                            "{}",
                            json!({ "job": args.job, "state": args.state, "message": args.message })
                        );
                    }
                    Some(signal) = items.next() => {
                        let args = signal.args()?;
                        let list: serde_json::Value = serde_json::from_str(args.items)?;
                        println!("{}", json!({ "items": list, "reason": args.reason }));
                    }
                    else => return Ok(()),
                }
            }
        }
    }
}

/// Reads the clipboard here, where the Wayland session is; the daemon saves it.
async fn paste(text: Option<String>) -> anyhow::Result<()> {
    let proxy = daemon::proxy().await?;
    let (mime, data) = match text {
        Some(text) => ("text/plain".to_string(), text.into_bytes()),
        None => {
            let listed = tools::output("wl-paste", ["--list-types"])
                .await
                .map_err(|_| anyhow::anyhow!("The clipboard is empty"))?;
            let types: Vec<&str> = listed.lines().map(str::trim).collect();
            if types.contains(&"text/uri-list") {
                let list = tools::output("wl-paste", ["--no-newline", "--type", "text/uri-list"])
                    .await?;
                if let Some(paths) = clip::file_paths(&list) {
                    let n = paths.len();
                    proxy.add(paths).await?;
                    let message = format!("Added {}", files::plural(n, "file"));
                    println!("{}", json!({ "ok": true, "message": message }));
                    return Ok(());
                }
            }
            let mime = clip::pick_type(&types)
                .ok_or_else(|| anyhow::anyhow!("Nothing on the clipboard the shelf can hold"))?;
            let out = tools::exec("wl-paste", ["--no-newline", "--type", &mime]).await?;
            (mime, out.stdout)
        }
    };
    println!("{}", proxy.paste(&mime, data).await?);
    Ok(())
}

/// The daemon has its own working directory, so paths go to it absolute.
fn absolute(files: &[String]) -> anyhow::Result<Vec<String>> {
    Ok(files
        .iter()
        .map(|f| std::path::absolute(f).map(|p| p.to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?)
}
