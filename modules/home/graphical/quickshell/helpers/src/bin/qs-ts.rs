//! `qs-ts watch`: Tailscale for the bar, as JSON lines on stdout.
//!
//! State is read from tailscaled's LocalAPI (the unix socket the `tailscale` CLI talks to):
//! again whenever the IPN bus says something happened, and every 30 s besides, as peers going
//! on- or offline don't always make it onto the bus. A line goes out only when it changed:
//!
//!   {"available": true, "backend": "Running", "self": {...}, "peers": [...], "serves": [...]}
//!
//! Changes go through the `tailscale` CLI (which knows how to edit serve configs, start funnels
//! and so on), one command at a time, from JSON lines on stdin:
//!
//!   {"op": "run", "args": ["set", "--exit-node=100.101.102.103"]}
//!   {"op": "refresh"}
//!
//! Each command answers with {"error": "..."}, empty when it worked. `qs-ts status` prints the
//! state once, for debugging.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{self, UnboundedSender};

const SOCKETS: [&str; 2] = [
    "/run/tailscale/tailscaled.sock",
    "/var/run/tailscale/tailscaled.sock",
];
// ipn.NotifyInitialState | NotifyNoPrivateKeys | NotifyRateLimit
const BUS_MASK: u32 = 2 | 16 | 256;
const POLL: Duration = Duration::from_secs(30);
// `tailscale up` waits for a login that may never come.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("watch") => watch().await,
        Some("status") => {
            emit(&fetch().await.to_string());
            Ok(())
        }
        _ => {
            eprintln!("usage: qs-ts watch | status");
            std::process::exit(2);
        }
    }
}

async fn watch() -> Result<()> {
    let (poke, mut poked) = mpsc::unbounded_channel();
    let (commands, queue) = mpsc::unbounded_channel();
    tokio::spawn(bus(poke.clone()));
    tokio::spawn(run_commands(queue, poke.clone()));
    tokio::spawn(read_stdin(commands, poke));

    let mut last = String::new();
    let mut poll = tokio::time::interval(POLL);
    loop {
        tokio::select! {
            _ = poll.tick() => {}
            Some(()) = poked.recv() => {
                // Let a burst of notifications settle into one read.
                tokio::time::sleep(Duration::from_millis(150)).await;
                while poked.try_recv().is_ok() {}
            }
        }
        let line = fetch().await.to_string();
        if line != last {
            emit(&line);
            last = line;
        }
    }
}

/// One line out; once nobody reads them any more there is nothing left to do.
fn emit(line: &str) {
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        std::process::exit(0);
    }
}

// ---- LocalAPI

async fn connect() -> Result<UnixStream> {
    let mut error = None;
    for path in SOCKETS {
        match UnixStream::connect(path).await {
            Ok(s) => return Ok(s),
            Err(e) => error = Some(e),
        }
    }
    Err(error.unwrap()).context("tailscaled isn't running")
}

/// Starts a LocalAPI GET. HTTP/1.0, so the body is never chunked: it ends with the connection.
async fn request(path: &str) -> Result<UnixStream> {
    let mut s = connect().await?;
    let head = format!("GET /localapi/v0/{path} HTTP/1.0\r\nHost: local-tailscaled.sock\r\n\r\n");
    s.write_all(head.as_bytes()).await?;
    Ok(s)
}

async fn get(path: &str) -> Result<Value> {
    let mut raw = Vec::new();
    request(path).await?.read_to_end(&mut raw).await?;
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .context("bad LocalAPI response")?;
    let head = String::from_utf8_lossy(&raw[..end]);
    let status = head.lines().next().unwrap_or("");
    if !status.contains(" 200 ") {
        bail!("LocalAPI {path}: {status}");
    }
    let body = &raw[end + 4..];
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(json!({}));
    }
    Ok(serde_json::from_slice(body)?)
}

/// Pokes on every notification from the IPN bus. What changed is read back from /status, so
/// the notifications themselves (which can carry the whole netmap) aren't parsed.
async fn bus(poke: UnboundedSender<()>) {
    loop {
        if let Ok(mut s) = request(&format!("watch-ipn-bus?mask={BUS_MASK}")).await {
            let mut buf = [0; 16 * 1024];
            while let Ok(1..) = s.read(&mut buf).await {
                let _ = poke.send(());
            }
        }
        // tailscaled went away (or wasn't there): show that, then try again.
        let _ = poke.send(());
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

// ---- State

#[derive(Serialize)]
struct Node {
    name: String,
    dns: String,
    ip: String,
}

#[derive(Serialize)]
struct Peer {
    name: String,
    dns: String,
    ip: String,
    os: String,
    online: bool,
    #[serde(rename = "exitOption")]
    exit_option: bool,
    #[serde(rename = "exitActive")]
    exit_active: bool,
}

#[derive(Serialize)]
struct Serve {
    port: String,
    path: String,
    target: String,
    funnel: bool,
    url: String,
}

async fn fetch() -> Value {
    let Ok(status) = get("status").await else {
        return json!({ "available": false, "backend": "", "self": null, "peers": [], "serves": [] });
    };
    let serves = get("serve-config")
        .await
        .map(|c| serves(&c))
        .unwrap_or_default();
    json!({
        "available": true,
        "backend": status["BackendState"].as_str().unwrap_or(""),
        "self": status.get("Self").filter(|s| s.is_object()).map(node),
        "peers": peers(&status),
        "serves": serves,
    })
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn node(p: &Value) -> Node {
    let dns = text(&p["DNSName"]).trim_end_matches('.').to_string();
    // The DNS label is the unique machine name; HostName can be generic.
    let name = match dns.split('.').next() {
        Some(label) if !label.is_empty() => label.to_string(),
        _ => text(&p["HostName"]).to_string(),
    };
    Node {
        name,
        dns,
        ip: text(&p["TailscaleIPs"][0]).to_string(),
    }
}

fn peers(status: &Value) -> Vec<Peer> {
    let mut peers: Vec<Peer> = status["Peer"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .map(|p| {
            let Node { name, dns, ip } = node(p);
            Peer {
                name,
                dns,
                ip,
                os: text(&p["OS"]).to_string(),
                online: p["Online"].as_bool().unwrap_or(false),
                exit_option: p["ExitNodeOption"].as_bool().unwrap_or(false),
                exit_active: p["ExitNode"].as_bool().unwrap_or(false),
            }
        })
        .filter(|p| !p.name.starts_with("funnel-ingress-node"))
        .collect();
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    peers
}

fn serves(config: &Value) -> Vec<Serve> {
    let funnel = &config["AllowFunnel"];
    let mut out = Vec::new();
    for (host_port, web) in config["Web"].as_object().into_iter().flatten() {
        let port = host_port.rsplit(':').next().unwrap_or("").to_string();
        let host = host_port.strip_suffix(":443").unwrap_or(host_port);
        for (path, h) in web["Handlers"].as_object().into_iter().flatten() {
            let target = h["Proxy"]
                .as_str()
                .or(h["Path"].as_str())
                .unwrap_or(if h.get("Text").is_some() { "text" } else { "?" });
            out.push(Serve {
                port: port.clone(),
                path: path.clone(),
                target: target.to_string(),
                funnel: funnel[host_port].as_bool().unwrap_or(false),
                url: format!("https://{host}{}", if path == "/" { "" } else { path }),
            });
        }
    }
    let num = |s: &Serve| s.port.parse::<u32>().unwrap_or(0);
    out.sort_by(|a, b| num(a).cmp(&num(b)).then_with(|| a.path.cmp(&b.path)));
    out
}

// ---- Commands

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Request {
    Run { args: Vec<String> },
    Refresh,
}

async fn read_stdin(commands: UnboundedSender<Vec<String>>, poke: UnboundedSender<()>) {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str(&line) {
            Ok(Request::Run { args }) if !args.is_empty() => {
                let _ = commands.send(args);
            }
            Ok(Request::Run { .. }) => {}
            Ok(Request::Refresh) => {
                let _ = poke.send(());
            }
            Err(e) => eprintln!("qs-ts: bad request {line:?}: {e}"),
        }
    }
    // The bar went away.
    std::process::exit(0);
}

async fn run_commands(mut queue: mpsc::UnboundedReceiver<Vec<String>>, poke: UnboundedSender<()>) {
    while let Some(args) = queue.recv().await {
        let error = match run(&args).await {
            Ok(()) => String::new(),
            Err(e) => format!("{e:#}"),
        };
        emit(&json!({ "error": error }).to_string());
        let _ = poke.send(());
    }
}

async fn run(args: &[String]) -> Result<()> {
    let child = tokio::process::Command::new("tailscale")
        .args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(COMMAND_TIMEOUT, child)
        .await
        .with_context(|| format!("tailscale {} timed out", args[0]))?
        .context("couldn't run tailscale")?;
    if out.status.success() {
        return Ok(());
    }
    // Tailscale prints informational messages on stderr too; only say something on failure,
    // and then its last line.
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    match text.lines().map(str::trim).rfind(|l| !l.is_empty()) {
        Some(line) => bail!("{line}"),
        None => bail!("tailscale {} failed", args[0]),
    }
}
