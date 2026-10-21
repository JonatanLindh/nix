pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Tailscale through qs-ts (helpers/): it pushes the state whenever tailscaled reports a change,
// and runs changes through the CLI one at a time. Changing anything needs `--operator=<user>`
// (set in common.nix).
Singleton {
    id: root

    property bool available: false
    // BackendState: Running, Stopped, NeedsLogin, Starting, ...
    property string backend: ""
    readonly property bool up: backend === "Running"
    // { name, dns, ip }
    property var self: null
    // [{ name, dns, ip, os, online, exitOption, exitActive }], online first
    property var peers: []
    readonly property var exitNode: peers.find(p => p.exitActive) ?? null
    readonly property var exitOptions: peers.filter(p => p.exitOption)
    // [{ port, path, target, funnel, url }]
    property var serves: []
    readonly property bool funnelActive: serves.some(s => s.funnel)
    property string error: ""

    // State is pushed, so this is only for a fresh look (when the panel opens).
    function refresh() {
        send({
            op: "refresh"
        });
    }

    function run(args) {
        send({
            op: "run",
            args: args
        });
    }

    function send(request) {
        if (watcher.running)
            watcher.write(JSON.stringify(request) + "\n");
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
        id: watcher
        command: ["qs-ts", "watch"]
        running: true
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => {
                let d;
                try {
                    d = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (d.error !== undefined) {
                    root.error = d.error;
                    return;
                }
                root.available = d.available;
                root.backend = d.backend;
                root.self = d.self;
                root.peers = d.peers;
                root.serves = d.serves;
            }
        }
        // It only stops if it couldn't start (not installed yet) or crashed; try again later.
        onRunningChanged: if (!running) {
            root.available = false;
            restart.start();
        }
    }

    Timer {
        id: restart
        interval: 10000
        onTriggered: watcher.running = true
    }
}
