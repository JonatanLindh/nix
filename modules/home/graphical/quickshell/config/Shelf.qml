pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// The drag shelf: files parked for dragging into other apps, plus tools that work on them.
// The `shelf` daemon (../shelf, org.jonatanlindh.Shelf) owns the items and runs the tools;
// this is its UI, talking to it through its CLI, which prints JSON. `shelf watch` keeps
// `items` in sync, so `shelf add` from anywhere shows up here. Tool results are the
// daemon's own files (~/.local/state/shelf/files), put on the shelf as new items and
// deleted once they leave it; keeping one means dragging it out. Selection, which
// monitor it's on and whether it's shown stay here.
Singleton {
    id: root

    // The daemon's items, [{ path, name, kind, temp }]; `temp` marks results, which the
    // daemon deletes when they leave the shelf.
    property var items: []
    readonly property int count: items.length
    // Where items that share a name live, by path: just enough of their folder to tell
    // them apart ("Downloads", "a/src", "~"). Results are told apart by "Temp" already.
    readonly property var places: {
        const home = Quickshell.env("HOME");
        const groups = {};
        for (const i of items)
            if (!i.temp)
                groups[i.name] = (groups[i.name] ?? []).concat([i.path]);
        const out = {};
        for (const paths of Object.values(groups)) {
            if (paths.length < 2)
                continue;
            const dirs = paths.map(p => p.split("/").slice(0, -1).filter(s => s !== ""));
            const tail = (d, k) => d.slice(-k).join("/");
            paths.forEach((p, n) => {
                const d = dirs[n];
                // The fewest trailing folders no other item's path ends in the same way.
                let k = 1;
                while (k < d.length && dirs.some((o, m) => m !== n && tail(o, k) === tail(d, k)))
                    k++;
                const dir = "/" + d.join("/");
                out[p] = dir === home ? "~" : k >= d.length ? dir : tail(d, k);
            });
        }
        return out;
    }
    property bool shown: false
    // The monitor the shelf shows on ("" = the focused one at the time).
    property string screenName: ""

    // Paths of the selected items; tools work on these, or on everything when none are.
    property var selected: []
    readonly property var targets: selected.length ? items.filter(i => selected.includes(i.path)) : items

    // Tools for the targets ([{ id, label, icon, options? }]), from `shelf actions`.
    property var actions: []
    // Which inline panel is open under the tools: an action id with options, "taildrop",
    // "more", "qr", or "".
    property string open: ""
    property var peers: []
    // The link QR's { url, size, path }, from the daemon; shown while `open` is "qr".
    property var linkQr: null
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

    function say(text, error) {
        status = text;
        statusError = !!error;
    }

    // Adds files (paths) and shows the shelf. The items come back through the watcher.
    function add(paths) {
        paths = paths.filter(p => p);
        if (paths.length)
            call(["add"].concat(paths));
        if (!shown)
            showOnFocused();
    }

    function remove(path) {
        call(["remove", path]);
    }

    function clear() {
        open = "";
        say("");
        call(["clear"]);
    }

    // A quick daemon call whose only output that matters is a failure.
    function call(args) {
        if (!installed) {
            say("The shelf needs a rebuild first", true);
            return;
        }
        caller.createObject(root, {
            command: [bin].concat(args)
        });
    }

    // From the daemon: the new item list, and why it changed.
    function setItems(list, reason) {
        items = list;
        selected = selected.filter(p => list.some(i => i.path === p));
        // Added from outside the bar (`shelf add`, a Taildrop arriving): show it, like a
        // drop would.
        if ((reason === "add" || reason === "taildrop") && !shown)
            showOnFocused();
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

    function refreshActions() {
        const paths = targets.map(i => i.kind === "folder" ? i.path + "/" : i.path);
        if (paths.length === 0) {
            actions = [];
            return;
        }
        if (!installed) {
            actions = [];
            return;
        }
        actionsQuery.running = false;
        actionsQuery.command = [bin, "actions"].concat(paths);
        actionsQuery.running = true;
    }

    // A tool tile was clicked.
    function activate(action) {
        // The QR is a card under the tools: the tile folds it away again.
        if (action.id === "qr" && open === "qr") {
            open = "";
            return;
        }
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
            say("The shelf needs a rebuild first", true);
            return;
        }
        const paths = targets.map(i => i.path);
        busy = busy.concat([id]);
        say(({
                funnel: "Starting a funnel link…",
                wormhole: "Getting a wormhole code…",
                shrink: "Re-encoding, this can take a while…",
                gif: "Making a GIF…",
                ocr: "Reading text…",
                hand: "Handing to the shelf…"
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
        if (r.qr) {
            linkQr = r.qr;
            open = "qr";
        }
        if (r.job)
            setJob(r.job);
        // The daemon has put the results on the shelf (or moved the files onto it); select
        // them, so the next tool works on them.
        const pick = r.outputs?.length ? r.outputs : r.select ?? [];
        if (pick.length)
            selected = pick.slice();
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
                    root.actions = [];
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

    // The items (first all of them, then on every change) and share updates from the
    // daemon, one JSON object per line.
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
                if (Array.isArray(e.items)) {
                    root.setItems(e.items, e.reason);
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

    Component {
        id: caller

        Process {
            id: callProc

            running: true
            stdout: StdioCollector {
                onStreamFinished: {
                    try {
                        const r = JSON.parse(text);
                        if (!r.ok)
                            root.say(r.message ?? "The shelf daemon failed", true);
                    } catch (e) {}
                    callProc.destroy();
                }
            }
        }
    }

    // ---- Persistence: which monitor the shelf was on (the items live in the daemon); the
    // shelf itself starts hidden.

    function save() {
        store.setText(JSON.stringify({
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
                root.screenName = JSON.parse(text())?.screenName ?? "";
            } catch (e) {}
            root.restored = true;
        }
        onLoadFailed: root.restored = true
    }

    // `qs ipc call shelf toggle`. Adding files is `shelf add`.
    IpcHandler {
        target: "shelf"

        function toggle(): void {
            root.toggle();
        }
    }
}
