//! Tailscale (Ts.qml, and the `qs-ts` helper it ran, folded in): the state read from tailscaled's
//! LocalAPI, the unix socket the `tailscale` CLI talks to, and changes made through the CLI,
//! which knows how to edit serve configs, start funnels and so on.
//!
//! The state is read again whenever the IPN bus says something happened, every 30 s besides
//! (peers going on- or offline don't always make it onto the bus), and after each command.
//! Changing anything needs `--operator=<user>` (set in common.nix).

use std::sync::LazyLock;
use std::time::Duration;

use futures::SinkExt;
use futures::channel::mpsc::Sender;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{Mutex, Notify, mpsc};

const SOCKETS: [&str; 2] = [
    "/run/tailscale/tailscaled.sock",
    "/var/run/tailscale/tailscaled.sock",
];
/// ipn.NotifyInitialState | NotifyNoPrivateKeys | NotifyRateLimit
const BUS_MASK: u32 = 2 | 16 | 256;
const POLL: Duration = Duration::from_secs(30);
/// `tailscale up` waits for a login that may never come.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// The only ports funnel allows.
pub const FUNNEL_PORTS: [&str; 3] = ["443", "8443", "10000"];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// tailscaled is running.
    pub available: bool,
    /// BackendState: Running, Stopped, NeedsLogin, Starting, ...
    pub backend: String,
    /// This device.
    pub me: Option<Node>,
    /// Online first, then by name.
    pub peers: Vec<Peer>,
    /// By port, then path.
    pub serves: Vec<Serve>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub name: String,
    /// MagicDNS name, without the trailing dot.
    pub dns: String,
    pub ip: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub node: Node,
    pub online: bool,
    /// It offers to be an exit node.
    pub exit_option: bool,
    /// It is our exit node.
    pub exit_active: bool,
}

/// Something served on the tailnet, or funnelled to the internet.
#[derive(Debug, Clone, PartialEq)]
pub struct Serve {
    pub port: String,
    pub path: String,
    /// "http://127.0.0.1:3000", a directory, "text", ...
    pub target: String,
    /// Public, through funnel.
    pub funnel: bool,
    pub url: String,
}

impl State {
    pub fn up(&self) -> bool {
        self.backend == "Running"
    }

    pub fn needs_login(&self) -> bool {
        self.backend == "NeedsLogin"
    }

    pub fn exit_node(&self) -> Option<&Peer> {
        self.peers.iter().find(|p| p.exit_active)
    }

    pub fn exit_options(&self) -> impl Iterator<Item = &Peer> {
        self.peers.iter().filter(|p| p.exit_option)
    }

    /// Something is reachable from the public internet.
    pub fn funnel_active(&self) -> bool {
        self.serves.iter().any(|s| s.funnel)
    }

    /// "On · exit node home-server", "Needs login", "Off", ...
    pub fn summary(&self) -> String {
        if self.needs_login() {
            "Needs login".into()
        } else if !self.up() {
            "Off".into()
        } else if let Some(node) = self.exit_node() {
            format!("Exit node · {}", node.node.name)
        } else if self.funnel_active() {
            "On · funnel public".into()
        } else {
            "On".into()
        }
    }

    /// Why there's nothing to show while it isn't up.
    pub fn down_reason(&self) -> String {
        match self.backend.as_str() {
            _ if !self.available => "Tailscale isn't running".into(),
            "NeedsLogin" => "Needs login: run `tailscale up`".into(),
            "Stopped" | "" => "Tailscale is stopped".into(),
            other => format!("Tailscale is {}", other.to_lowercase()),
        }
    }
}

/// Asks for a fresh read (a command finished, a panel opened).
static POKE: Notify = Notify::const_new();
/// Commands run one at a time, in order.
static COMMANDS: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// The state, each time it changes.
pub fn watch() -> futures::channel::mpsc::Receiver<State> {
    crate::runtime::stream(4, run)
}

async fn run(mut out: Sender<State>) {
    let (poke, mut poked) = mpsc::unbounded_channel();
    let bus = tokio::spawn(bus(poke));
    let mut last = None;
    let mut poll = tokio::time::interval(POLL);
    loop {
        tokio::select! {
            _ = poll.tick() => {}
            () = POKE.notified() => {}
            Some(()) = poked.recv() => {
                // Let a burst of notifications settle into one read.
                tokio::time::sleep(Duration::from_millis(150)).await;
                while poked.try_recv().is_ok() {}
            }
        }
        let state = fetch().await;
        if last.as_ref() != Some(&state) {
            last = Some(state.clone());
            if out.send(state).await.is_err() {
                break;
            }
        }
    }
    bus.abort();
}

/// A fresh read, soon.
pub fn refresh() {
    POKE.notify_one();
}

// ---- LocalAPI

async fn connect() -> std::io::Result<UnixStream> {
    let mut error = None;
    for path in SOCKETS {
        match UnixStream::connect(path).await {
            Ok(s) => return Ok(s),
            Err(e) => error = Some(e),
        }
    }
    Err(error.expect("a socket was tried"))
}

/// Starts a LocalAPI GET. HTTP/1.0, so the body is never chunked: it ends with the connection.
async fn request(path: &str) -> std::io::Result<UnixStream> {
    let mut s = connect().await?;
    let head = format!("GET /localapi/v0/{path} HTTP/1.0\r\nHost: local-tailscaled.sock\r\n\r\n");
    s.write_all(head.as_bytes()).await?;
    Ok(s)
}

async fn get(path: &str) -> Option<Value> {
    let mut raw = Vec::new();
    request(path).await.ok()?.read_to_end(&mut raw).await.ok()?;
    body(&raw)
}

/// The JSON body of a 200 response; an empty one is an empty object.
fn body(raw: &[u8]) -> Option<Value> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let status = String::from_utf8_lossy(&raw[..end]);
    if !status.lines().next()?.contains(" 200 ") {
        return None;
    }
    let body = &raw[end + 4..];
    if body.iter().all(u8::is_ascii_whitespace) {
        return Some(Value::Object(Default::default()));
    }
    serde_json::from_slice(body).ok()
}

/// Pokes on every notification from the IPN bus. What changed is read back from /status, so the
/// notifications themselves (which can carry the whole netmap) aren't parsed.
async fn bus(poke: mpsc::UnboundedSender<()>) {
    loop {
        if let Ok(mut s) = request(&format!("watch-ipn-bus?mask={BUS_MASK}")).await {
            let mut buf = [0; 16 * 1024];
            while let Ok(1..) = s.read(&mut buf).await {
                if poke.send(()).is_err() {
                    return;
                }
            }
        }
        // tailscaled went away (or wasn't there): show that, then try again.
        if poke.send(()).is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn fetch() -> State {
    let Some(status) = get("status").await else {
        return State::default();
    };
    let serves = get("serve-config")
        .await
        .map(|c| serves(&c))
        .unwrap_or_default();
    parse(&status, serves)
}

// ---- Parsing

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn parse(status: &Value, serves: Vec<Serve>) -> State {
    State {
        available: true,
        backend: text(&status["BackendState"]).to_string(),
        me: status.get("Self").filter(|s| s.is_object()).map(node),
        peers: peers(status),
        serves,
    }
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
        .map(|p| Peer {
            node: node(p),
            online: p["Online"].as_bool().unwrap_or(false),
            exit_option: p["ExitNodeOption"].as_bool().unwrap_or(false),
            exit_active: p["ExitNode"].as_bool().unwrap_or(false),
        })
        .filter(|p| !p.node.name.starts_with("funnel-ingress-node"))
        .collect();
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.node.name.to_lowercase().cmp(&b.node.name.to_lowercase()))
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

/// "http://127.0.0.1:3000" → ":3000"; anything else as it is.
pub fn short_target(target: &str) -> &str {
    ["http://", "https://"]
        .iter()
        .filter_map(|scheme| target.strip_prefix(scheme))
        .flat_map(|rest| ["127.0.0.1", "localhost"].map(|host| rest.strip_prefix(host)))
        .flatten()
        .find(|rest| rest.starts_with(':'))
        .unwrap_or(target)
}

// ---- Commands, as `tailscale` arguments

pub fn up_args(up: bool) -> Vec<String> {
    vec![if up { "up" } else { "down" }.into()]
}

/// Routes everything through `ip`, or nothing with `None`.
pub fn exit_node_args(ip: Option<&str>) -> Vec<String> {
    vec!["set".into(), format!("--exit-node={}", ip.unwrap_or(""))]
}

fn path_args(path: &str) -> Option<String> {
    (!path.is_empty() && path != "/").then(|| format!("--set-path={path}"))
}

/// Serves `target` ("3000", "localhost:3000", a file path, ...) on `port` at `path`, publicly
/// through funnel or on the tailnet only. Also flips an existing entry between the two.
pub fn serve_args(port: &str, target: &str, public: bool, path: &str) -> Vec<String> {
    let mut args: Vec<String> = vec![
        if public { "funnel" } else { "serve" }.into(),
        "--bg".into(),
        "--yes".into(),
        format!("--https={port}"),
    ];
    args.extend(path_args(path));
    args.push(target.into());
    args
}

pub fn remove_args(serve: &Serve) -> Vec<String> {
    let mut args: Vec<String> = vec![
        if serve.funnel { "funnel" } else { "serve" }.into(),
        "--yes".into(),
        format!("--https={}", serve.port),
    ];
    args.extend(path_args(&serve.path));
    args.push("off".into());
    args
}

/// Runs `tailscale <args>` after any command before it, then reads the state again. The error is
/// the CLI's last line of output.
pub async fn run_command(args: Vec<String>) -> Result<(), String> {
    let result = {
        let _turn = COMMANDS.lock().await;
        command(&args).await
    };
    refresh();
    result.inspect_err(|e| eprintln!("tailscale {}: {e}", args.join(" ")))
}

async fn command(args: &[String]) -> Result<(), String> {
    let child = tokio::process::Command::new("tailscale")
        .args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let name = args.first().map_or("", String::as_str);
    let out = tokio::time::timeout(COMMAND_TIMEOUT, child)
        .await
        .map_err(|_| format!("tailscale {name} timed out"))?
        .map_err(|e| format!("couldn't run tailscale: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    // Tailscale prints informational messages on stderr too; only say something on failure, and
    // then its last line.
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Err(last_line(&text).unwrap_or_else(|| format!("tailscale {name} failed")))
}

fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_status() {
        let status = json!({
            "BackendState": "Running",
            "Self": {"DNSName": "desktop.tail.ts.net.", "HostName": "desktop", "TailscaleIPs": ["100.1.1.1"]},
            "Peer": {
                "a": {"DNSName": "zed.tail.ts.net.", "TailscaleIPs": ["100.1.1.2"], "Online": false},
                "b": {"DNSName": "Box.tail.ts.net.", "TailscaleIPs": ["100.1.1.3"], "Online": true,
                      "ExitNodeOption": true, "ExitNode": true},
                "c": {"DNSName": "", "HostName": "alpha", "TailscaleIPs": ["100.1.1.4"], "Online": true},
                "d": {"DNSName": "funnel-ingress-node-1.ts.net.", "Online": true},
            }
        });
        let state = parse(&status, Vec::new());
        assert!(state.up());
        assert_eq!(state.me.as_ref().unwrap().name, "desktop");
        assert_eq!(state.me.as_ref().unwrap().dns, "desktop.tail.ts.net");
        let names: Vec<_> = state.peers.iter().map(|p| p.node.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Box", "zed"]);
        assert_eq!(state.exit_node().unwrap().node.ip, "100.1.1.3");
        assert_eq!(state.summary(), "Exit node · Box");
    }

    #[test]
    fn parses_serves() {
        let config = json!({
            "Web": {
                "desktop.ts.net:8443": {"Handlers": {"/api": {"Proxy": "http://127.0.0.1:3000"}}},
                "desktop.ts.net:443": {"Handlers": {"/": {"Path": "/srv"}, "/hi": {"Text": "hi"}}},
            },
            "AllowFunnel": {"desktop.ts.net:443": true}
        });
        let serves = serves(&config);
        let urls: Vec<_> = serves.iter().map(|s| s.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://desktop.ts.net",
                "https://desktop.ts.net/hi",
                "https://desktop.ts.net:8443/api"
            ]
        );
        assert_eq!(serves[0].target, "/srv");
        assert_eq!(serves[1].target, "text");
        assert!(serves[0].funnel && !serves[2].funnel);
        let state = State {
            backend: "Running".into(),
            serves,
            ..State::default()
        };
        assert!(state.funnel_active());
        assert_eq!(state.summary(), "On · funnel public");
    }

    #[test]
    fn reads_responses() {
        assert_eq!(
            body(b"HTTP/1.0 200 OK\r\nX: y\r\n\r\n{\"a\":1}"),
            Some(json!({"a": 1}))
        );
        assert_eq!(body(b"HTTP/1.0 200 OK\r\n\r\n\n"), Some(json!({})));
        assert_eq!(body(b"HTTP/1.0 403 Forbidden\r\n\r\n{}"), None);
    }

    #[test]
    fn shortens_targets() {
        assert_eq!(short_target("http://127.0.0.1:3000"), ":3000");
        assert_eq!(short_target("https://localhost:8080"), ":8080");
        assert_eq!(
            short_target("http://example.com:80"),
            "http://example.com:80"
        );
        assert_eq!(short_target("/srv/www"), "/srv/www");
    }

    #[test]
    fn builds_commands() {
        assert_eq!(
            serve_args("443", "3000", true, "/"),
            ["funnel", "--bg", "--yes", "--https=443", "3000"]
        );
        assert_eq!(
            serve_args("8443", "3000", false, "/api"),
            [
                "serve",
                "--bg",
                "--yes",
                "--https=8443",
                "--set-path=/api",
                "3000"
            ]
        );
        let serve = Serve {
            port: "443".into(),
            path: "/x".into(),
            target: "t".into(),
            funnel: true,
            url: String::new(),
        };
        assert_eq!(
            remove_args(&serve),
            ["funnel", "--yes", "--https=443", "--set-path=/x", "off"]
        );
        assert_eq!(exit_node_args(None), ["set", "--exit-node="]);
    }

    #[test]
    fn explains_down() {
        let mut state = State::default();
        assert_eq!(state.down_reason(), "Tailscale isn't running");
        state.available = true;
        state.backend = "NeedsLogin".into();
        assert_eq!(state.summary(), "Needs login");
        state.backend = "Starting".into();
        assert_eq!(state.down_reason(), "Tailscale is starting");
        assert_eq!(last_line("a\n  b  \n\n"), Some("b".into()));
    }
}
