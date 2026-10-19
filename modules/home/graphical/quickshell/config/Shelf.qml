pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// The drag shelf: files parked for dragging into other apps, plus tools that work on them.
// The tools run in the `shelf` daemon (../shelf, org.jonatanlindh.Shelf) through its CLI,
// which prints JSON; its results land in ~/Downloads/Shelf and come back here as new
// items. Without the daemon the shelf still holds and drags files.
Singleton {
    id: root

    // [{ path, name, kind, fresh }]; `fresh` marks results ("New").
    property var items: []
    readonly property int count: items.length
    property bool shown: false
    // The monitor the shelf shows on ("" = the focused one at the time).
    property string screenName: ""

    // Paths of the selected items; tools work on these, or on everything when none are.
    property var selected: []
    readonly property var targets: selected.length ? items.filter(i => selected.includes(i.path)) : items

    // Tools for the targets ([{ id, label, icon, options? }]), from `shelf actions`.
    property var actions: []
    // Which inline panel is open under the tools: an action id with options, "taildrop",
    // "more", or "".
    property string open: ""
    property var peers: []
    property bool peersLoading: false

    // Running shares (from the daemon): the funnel link and the wormhole send, or null.
    property var funnel: null
    property var wormhole: null
    property string wormholeState: "waiting"
    // Cards can be folded away while their share keeps running.
    property bool funnelCard: true
    property bool wormholeCard: true

    property string status: ""
    property bool statusError: false
    // Action ids running now.
    property var busy: []

    // Whether the `shelf` CLI is installed (it arrives with a rebuild).
    property bool installed: false
    readonly property string bin: "shelf"

    function kindOf(path) {
        const name = path.split("/").pop().toLowerCase();
        if (path.endsWith("/"))
            return "folder";
        if (/\.(mp4|mkv|webm|mov|avi|m4v)$/.test(name))
            return "video";
        if (/\.(png|jpe?g|gif|webp|avif|heic|heif|tiff?|bmp|svg)$/.test(name))
            return "image";
        if (/\.(zip|tar|gz|tgz|xz|txz|zst|bz2|7z|rar|iso)$/.test(name))
            return "archive";
        if (/\.(nix|lua|qml|js|ts|py|rs|go|c|h|cpp|sh|json|toml|ya?ml|html|css)$/.test(name))
            return "code";
        return "file";
    }

    function item(path, fresh) {
        const clean = path.replace(/\/+$/, "") || "/";
        return {
            path: clean,
            name: clean.split("/").pop(),
            kind: kindOf(path),
            fresh: !!fresh
        };
    }

    function say(text, error) {
        status = text;
        statusError = !!error;
    }

    // Adds files (paths) and shows the shelf. Dirs get their kind fixed up by a stat.
    function add(paths) {
        const fresh = paths.filter(p => p && !items.some(i => i.path === p.replace(/\/+$/, "")));
        if (fresh.length) {
            items = items.concat(fresh.map(p => item(p, false)));
            detectFolders(fresh);
            save();
        }
        if (!shown)
            showOnFocused();
    }

    function remove(path) {
        items = items.filter(i => i.path !== path);
        selected = selected.filter(p => p !== path);
        save();
    }

    function clear() {
        items = [];
        selected = [];
        open = "";
        say("");
        save();
    }

    function toggleSelected(path) {
        selected = selected.includes(path) ? selected.filter(p => p !== path) : selected.concat([path]);
        open = "";
    }

    function showOnFocused() {
        screenName = Hyprland.focusedMonitor?.name ?? "";
        shown = true;
    }

    function toggle(screen) {
        if (shown && (screen === undefined || screen === screenName)) {
            shown = false;
            return;
        }
        screenName = screen ?? Hyprland.focusedMonitor?.name ?? "";
        shown = true;
    }

    function fileUrl(path) {
        return "file://" + path.split("/").map(encodeURIComponent).join("/");
    }

    function uriList(paths) {
        return paths.map(fileUrl).join("\r\n") + "\r\n";
    }

    function copy(text) {
        Quickshell.execDetached(["wl-copy", "--", text]);
    }

    // ---- Tools

    // Shown when the daemon isn't installed: what works without it.
    function fallbackActions(n) {
        const out = [];
        if (n >= 2)
            out.push({
                id: "archive",
                label: "Archive",
                icon: "archive"
            });
        return out.concat([
            {
                id: "funnel",
                label: "Funnel link",
                icon: "link"
            },
            {
                id: "wormhole",
                label: "Wormhole",
                icon: "wormhole"
            },
            {
                id: "localsend",
                label: "LocalSend",
                icon: "send"
            },
            {
                id: "taildrop",
                label: "Taildrop",
                icon: "tailscale"
            },
            {
                id: "copy-paths",
                label: n === 1 ? "Copy path" : "Copy paths",
                icon: "copy"
            }
        ]);
    }

    function refreshActions() {
        const paths = targets.map(i => i.kind === "folder" ? i.path + "/" : i.path);
        if (paths.length === 0) {
            actions = [];
            return;
        }
        if (!installed) {
            actions = fallbackActions(paths.length);
            return;
        }
        actionsQuery.running = false;
        actionsQuery.command = [bin, "actions"].concat(paths);
        actionsQuery.running = true;
    }

    // A tool tile was clicked.
    function activate(action) {
        if (action.id === "more" || action.options || action.id === "taildrop") {
            open = open === action.id ? "" : action.id;
            if (open === "taildrop")
                loadPeers();
            return;
        }
        open = "";
        if (action.id === "copy-paths") {
            const paths = targets.map(i => i.path);
            copy(paths.join("\n"));
            say(paths.length === 1 ? "Copied path" : `Copied ${paths.length} paths`);
            return;
        }
        // A share that's running: the tile shows/hides its card instead of starting another.
        if (action.id === "funnel" && funnel) {
            funnelCard = !funnelCard;
            return;
        }
        if (action.id === "wormhole" && wormhole) {
            wormholeCard = !wormholeCard;
            return;
        }
        run(action.id, "");
    }

    function run(id, opt) {
        open = "";
        if (!installed) {
            say("Shelf tools need a rebuild first", true);
            return;
        }
        const paths = targets.map(i => i.path);
        busy = busy.concat([id]);
        say(({
                funnel: "Starting a funnel link…",
                wormhole: "Getting a wormhole code…",
                shrink: "Re-encoding, this can take a while…",
                gif: "Making a GIF…",
                ocr: "Reading text…"
            })[id] ?? "Working…");
        runner.createObject(root, {
            action: id,
            command: [bin, "run", id, "--opt", opt].concat(paths)
        });
    }

    function finished(id, text) {
        busy = busy.filter(b => b !== id);
        let r;
        try {
            r = JSON.parse(text);
        } catch (e) {
            say(text.trim() || "The shelf tool failed", true);
            return;
        }
        // A share's card says what's going on; the status line stays quiet.
        say(r.job ? "" : r.message ?? "", !r.ok);
        if (r.copy)
            copy(r.copy);
        if (r.job)
            setJob(r.job);
        if (r.outputs?.length) {
            // Results go on top, marked new and selected, so the next tool works on them.
            const kept = items.filter(i => !r.outputs.includes(i.path));
            items = r.outputs.map(p => item(p, true)).concat(kept);
            detectFolders(r.outputs);
            selected = r.outputs.slice();
            save();
        }
    }

    function setJob(job) {
        if (job.kind === "funnel") {
            funnel = job;
            funnelCard = true;
        } else if (job.kind === "wormhole") {
            wormhole = job;
            wormholeState = "waiting";
            wormholeCard = true;
        }
    }

    function cancel(job) {
        if (!job)
            return;
        Quickshell.execDetached([bin, "cancel", job.id]);
    }

    function loadPeers() {
        if (!installed) {
            peers = [];
            return;
        }
        peersLoading = true;
        peersQuery.running = true;
    }

    onTargetsChanged: refreshActions()
    // Once the saved state is in, so restoring it doesn't write it straight back.
    property bool restored: false
    onShownChanged: if (restored)
        save()
    onInstalledChanged: {
        refreshActions();
        if (installed) {
            watcher.running = true;
            jobsQuery.running = true;
        }
    }

    // ---- Daemon I/O

    Process {
        command: ["sh", "-c", 'command -v "$1"', "sh", root.bin]
        running: true
        onExited: code => root.installed = code === 0
    }

    Process {
        id: actionsQuery
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const list = JSON.parse(text);
                    if (Array.isArray(list))
                        root.actions = list;
                } catch (e) {
                    root.actions = root.fallbackActions(root.targets.length);
                }
            }
        }
    }

    Process {
        id: peersQuery
        command: [root.bin, "peers"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.peersLoading = false;
                try {
                    const list = JSON.parse(text);
                    root.peers = Array.isArray(list) ? list : [];
                    if (!Array.isArray(list))
                        root.say(list.message ?? "Can't list devices", true);
                } catch (e) {
                    root.peers = [];
                }
            }
        }
    }

    // Shares that were running before a reload.
    Process {
        id: jobsQuery
        command: [root.bin, "jobs"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const list = JSON.parse(text);
                    if (Array.isArray(list))
                        list.forEach(j => root.setJob(j));
                } catch (e) {}
            }
        }
    }

    // Share updates from the daemon, one JSON object per line.
    Process {
        id: watcher
        command: [root.bin, "watch"]
        stdout: SplitParser {
            onRead: line => {
                let e;
                try {
                    e = JSON.parse(line);
                } catch (err) {
                    return;
                }
                if (root.wormhole?.id === e.job) {
                    if (e.state === "sending") {
                        root.wormholeState = "sending";
                        return;
                    }
                    root.wormhole = null;
                } else if (root.funnel?.id === e.job) {
                    root.funnel = null;
                } else {
                    return;
                }
                root.say(e.message, e.state === "failed");
            }
        }
        // It only ends if the bus connection drops; reconnect.
        onRunningChanged: if (!running && root.installed)
            rewatch.restart()
    }

    Timer {
        id: rewatch
        interval: 5000
        onTriggered: watcher.running = true
    }

    Component {
        id: runner

        Process {
            id: proc

            property string action
            property string out: ""
            // Output and exit can arrive in either order; finish once both have.
            property int pending: 2

            function step() {
                if (--pending > 0)
                    return;
                root.finished(action, out);
                destroy();
            }

            running: true
            stdout: StdioCollector {
                onStreamFinished: {
                    proc.out = text;
                    proc.step();
                }
            }
            onExited: step()
        }
    }

    // Marks dropped/added directories as folders (their paths carry no trailing slash).
    function detectFolders(paths) {
        folderProbe.createObject(root, {
            command: ["sh", "-c", 'for p; do [ -d "$p" ] && printf "%s\\n" "$p"; done; true', "sh"].concat(paths)
        });
    }

    Component {
        id: folderProbe

        Process {
            id: probe

            running: true
            stdout: StdioCollector {
                onStreamFinished: {
                    const dirs = text.split("\n").filter(l => l).map(p => p.replace(/\/+$/, ""));
                    if (dirs.length)
                        root.items = root.items.map(i => dirs.includes(i.path) ? Object.assign({}, i, {
                                kind: "folder"
                            }) : i);
                    probe.destroy();
                }
            }
        }
    }

    // ---- Persistence: the items survive reloads and restarts; the shelf itself starts hidden.

    function save() {
        store.setText(JSON.stringify({
            items: items,
            screenName: screenName
        }));
    }

    FileView {
        id: store

        path: Quickshell.statePath("shelf.json")
        // A missing file just means the shelf has never been used.
        printErrors: false
        onLoaded: {
            try {
                const state = JSON.parse(text());
                const saved = state?.items ?? [];
                root.screenName = state?.screenName ?? "";
                root.items = saved.filter(i => i?.path).map(i => Object.assign(root.item(i.path, i.fresh), {
                        kind: i.kind ?? root.kindOf(i.path)
                    }));
            } catch (e) {
                root.items = [];
            }
            root.restored = true;
        }
        onLoadFailed: root.restored = true
    }

    // `qs ipc call shelf add /path/to/file`, `qs ipc call shelf toggle`.
    IpcHandler {
        target: "shelf"

        function add(path: string): void {
            root.add([path]);
        }

        function toggle(): void {
            root.toggle();
        }
    }
}
