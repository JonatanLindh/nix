pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Tailscale through its CLI. Changing anything needs `--operator=<user>` (set in common.nix).
Singleton {
    id: root

    property bool available: false
    // BackendState: Running, Stopped, NeedsLogin, Starting, ...
    property string backend: ""
    readonly property bool up: backend === "Running"
    property var self: null
    property var peers: []
    readonly property var exitNode: peers.find(p => p.exitActive) ?? null
    readonly property var exitOptions: peers.filter(p => p.exitOption)
    // [{ port, path, target, funnel, url }]
    property var serves: []
    readonly property bool funnelActive: serves.some(s => s.funnel)
    property string error: ""
    // Poll faster while the panel is open.
    property bool watching: false

    function refresh() {
        statusQuery.running = true;
        serveQuery.running = true;
    }

    // Commands run one at a time, then state is re-read.
    property var queue: []

    function run(args) {
        queue = queue.concat([args]);
        next();
    }

    function next() {
        if (command.running || queue.length === 0)
            return;
        // Tailscale also prints informational messages on stderr; only surface output on failure.
        command.command = ["sh", "-c", 'out=$("$@" 2>&1) || { printf "%s" "$out" >&2; exit 1; }', "sh", "tailscale"].concat(queue[0]);
        queue = queue.slice(1);
        command.running = true;
    }

    function setUp(on) {
        run([on ? "up" : "down"]);
    }

    function setExitNode(peer) {
        run(["set", "--exit-node=" + (peer?.ip ?? "")]);
    }

    function pathArgs(path) {
        return path && path !== "/" ? ["--set-path=" + path] : [];
    }

    // target: "3000", "localhost:3000", "http://127.0.0.1:3000", a file path, ...
    function addServe(port, target, publicly, path) {
        run([publicly ? "funnel" : "serve", "--bg", "--yes", "--https=" + port].concat(pathArgs(path), [target]));
    }

    function setPublic(entry, publicly) {
        run([publicly ? "funnel" : "serve", "--bg", "--yes", "--https=" + entry.port].concat(pathArgs(entry.path), [entry.target]));
    }

    function removeServe(entry) {
        run([entry.funnel ? "funnel" : "serve", "--yes", "--https=" + entry.port].concat(pathArgs(entry.path), ["off"]));
    }

    Process {
        id: command
        stderr: StdioCollector {
            onStreamFinished: root.error = text.trim().split("\n").filter(l => l.trim()).pop() ?? ""
        }
        onRunningChanged: {
            if (running)
                return;
            root.refresh();
            root.next();
        }
    }

    Process {
        id: statusQuery
        command: ["tailscale", "status", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                let d;
                try {
                    d = JSON.parse(text);
                } catch (e) {
                    root.available = false;
                    return;
                }
                const strip = s => (s ?? "").replace(/\.$/, "");
                // The DNS label is the unique machine name; HostName can be generic.
                const name = p => strip(p.DNSName).split(".")[0] || p.HostName;
                root.available = true;
                root.backend = d.BackendState ?? "";
                root.self = d.Self ? {
                    name: name(d.Self),
                    dns: strip(d.Self.DNSName),
                    ip: d.Self.TailscaleIPs?.[0] ?? ""
                } : null;
                root.peers = Object.values(d.Peer ?? {}).map(p => ({
                            name: name(p),
                            dns: strip(p.DNSName),
                            ip: p.TailscaleIPs?.[0] ?? "",
                            os: p.OS ?? "",
                            online: !!p.Online,
                            exitOption: !!p.ExitNodeOption,
                            exitActive: !!p.ExitNode
                        })).filter(p => !p.name.startsWith("funnel-ingress-node")).sort((a, b) => (b.online - a.online) || a.name.localeCompare(b.name));
            }
        }
    }

    Process {
        id: serveQuery
        command: ["tailscale", "serve", "status", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                let d;
                try {
                    d = JSON.parse(text || "{}");
                } catch (e) {
                    return;
                }
                const funnel = d.AllowFunnel ?? {};
                const out = [];
                for (const [hostPort, web] of Object.entries(d.Web ?? {})) {
                    const port = hostPort.split(":").pop();
                    for (const [path, h] of Object.entries(web.Handlers ?? {})) {
                        out.push({
                            port: port,
                            path: path,
                            target: h.Proxy ?? h.Path ?? (h.Text !== undefined ? "text" : "?"),
                            funnel: !!funnel[hostPort],
                            url: "https://" + hostPort.replace(/:443$/, "") + (path === "/" ? "" : path)
                        });
                    }
                }
                root.serves = out.sort((a, b) => a.port - b.port || a.path.localeCompare(b.path));
            }
        }
    }

    Timer {
        interval: root.watching ? 2000 : 10000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }
}
