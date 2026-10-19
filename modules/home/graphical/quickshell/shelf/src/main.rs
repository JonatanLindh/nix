//! `shelf`: the drag shelf's backend. `shelf daemon` serves org.jonatanlindh.Shelf on the
//! session bus and does the work; the other subcommands are a thin CLI for Quickshell that
//! print JSON on stdout.

mod actions;
mod daemon;
mod files;
mod http;
mod jobs;
mod peers;
mod qr;
mod tools;

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
    /// Print share updates as JSON lines until killed.
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
        Command::Run { action, opt, files } => {
            // The daemon has its own working directory.
            let files = files
                .iter()
                .map(|f| std::path::absolute(f).map(|p| p.to_string_lossy().into_owned()))
                .collect::<Result<Vec<_>, _>>()?;
            let proxy = daemon::proxy().await?;
            println!("{}", proxy.run(&action, &opt, files).await?);
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
            let mut changes = proxy.receive_job_changed().await?;
            while let Some(signal) = changes.next().await {
                let args = signal.args()?;
                println!(
                    "{}",
                    json!({ "job": args.job, "state": args.state, "message": args.message })
                );
            }
            Ok(())
        }
    }
}
