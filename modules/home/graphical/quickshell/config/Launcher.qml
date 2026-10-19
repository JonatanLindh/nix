pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Widgets

// The launcher, opened with super: apps, and through a prefix the other modes, shown as a
// chip in front of the search field (backspace on an empty field drops it again):
//   >cmd   run a shell command          =1+2   calculator (enter copies the result)
//   ~      files in home                d~     files in ~/Downloads, newest first
//   /      files anywhere               "      clipboard history (also super+v)
//   *      1Password (also super+shift+p)
// A layer window of its own so it takes the keyboard straight away. Apps start through
// uwsm, each in its own scope. Files open, show in Files, copy their path or go to the shelf
// (the same modifiers on enter and click), and can be dragged out into other apps.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: ["launcher", "clip", "op"].includes(bar.panel)

    // "apps" | "run" | "calc" | "files" | "clip" | "pass"; files also have a scope.
    property string mode: "apps"
    property string scope: "home"
    readonly property string query: search.text
    // Selected row of the current mode's list.
    property int sel: 0
    // A file is being dragged out: the card fades and lets the pointer through.
    property bool dragging: false

    readonly property string home: Quickshell.env("HOME")

    // ---- Apps

    // Visible desktop entries, alphabetical, with lowercased fields for matching.
    readonly property var apps: DesktopEntries.applications.values.filter(e => !e.noDisplay).map(e => ({
                entry: e,
                id: e.id,
                name: e.name,
                icon: e.icon,
                lname: e.name.toLowerCase(),
                words: e.name.toLowerCase().split(/[\s\-_.]+/),
                hay: [e.genericName, e.id].concat(e.keywords).join(" ").toLowerCase(),
                detail: e.genericName || e.comment
            })).sort((a, b) => a.lname.localeCompare(b.lname)).map((a, i) => Object.assign(a, {
            order: i
        }))

    // Launches per desktop id: { id: { n: count, t: last launch in ms } }, kept in
    // the state dir.
    property var launches: ({})

    readonly property bool showRecent: mode === "apps" && query.trim() === "" && recent.length > 0
    readonly property var recent: apps.filter(a => launches[a.id]).sort((x, y) => frecency(y.id) - frecency(x.id) || x.order - y.order).slice(0, 6)

    // ---- What the current mode shows

    readonly property var shownApps: mode === "apps" ? rank(query) : []
    readonly property var calcResult: mode === "calc" ? calc(query) : null
    readonly property var shownClips: mode === "clip" ? clip.filter(query) : []
    readonly property var shownPass: mode === "pass" ? (pw.item ? pw.matchFields(query) : pw.rank(query)) : []
    // File results for `fileKey` (see the search below).
    property var files: []
    property int filesTotal: 0

    readonly property var rows: mode === "apps" ? shownApps : mode === "files" ? files : mode === "clip" ? shownClips : mode === "pass" ? shownPass : []
    readonly property var current: rows.length > 0 ? rows[Math.min(sel, rows.length - 1)] : null

    // Frequency, weighted towards recent use.
    function frecency(id) {
        const u = launches[id];
        if (!u)
            return 0;
        const days = (Date.now() - u.t) / 86400000;
        return u.n * (days < 1 ? 4 : days < 7 ? 2 : days < 30 ? 1 : 0.5);
    }

    // Apps rank by name prefix > word prefix > substring (name, generic name, keywords,
    // id) > subsequence of the name; ties go to the more used, then alphabetical.
    function rank(text) {
        const q = text.trim().toLowerCase();
        const scored = [];
        for (const a of apps) {
            let tier = -1;
            if (!q)
                tier = 0;
            else if (a.lname.startsWith(q))
                tier = 0;
            else if (a.words.some(w => w.startsWith(q)))
                tier = 1;
            else if (a.lname.includes(q) || a.hay.includes(q))
                tier = 2;
            else if (subsequence(q, a.lname))
                tier = 3;
            if (tier >= 0)
                scored.push({
                    tier: tier,
                    use: frecency(a.id),
                    app: a
                });
        }
        // The JS sort here isn't stable, so alphabetical order is the last key.
        scored.sort((x, y) => x.tier - y.tier || y.use - x.use || x.app.order - y.app.order);
        return scored.map(s => s.app);
    }

    function subsequence(q, s) {
        let i = 0;
        for (let j = 0; j < s.length && i < q.length; j++)
            if (s[j] === q[i])
                i++;
        return i === q.length;
    }

    // Arithmetic only: digits, operators, parens, dots. "^" is a power.
    function calc(expr) {
        const e = expr.trim();
        if (!e)
            return {
                ok: false,
                text: "Type an expression"
            };
        if (/^[\d\s+\-*/%^().]+$/.test(e)) {
            try {
                const r = Function(`"use strict"; return (${e.replace(/\^/g, "**")});`)();
                if (typeof r === "number" && isFinite(r))
                    return {
                        ok: true,
                        text: String(parseFloat(r.toPrecision(12)))
                    };
            } catch (err) {}
        }
        return {
            ok: false,
            text: "Not an expression"
        };
    }

    function launchApp(app) {
        if (!app)
            return;
        close();
        record(app.id);
        Quickshell.execDetached(["uwsm-app", "--", app.id.endsWith(".desktop") ? app.id : app.id + ".desktop"]);
    }

    function record(id) {
        const l = Object.assign({}, launches);
        const u = l[id] ?? {
            n: 0,
            t: 0
        };
        l[id] = {
            n: u.n + 1,
            t: Date.now()
        };
        launches = l;
        store.setText(JSON.stringify(l));
    }

    // ---- Modes

    // The prefix characters that switch modes from the apps search.
    readonly property var prefixes: ({
            ">": "run",
            "=": "calc",
            "\"": "clip",
            "*": "pass",
            "~": "files",
            "/": "files"
        })

    readonly property string modeLabel: {
        if (mode === "files")
            return "Files · " + ({
                    home: "Home",
                    downloads: "Downloads",
                    root: "/"
                })[scope];
        if (mode === "pass" && pw.item)
            return "Passwords · Fields";
        return ({
                run: "Run",
                calc: "Calculate",
                clip: "Clipboard",
                pass: "Passwords"
            })[mode] ?? "";
    }

    function setMode(m, s) {
        mode = m;
        scope = s ?? "home";
        sel = 0;
        if (m === "clip")
            clip.load();
        else if (m === "pass")
            pw.refresh();
        else if (m === "files") {
            finder.key = "";
            finder.done = "";
            searchSoon();
        }
    }

    // A prefix typed (or pasted) at the start of the apps search becomes the mode chip.
    // Downloads is "d~"; after "~" everything typed is search text.
    function textEdited() {
        let t = search.text;
        if (mode === "apps" && t.startsWith("d~")) {
            setMode("files", "downloads");
            t = t.slice(2);
        } else if (mode === "apps" && prefixes[t.charAt(0)]) {
            const c = t.charAt(0);
            setMode(prefixes[c], c === "/" ? "root" : "home");
            t = t.slice(1);
        }
        // Setting it here doesn't come back through onTextEdited.
        if (t !== search.text)
            search.text = t;
        sel = 0;
        if (mode === "files")
            searchSoon();
    }

    function activate(item, modifiers) {
        const shift = modifiers & Qt.ShiftModifier;
        if (mode === "apps") {
            launchApp(item);
        } else if (mode === "run") {
            const cmd = query.trim();
            if (!cmd)
                return;
            close();
            Quickshell.execDetached(["uwsm-app", "--", "sh", "-c", cmd]);
        } else if (mode === "calc") {
            if (!calcResult?.ok)
                return;
            close();
            Quickshell.clipboardText = calcResult.text;
        } else if (mode === "files") {
            actFile(item, modifiers);
        } else if (mode === "clip") {
            if (!item)
                return;
            close();
            clip.pick(item, !shift);
        } else if (mode === "pass") {
            if (pw.item)
                takeField(item, !shift);
            else if (modifiers & Qt.ControlModifier)
                takePass(item, "username", !shift);
            else if (modifiers & Qt.AltModifier)
                takePass(item, "otp", !shift);
            else
                takePass(item, "password", !shift);
        }
    }

    function takePass(item, what, paste) {
        if (!item)
            return;
        close();
        pw.take(item, what, paste);
    }

    function takeField(field, paste) {
        if (!field)
            return;
        const target = pw.item;
        close();
        pw.take(target, field.type === "OTP" ? "otp" : field.id, paste);
    }

    // The item list's search and selection, put back when leaving an item's fields.
    property string pwListQuery: ""
    property int pwListSel: 0

    function showFields(item) {
        pwListQuery = search.text;
        pwListSel = sel;
        pw.showFields(item);
        search.text = "";
        sel = 0;
    }

    function back() {
        pw.back();
        search.text = pwListQuery;
        sel = pwListSel;
    }

    function close() {
        bar.panel = "";
    }

    // ---- Files

    // fd when it's installed (fast, skips .gitignored trees), find otherwise.
    property bool hasFd: true
    // What the file list should show, as JSON [scope, query]; `finder.key` is what the
    // running (or last) search was for.
    property string fileKey: ""
    readonly property bool filesBusy: mode === "files" && (fileKey !== finder.done || debounce.running)
    readonly property int fileLimit: 50

    readonly property string filesHeading: {
        if (mode !== "files")
            return "";
        const q = query.trim() !== "";
        if (scope === "downloads")
            return q ? "In Downloads" : "Downloads · newest first";
        if (scope === "root")
            return q ? "Everywhere" : "Type to search the whole system";
        return q ? "In Home" : "Recent files";
    }

    function searchSoon() {
        debounce.restart();
    }

    function runSearch() {
        if (!open || mode !== "files")
            return;
        fileKey = JSON.stringify([scope, query.trim()]);
        if (finder.running) {
            // Stopped; it starts again from onRunningChanged.
            if (finder.key !== fileKey)
                finder.running = false;
            return;
        }
        if (finder.key === fileKey)
            return;
        const cmd = fileCommand(scope, query.trim());
        finder.key = fileKey;
        if (cmd.length === 0) {
            files = [];
            filesTotal = 0;
            finder.done = fileKey;
            return;
        }
        finder.command = cmd;
        finder.running = true;
    }

    // A command printing "mtime\ttype\tpath\0" per file.
    function fileCommand(scope, q) {
        const stat = 'xargs -0 -r stat --printf "%Y\\t%F\\t%n\\0" -- 2>/dev/null';
        const base = scope === "downloads" ? home + "/Downloads" : scope === "root" ? "/" : home;
        const words = q.split(/\s+/).filter(w => w);
        if (words.length === 0) {
            if (scope === "downloads")
                return ["sh", "-c", 'find "$1" -mindepth 1 -maxdepth 1 ! -name ".*" -print0 | ' + stat, "sh", base];
            if (scope === "home")
                // Recently used files (GTK's list, which most apps add to), most recent first.
                return ["sh", "-c", 'f="$HOME/.local/share/recently-used.xbel"; [ -r "$f" ] || exit 0; ' + 'sed -n \'s/.*<bookmark href="file:\\/\\/\\([^"]*\\)".* modified="\\([^"]*\\)".*/\\2\t\\1/p\' "$f" | sort -r | cut -f2 | ' + 'sed "s/&amp;/\\&/g" | awk \'!seen[$0]++\' | head -n 120 | ' + 'while IFS= read -r p; do printf "%b\\0" "$(printf %s "$p" | sed "s/%/\\\\x/g")"; done | ' + stat];
            return [];
        }
        // A "/" in the query matches against the whole path instead of the name.
        const full = words.some(w => w.includes("/"));
        // Skipped when searching from /: huge, virtual, or both.
        const skip = ["/nix/store", "/proc", "/sys", "/dev", "/run"];
        if (hasFd) {
            const args = ["--color=never", "--fixed-strings", "--print0", "--max-results", "400"];
            if (full)
                args.push("--full-path");
            if (scope === "root")
                for (const s of skip)
                    args.push("--exclude", s);
            for (const w of words.slice(1))
                args.push("--and=" + w);
            args.push("--", words[0], base);
            return ["sh", "-c", 'fd "$@" | ' + stat, "sh"].concat(args);
        }
        const glob = w => "*" + w.replace(/[*?[\]\\]/g, "\\$&") + "*";
        const args = [base, "-mindepth", "1", "(", "-name", ".*"];
        if (scope === "root")
            for (const s of skip)
                args.push("-o", "-path", s);
        args.push(")", "-prune", "-o");
        for (const w of words)
            args.push(full ? "-ipath" : "-iname", glob(w));
        args.push("-print0");
        return ["sh", "-c", 'find "$@" 2>/dev/null | head -z -n 400 | ' + stat, "sh"].concat(args);
    }

    // Name prefix > name has every word > path match; then shallower, then newer. Listings
    // without a query keep their own order (newest first, or most recently used).
    function parseFiles(text, key) {
        const [scope, q] = JSON.parse(key);
        const words = q.toLowerCase().split(/\s+/).filter(w => w);
        const out = [];
        for (const rec of text.split("\0")) {
            const parts = rec.split("\t");
            if (parts.length < 3)
                continue;
            const path = parts.slice(2).join("\t").replace(/(.)\/$/, "$1");
            const slash = path.lastIndexOf("/");
            const name = path.slice(slash + 1);
            const dir = path.slice(0, slash) || "/";
            const lname = name.toLowerCase();
            out.push({
                path: path,
                name: name,
                dir: dir === home ? "~" : dir.startsWith(home + "/") ? "~" + dir.slice(home.length) : dir,
                mtime: Number(parts[0]),
                kind: parts[1] === "directory" ? "folder" : fileKind(lname),
                tier: !words.length ? 0 : lname.startsWith(words[0]) ? 0 : words.every(w => lname.includes(w)) ? 1 : 2,
                depth: path.split("/").length
            });
        }
        if (words.length)
            out.sort((a, b) => a.tier - b.tier || a.depth - b.depth || b.mtime - a.mtime);
        else if (scope === "downloads")
            out.sort((a, b) => b.mtime - a.mtime);
        filesTotal = out.length;
        return out.slice(0, fileLimit);
    }

    function fileKind(name) {
        if (/\.(png|jpe?g|gif|webp|avif|heic|svg|bmp|tiff?|cr[23]|arw|nef|dng)$/.test(name))
            return "image";
        if (/\.(mp4|mkv|webm|mov|avi|m4v)$/.test(name))
            return "video";
        if (/\.(zip|tar|gz|tgz|xz|zst|bz2|7z|rar|iso)$/.test(name))
            return "archive";
        if (/\.(nix|lua|qml|js|mjs|ts|tsx|jsx|py|rs|go|c|h|cc|cpp|hpp|java|kt|sh|fish|zsh|json|toml|ya?ml|html|css|scss|sql|zig|hs|rb|php|lock)$/.test(name) || ["makefile", "justfile", "dockerfile"].includes(name))
            return "code";
        return "file";
    }

    readonly property var fileTints: ({
            folder: "#3f78b8",
            file: "#b0533f",
            image: "#8c5cb0",
            code: "#2f8f7f",
            archive: "#7a6a4a",
            video: "#b0753f"
        })

    function age(mtime) {
        const s = Date.now() / 1000 - mtime;
        if (s < 60)
            return "now";
        if (s < 3600)
            return Math.floor(s / 60) + " min";
        if (s < 86400)
            return Math.floor(s / 3600) + " h";
        if (s < 86400 * 7)
            return Math.floor(s / 86400) + " d";
        if (s < 86400 * 30)
            return Math.floor(s / 86400 / 7) + " w";
        if (s < 86400 * 365)
            return Math.floor(s / 86400 / 30) + " mo";
        return Math.floor(s / 86400 / 365) + " y";
    }

    function fileUrl(path) {
        return "file://" + path.split("/").map(encodeURIComponent).join("/");
    }

    function onShelf(path) {
        return Shelf.items.some(i => (i?.path ?? i) === path);
    }

    // No modifier opens, shift shows it in Files, ctrl copies the path, alt adds it to the
    // shelf. The same with enter and with a click.
    function actFile(f, modifiers) {
        if (!f)
            return;
        close();
        if (modifiers & Qt.AltModifier) {
            Shelf.add([f.path]);
        } else if (modifiers & Qt.ControlModifier) {
            Quickshell.clipboardText = f.path;
        } else if (modifiers & Qt.ShiftModifier) {
            // The file manager selects it in its folder; just the folder if none answers.
            Quickshell.execDetached(["sh", "-c", 'dbus-send --session --print-reply --dest=org.freedesktop.FileManager1 --type=method_call /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems "array:string:$1" string: >/dev/null 2>&1 || exec uwsm-app -- xdg-open "$2"', "sh", fileUrl(f.path), f.path.slice(0, f.path.lastIndexOf("/")) || "/"]);
        } else {
            Quickshell.execDetached(["uwsm-app", "--", "xdg-open", f.path]);
        }
    }

    // ---- Small helpers for the views

    // A stable colour per name for letter tiles.
    readonly property var palette: ["#3b6fd6", "#d9822b", "#2f9e8f", "#4a86c8", "#7a6a55", "#3d4b8f", "#3f8f5a", "#7b3f8c", "#8a3f78", "#2c9b55", "#2b4a6e", "#5865c9", "#3a6ea8", "#6d4aff", "#b0533f"]

    function tint(name) {
        let h = 0;
        for (const c of name ?? "")
            h = (h * 31 + c.charCodeAt(0)) | 0;
        return palette[Math.abs(h) % palette.length];
    }

    readonly property var hints: {
        const K = (keys, text) => ({
                keys: keys,
                text: text
            });
        if (mode === "files")
            return [K(["↵"], "Open"), K(["⇧", "↵"], "Show in Files"), K(["ctrl", "↵"], "Copy path"), K(["alt", "↵"], "Shelf")];
        if (mode === "clip")
            return [K(["↵"], "Paste"), K(["⇧", "↵"], "Copy"), K(["del"], "Remove")];
        if (mode === "pass")
            return pw.item ? [K(["↵"], "Paste"), K(["⇧", "↵"], "Copy"), K(["esc"], "Back")] : [K(["↵"], "Password"), K(["ctrl", "↵"], "Username"), K(["alt", "↵"], "Code"), K(["⇥"], "Fields")];
        if (mode === "run")
            return [K(["↵"], "Run"), K(["esc"], "Close")];
        if (mode === "calc")
            return [K(["↵"], "Copy result"), K(["esc"], "Close")];
        return [K(["↑", "↓"], "Navigate"), K(["~"], "Files"), K(["\""], "Clipboard"), K(["*"], "Passwords"), K([">"], "Run"), K(["="], "Calc")];
    }

    readonly property string status: {
        const n = rows.length;
        if (mode === "apps")
            return n + (n === 1 ? " app" : " apps");
        if (mode === "run")
            return "Shell command";
        if (mode === "calc")
            return "Calculator";
        if (mode === "files") {
            if (filesBusy && n === 0)
                return "Searching…";
            const count = filesTotal > n ? `${n} of ${filesTotal}${filesTotal >= 400 ? "+" : ""} files` : n + (n === 1 ? " file" : " files");
            return count + (scope === "root" ? " · skips /nix/store" : "") + (hasFd ? "" : " · install fd for faster search");
        }
        if (mode === "clip")
            return `${n} of ${clip.entries.length} entries`;
        if (pw.authorizing)
            return "Authorize in 1Password…";
        if (pw.loading)
            return "Loading…";
        if (pw.error)
            return pw.error;
        return pw.item ? `${n} fields · ${pw.item.title}` : n + (n === 1 ? " item" : " items") + " · 1Password";
    }

    onSelChanged: Qt.callLater(followSelection)
    onCurrentChanged: {
        if (mode === "clip")
            clip.preview(current);
        else if (mode === "pass" && !pw.item)
            detailDelay.restart();
    }

    function followSelection() {
        const list = mode === "apps" ? appsList : mode === "files" ? filesList : mode === "clip" ? clipList : mode === "pass" ? passList : null;
        if (list && sel < list.count)
            list.positionViewAtIndex(sel, ListView.Contain);
    }

    function move(by) {
        if (rows.length > 0)
            sel = Math.max(0, Math.min(rows.length - 1, Math.min(sel, rows.length - 1) + by));
    }

    // Opening (or switching between launcher, clip and op while open) starts fresh.
    Connections {
        target: root.bar
        function onPanelChanged() {
            if (!root.open) {
                root.dragging = false;
                return;
            }
            search.text = "";
            root.setMode(root.bar.panel === "clip" ? "clip" : root.bar.panel === "op" ? "pass" : "apps");
            appsList.positionViewAtBeginning();
            Qt.callLater(() => search.forceActiveFocus());
        }
    }

    // 1Password's authorization dialog had the keyboard; take it back.
    Connections {
        target: pw
        function onLoadingChanged() {
            if (!pw.loading && root.open)
                Qt.callLater(() => search.forceActiveFocus());
        }
    }

    Clip {
        id: clip
        bar: root.bar
    }

    Passwords {
        id: pw
        bar: root.bar
    }

    Timer {
        id: debounce
        interval: 140
        onTriggered: root.runSearch()
    }

    // Field metadata for the selected item's detail pane, once the selection settles.
    Timer {
        id: detailDelay
        interval: 250
        onTriggered: if (root.mode === "pass" && !pw.item)
            pw.prefetch(root.current)
    }

    screen: bar.screen
    // Created once and kept: opening just flips state, with no new surface and no layer
    // open animation. While closed it draws nothing, takes no clicks and no keyboard.
    visible: true
    // Anchored to the top edge only, so the compositor centres it horizontally.
    anchors.top: true
    margins.top: 110
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: 720
    // Fixed height: resizing the layer surface on every keystroke makes it jitter (Hyprland
    // animates layer geometry). The card sizes itself inside; input only reaches the card.
    implicitHeight: 620
    mask: open && !dragging ? contentMask : noInput
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:launcher"
    // While 1Password shows its authorization dialog, let go of the keyboard so it can be
    // typed into.
    WlrLayershell.keyboardFocus: open && !(mode === "pass" && pw.authorizing) ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    Region {
        id: contentMask
        item: card
    }

    Region {
        id: noInput
    }

    // Activating the grab in the same tick the window opens gets it cleared right away (which
    // closes the window); give it a moment, like Panel.qml does.
    property bool grabReady: false

    Timer {
        id: grabDelay
        interval: 50
        running: root.open
        onRunningChanged: if (running)
            root.grabReady = false
        onTriggered: root.grabReady = true
    }

    // Clicking anywhere else closes it, except while 1Password loads: its authorization
    // dialog may need the click.
    HyprlandFocusGrab {
        windows: [root]
        active: root.grabReady && root.open && !(root.mode === "pass" && pw.loading)
        onCleared: root.close()
    }

    // ---- Pieces

    // An app's icon from its desktop entry, or a letter tile when the theme has none.
    component AppIcon: Item {
        id: appIcon

        required property var app
        property int size: 30
        readonly property string source: app?.icon ? Quickshell.iconPath(app.icon, true) : ""

        implicitWidth: size
        implicitHeight: size

        IconImage {
            anchors.fill: parent
            visible: appIcon.source !== ""
            source: appIcon.source
            asynchronous: true
        }
        LetterTile {
            anchors.fill: parent
            visible: appIcon.source === ""
            name: appIcon.app?.name ?? ""
        }
    }

    component LetterTile: Rectangle {
        id: letter

        property string name: ""

        radius: Math.round(width * 0.27)
        color: root.tint(name)

        Rectangle {
            anchors.top: parent.top
            anchors.topMargin: 1
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width - parent.radius * 2
            height: 1
            color: Qt.rgba(1, 1, 1, 0.25)
        }
        UiText {
            anchors.centerIn: parent
            text: letter.name.charAt(0).toUpperCase()
            color: "white"
            font.pixelSize: Math.round(letter.width * 0.42)
            font.weight: Font.DemiBold
        }
    }

    // Tinted square with a line icon (file types, clipboard kinds, the run row).
    component IconTile: Rectangle {
        property alias icon: tileIcon.name
        property alias iconColor: tileIcon.color
        property alias iconSize: tileIcon.size

        implicitWidth: 30
        implicitHeight: 30
        radius: 8

        Icon {
            id: tileIcon
            anchors.centerIn: parent
            size: 16
            strokeWidth: 2
            color: Theme.text
        }
    }

    component Hairline: Rectangle {
        Layout.fillWidth: true
        implicitHeight: 1
        color: Theme.divider
    }

    component Empty: UiText {
        Layout.fillWidth: true
        Layout.topMargin: 22
        Layout.bottomMargin: 22
        horizontalAlignment: Text.AlignHCenter
        dim: true
    }

    // One field of the 1Password detail pane; clicking takes it (shift: copy only).
    component PassField: MouseArea {
        id: passField

        property string label: ""
        property string value: ""
        property bool secret: false
        property bool link: false
        property bool strong: false
        property var keys: []

        signal take(bool paste)

        Layout.fillWidth: true
        implicitHeight: 54
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: mouse => take(!(mouse.modifiers & Qt.ShiftModifier))

        Rectangle {
            anchors.fill: parent
            radius: 12
            color: passField.containsMouse ? Theme.fillHover : passField.strong ? Qt.rgba(1, 1, 1, 0.1) : passField.link ? "transparent" : Qt.rgba(1, 1, 1, 0.06)
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 12

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                UiText {
                    Layout.fillWidth: true
                    text: passField.label
                    dim: true
                    font.pixelSize: Theme.tiny
                }
                UiText {
                    Layout.fillWidth: true
                    text: passField.value
                    textFormat: Text.PlainText
                    mono: passField.secret
                    color: passField.link ? Theme.accent : Theme.text
                    font.letterSpacing: passField.secret ? 1.5 : 0
                }
            }
            KeyHint {
                keys: passField.keys
            }
        }
    }

    // ---- The card

    Glass {
        id: card

        visible: root.open
        anchors.top: parent.top
        anchors.horizontalCenter: parent.horizontalCenter
        width: 720
        height: column.implicitHeight
        radius: 24
        // It would cover the drop targets of a file dragged out of it.
        opacity: root.dragging ? 0 : 1

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.fast
            }
        }

        ColumnLayout {
            id: column

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            spacing: 0

            // Search row: icon, mode chip, field.
            RowLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 62
                Layout.leftMargin: 20
                Layout.rightMargin: 20
                spacing: 12

                Icon {
                    name: "search"
                    size: 20
                    strokeWidth: 2.1
                    color: Qt.alpha(Theme.text, 0.7)
                }

                Rectangle {
                    visible: root.mode !== "apps"
                    implicitWidth: chipText.implicitWidth + 20
                    implicitHeight: 26
                    radius: 8
                    color: Theme.accent

                    UiText {
                        id: chipText
                        anchors.centerIn: parent
                        text: root.modeLabel
                        color: Theme.accentInk
                        font.pixelSize: Theme.small
                        font.weight: Font.DemiBold
                    }
                }

                TextInput {
                    id: search

                    Layout.fillWidth: true
                    color: Theme.white
                    selectionColor: Theme.accent
                    selectedTextColor: Theme.accentInk
                    font.family: root.mode === "run" || root.mode === "calc" ? Theme.mono : Theme.font
                    font.pixelSize: 20
                    font.letterSpacing: -0.2
                    clip: true
                    renderType: Text.NativeRendering
                    onTextEdited: root.textEdited()

                    Keys.onPressed: event => {
                        const k = event.key;
                        const shift = event.modifiers & Qt.ShiftModifier;
                        if (k === Qt.Key_Escape) {
                            if (root.mode === "pass" && pw.item)
                                root.back();
                            else
                                root.close();
                        } else if (k === Qt.Key_Up) {
                            root.move(-1);
                        } else if (k === Qt.Key_Down) {
                            root.move(1);
                        } else if (k === Qt.Key_PageUp) {
                            root.move(-8);
                        } else if (k === Qt.Key_PageDown) {
                            root.move(8);
                        } else if (k === Qt.Key_Backspace && search.text === "" && root.mode !== "apps") {
                            if (root.mode === "pass" && pw.item)
                                root.back();
                            else
                                root.setMode("apps");
                        } else if (k === Qt.Key_Return || k === Qt.Key_Enter) {
                            root.activate(root.current, event.modifiers);
                        } else if (root.mode === "pass" && !pw.item && (k === Qt.Key_Tab || (k === Qt.Key_Right && search.cursorPosition === search.text.length))) {
                            if (root.current)
                                root.showFields(root.current);
                        } else if (root.mode === "pass" && pw.item && (k === Qt.Key_Backtab || (k === Qt.Key_Left && search.cursorPosition === 0))) {
                            // The way back mirrors the way in: ← (at the start of the text) or shift+tab.
                            root.back();
                        } else if (root.mode === "clip" && k === Qt.Key_Delete && (shift || search.cursorPosition === search.text.length)) {
                            clip.remove(root.current);
                        } else {
                            return;
                        }
                        event.accepted = true;
                    }

                    UiText {
                        anchors.fill: parent
                        visible: search.text === ""
                        text: ({
                                apps: "Search apps",
                                run: "Command",
                                calc: "Expression",
                                files: root.scope === "root" ? "Search the whole system" : root.scope === "downloads" ? "Search Downloads" : "Search files",
                                clip: "Search clipboard",
                                pass: pw.item ? "Filter fields" : "Search 1Password"
                            })[root.mode]
                        color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.5)
                        font.family: search.font.family
                        font.pixelSize: search.font.pixelSize
                    }
                }
            }

            Hairline {}

            // ---- Single-column modes: apps, run, calc, files.
            ColumnLayout {
                visible: root.mode !== "clip" && root.mode !== "pass"
                Layout.fillWidth: true
                Layout.margins: 10
                spacing: 6

                SectionLabel {
                    visible: root.showRecent
                    text: "Recent"
                }

                Row {
                    id: recentRow
                    visible: root.showRecent
                    Layout.fillWidth: true
                    Layout.bottomMargin: 4
                    spacing: 4

                    Repeater {
                        model: root.showRecent ? root.recent : []

                        MouseArea {
                            id: tile

                            required property var modelData

                            width: (recentRow.width - recentRow.spacing * 5) / 6
                            height: 92
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.launchApp(modelData)

                            Rectangle {
                                anchors.fill: parent
                                radius: 14
                                color: tile.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
                            }
                            AppIcon {
                                anchors.horizontalCenter: parent.horizontalCenter
                                y: 10
                                size: 48
                                app: tile.modelData
                            }
                            UiText {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                anchors.margins: 4
                                anchors.bottomMargin: 10
                                horizontalAlignment: Text.AlignHCenter
                                text: tile.modelData.name
                                textFormat: Text.PlainText
                                color: Qt.alpha(Theme.text, 0.85)
                                font.pixelSize: Theme.small
                            }
                        }
                    }
                }

                SectionLabel {
                    visible: root.showRecent
                    text: "Applications"
                }

                ListView {
                    id: appsList

                    visible: root.mode === "apps"
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(contentHeight, root.showRecent ? 226 : 440)
                    clip: true
                    spacing: 2
                    model: root.shownApps
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: MouseArea {
                        id: appRow

                        required property var modelData
                        required property int index
                        readonly property bool current: index === root.sel

                        width: appsList.width
                        height: 44
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onEntered: root.sel = index
                        onClicked: root.launchApp(modelData)

                        Rectangle {
                            anchors.fill: parent
                            radius: 12
                            color: appRow.current ? Theme.fillHover : "transparent"
                        }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 12

                            AppIcon {
                                app: appRow.modelData
                            }
                            UiText {
                                Layout.maximumWidth: appsList.width * 0.5
                                text: appRow.modelData.name
                                textFormat: Text.PlainText
                                font.weight: Font.Medium
                            }
                            UiText {
                                Layout.fillWidth: true
                                text: appRow.modelData.detail ?? ""
                                textFormat: Text.PlainText
                                color: Qt.alpha(Theme.text, 0.62)
                            }
                            KeyHint {
                                visible: appRow.current
                                keys: ["↵"]
                                text: "Open"
                            }
                        }
                    }
                }

                // Run: the command as one row.
                Rectangle {
                    visible: root.mode === "run"
                    Layout.fillWidth: true
                    implicitHeight: 44
                    radius: 12
                    color: Theme.fillHover

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 12

                        IconTile {
                            color: "#2b3a3e"
                            icon: "terminal"
                        }
                        UiText {
                            Layout.fillWidth: true
                            mono: true
                            text: root.query.trim() || "Type a command"
                            faint: root.query.trim() === ""
                        }
                        KeyHint {
                            keys: ["↵"]
                            text: "Run in shell"
                        }
                    }
                }

                // Calculator: the result, big.
                Rectangle {
                    visible: root.mode === "calc"
                    Layout.fillWidth: true
                    Layout.margins: 4
                    implicitHeight: calcColumn.implicitHeight + 36
                    radius: 16
                    color: Theme.fill

                    ColumnLayout {
                        id: calcColumn
                        anchors.left: parent.left
                        anchors.right: calcHint.left
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.leftMargin: 20
                        anchors.rightMargin: 16
                        spacing: 4

                        UiText {
                            Layout.fillWidth: true
                            mono: true
                            dim: true
                            text: root.query.trim() || " "
                        }
                        UiText {
                            Layout.fillWidth: true
                            mono: true
                            text: root.calcResult?.text ?? ""
                            color: root.calcResult?.ok ? Theme.white : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.5)
                            font.pixelSize: 32
                            font.weight: Font.Medium
                            font.letterSpacing: -0.6
                        }
                    }
                    KeyHint {
                        id: calcHint
                        anchors.right: parent.right
                        anchors.rightMargin: 20
                        anchors.verticalCenter: parent.verticalCenter
                        keys: ["↵"]
                        text: "Copy"
                        opacity: root.calcResult?.ok ? 1 : 0.4
                    }
                }

                // Files.
                SectionLabel {
                    visible: root.mode === "files"
                    Layout.bottomMargin: 2
                    text: root.filesHeading
                }

                ListView {
                    id: filesList

                    visible: root.mode === "files"
                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(contentHeight, 440)
                    clip: true
                    spacing: 2
                    model: root.mode === "files" ? root.files : []
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: MouseArea {
                        id: fileRow

                        required property var modelData
                        required property int index
                        readonly property bool current: index === root.sel
                        readonly property bool shelved: root.onShelf(modelData.path)

                        width: filesList.width
                        height: 48
                        hoverEnabled: true
                        cursorShape: drag.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                        onEntered: if (!root.dragging)
                            root.sel = index
                        // A click with a modifier does what enter with it does.
                        onClicked: mouse => root.actFile(modelData, mouse.modifiers)
                        // Dragging the row drags the file out (the proxy carries it).
                        drag.target: dragProxy
                        drag.threshold: 8
                        onPressed: fileTile.grabToImage(result => dragProxy.Drag.imageSource = result.url)

                        Item {
                            id: dragProxy

                            width: 1
                            height: 1
                            Drag.dragType: Drag.Automatic
                            Drag.supportedActions: Qt.CopyAction
                            Drag.proposedAction: Qt.CopyAction
                            Drag.hotSpot.x: 15
                            Drag.hotSpot.y: 15
                            Drag.mimeData: ({
                                    "text/uri-list": root.fileUrl(fileRow.modelData.path) + "\r\n",
                                    "text/plain": fileRow.modelData.path
                                })
                            Drag.active: fileRow.drag.active
                            Drag.onDragStarted: root.dragging = true
                            // The drop action reads IgnoreAction even after a good drop, so
                            // don't look at it: the launcher is done either way.
                            Drag.onDragFinished: {
                                root.dragging = false;
                                root.close();
                            }
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: 12
                            color: fileRow.current ? Theme.fillHover : "transparent"
                        }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 12

                            IconTile {
                                id: fileTile
                                color: root.fileTints[fileRow.modelData.kind]
                                icon: fileRow.modelData.kind
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                UiText {
                                    Layout.fillWidth: true
                                    text: fileRow.modelData.name
                                    textFormat: Text.PlainText
                                    font.weight: Font.Medium
                                }
                                UiText {
                                    Layout.fillWidth: true
                                    text: fileRow.modelData.dir
                                    textFormat: Text.PlainText
                                    mono: true
                                    color: Qt.alpha(Theme.text, 0.6)
                                    font.pixelSize: Theme.tiny
                                    elide: Text.ElideMiddle
                                }
                            }
                            Rectangle {
                                visible: fileRow.shelved
                                implicitWidth: shelfText.implicitWidth + 16
                                implicitHeight: 20
                                radius: 10
                                color: Qt.alpha(Theme.accent, 0.26)

                                UiText {
                                    id: shelfText
                                    anchors.centerIn: parent
                                    text: "On shelf"
                                    font.pixelSize: Theme.tiny
                                    font.weight: Font.Medium
                                }
                            }
                            Icon {
                                visible: fileRow.current
                                name: "grip"
                                size: 14
                                color: Qt.alpha(Theme.text, 0.5)
                            }
                            KeyHint {
                                visible: fileRow.current
                                keys: ["↵"]
                                text: "Open"
                            }
                            UiText {
                                visible: !fileRow.current
                                text: root.age(fileRow.modelData.mtime)
                                color: Qt.alpha(Theme.text, 0.6)
                                font.pixelSize: Theme.small
                            }
                        }
                    }
                }

                Empty {
                    visible: (root.mode === "apps" || (root.mode === "files" && !root.filesBusy && (root.query.trim() !== "" || root.scope !== "root"))) && root.rows.length === 0
                    text: root.mode === "files" && root.query.trim() === "" ? (root.scope === "home" ? "No recent files" : "Nothing here") : "No matches"
                }
            }

            // ---- Split modes: clipboard and passwords. List on the left, detail on the right.
            RowLayout {
                visible: root.mode === "clip" || root.mode === "pass"
                Layout.fillWidth: true
                Layout.preferredHeight: 400
                spacing: 0

                Item {
                    Layout.preferredWidth: 290
                    Layout.fillHeight: true

                    ListView {
                        id: clipList

                        visible: root.mode === "clip"
                        anchors.fill: parent
                        anchors.margins: 10
                        anchors.rightMargin: 6
                        clip: true
                        spacing: 2
                        model: root.shownClips
                        boundsBehavior: Flickable.StopAtBounds

                        delegate: MouseArea {
                            id: clipRow

                            required property var modelData
                            required property int index
                            readonly property bool current: index === root.sel

                            width: clipList.width
                            height: 48
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onEntered: root.sel = index
                            // Click pastes like enter; shift+click only copies.
                            onClicked: mouse => root.activate(modelData, mouse.modifiers)

                            Rectangle {
                                anchors.fill: parent
                                radius: 12
                                color: clipRow.current ? Theme.fillHover : "transparent"
                            }
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 10
                                spacing: 12

                                IconTile {
                                    implicitWidth: 28
                                    implicitHeight: 28
                                    color: Qt.rgba(1, 1, 1, 0.1)
                                    icon: clipRow.modelData.kind
                                    iconSize: 15
                                    iconColor: Qt.alpha(Theme.text, 0.85)
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    UiText {
                                        Layout.fillWidth: true
                                        text: clipRow.modelData.title
                                        textFormat: Text.PlainText
                                    }
                                    UiText {
                                        Layout.fillWidth: true
                                        text: clip.meta(clipRow.modelData)
                                        color: Qt.alpha(Theme.text, 0.6)
                                        font.pixelSize: Theme.tiny
                                    }
                                }
                                CircleButton {
                                    visible: clipRow.containsMouse
                                    size: 22
                                    iconSize: 11
                                    icon: "close"
                                    onClicked: clip.remove(clipRow.modelData)
                                }
                            }
                        }
                    }

                    ListView {
                        id: passList

                        visible: root.mode === "pass"
                        anchors.fill: parent
                        anchors.margins: 10
                        anchors.rightMargin: 6
                        clip: true
                        spacing: 2
                        model: root.shownPass
                        boundsBehavior: Flickable.StopAtBounds

                        delegate: MouseArea {
                            id: passRow

                            required property var modelData
                            required property int index
                            readonly property bool current: index === root.sel
                            // Field rows (field view) carry a type; item rows a category.
                            readonly property bool isField: modelData.type !== undefined

                            width: passList.width
                            height: 48
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onEntered: root.sel = index
                            // Click takes the password (or this field), like enter; the
                            // modifiers mean the same as with enter.
                            onClicked: mouse => root.activate(modelData, mouse.modifiers)

                            Rectangle {
                                anchors.fill: parent
                                radius: 12
                                color: passRow.current ? Theme.fillHover : "transparent"
                            }
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 10
                                spacing: 12

                                LetterTile {
                                    visible: !passRow.isField
                                    implicitWidth: 30
                                    implicitHeight: 30
                                    radius: 8
                                    name: passRow.modelData.title ?? ""
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    UiText {
                                        Layout.fillWidth: true
                                        text: passRow.isField ? (passRow.modelData.type === "OTP" ? "One-time code" : passRow.modelData.label) : passRow.modelData.title
                                        textFormat: Text.PlainText
                                        font.weight: Font.Medium
                                    }
                                    UiText {
                                        Layout.fillWidth: true
                                        textFormat: Text.PlainText
                                        text: {
                                            const m = passRow.modelData;
                                            if (passRow.isField)
                                                return m.value === null ? "••••••••" : m.value.replace(/\s+/g, " ");
                                            return m.category === "LOGIN" ? m.user : m.category.toLowerCase().replace("_", " ");
                                        }
                                        mono: passRow.isField && passRow.modelData.value === null
                                        color: Qt.alpha(Theme.text, 0.6)
                                        font.pixelSize: Theme.tiny
                                    }
                                }
                                CircleButton {
                                    visible: !passRow.isField && passRow.containsMouse
                                    size: 22
                                    iconSize: 12
                                    icon: "chevron-right"
                                    onClicked: root.showFields(passRow.modelData)
                                }
                            }
                        }
                    }

                    Empty {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.topMargin: 22
                        visible: root.rows.length === 0 && !(root.mode === "pass" && pw.loading)
                        text: root.mode === "clip" ? (clip.entries.length === 0 ? "Clipboard history is empty" : "No matches") : pw.item ? "No fields" : pw.items.length === 0 ? "No items" : "No matches"
                    }

                    // Column divider.
                    Rectangle {
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 1
                        color: Theme.divider
                    }
                }

                // Clipboard preview.
                ColumnLayout {
                    visible: root.mode === "clip" && root.current !== null
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 16
                    spacing: 12

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        radius: 14
                        color: Qt.rgba(0, 0, 0, 0.22)
                        clip: true

                        Text {
                            visible: root.current?.kind !== "image"
                            anchors.fill: parent
                            anchors.margins: 14
                            text: root.mode === "clip" && root.current ? (clip.texts[root.current.id] ?? root.current.text) : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.WrapAnywhere
                            color: Theme.text
                            font.family: Theme.mono
                            font.pixelSize: 12
                            lineHeight: 1.45
                            verticalAlignment: Text.AlignTop
                            renderType: Text.NativeRendering
                        }

                        Image {
                            visible: root.current?.kind === "image"
                            anchors.fill: parent
                            anchors.margins: 14
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                            cache: false
                            sourceSize.width: 720
                            source: root.mode === "clip" && root.current?.kind === "image" ? `file://${clip.thumbDir}/${root.current.id}?${clip.thumbGeneration}` : ""
                        }
                    }

                    UiText {
                        Layout.fillWidth: true
                        text: root.mode === "clip" ? clip.meta(root.current) : ""
                        color: Qt.alpha(Theme.text, 0.62)
                        font.pixelSize: Theme.small
                    }
                }

                // 1Password detail: the selected item (or the item whose fields are listed).
                ColumnLayout {
                    id: detail

                    readonly property var item: root.mode !== "pass" ? null : pw.item ?? root.current

                    visible: detail.item !== null
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 16
                    spacing: 12

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.bottomMargin: 6
                        spacing: 12

                        LetterTile {
                            implicitWidth: 44
                            implicitHeight: 44
                            radius: 12
                            name: detail.item?.title ?? ""
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            UiText {
                                Layout.fillWidth: true
                                text: detail.item?.title ?? ""
                                textFormat: Text.PlainText
                                font.pixelSize: 16
                                font.weight: Font.DemiBold
                            }
                            UiText {
                                Layout.fillWidth: true
                                text: detail.item?.vaultName || (detail.item?.category ?? "").toLowerCase().replace("_", " ")
                                color: Qt.alpha(Theme.text, 0.62)
                                font.pixelSize: Theme.small
                            }
                        }
                    }

                    PassField {
                        visible: (detail.item?.user ?? "") !== ""
                        label: "Username"
                        value: detail.item?.user ?? ""
                        keys: ["ctrl", "↵"]
                        onTake: paste => root.takePass(detail.item, "username", paste)
                    }
                    PassField {
                        label: "Password"
                        value: "••••••••••••"
                        secret: true
                        strong: true
                        keys: ["↵"]
                        onTake: paste => root.takePass(detail.item, "password", paste)
                    }
                    PassField {
                        visible: pw.hasOtp(detail.item) === true
                        label: "One-time code"
                        value: "••• •••"
                        secret: true
                        keys: ["alt", "↵"]
                        onTake: paste => root.takePass(detail.item, "otp", paste)
                    }
                    PassField {
                        readonly property string url: (detail.item?.urls ?? "").split(" ")[0]

                        visible: url !== ""
                        label: "Website"
                        value: url.replace(/^https?:\/\//, "").replace(/\/$/, "")
                        link: true
                        onTake: {
                            root.close();
                            Quickshell.execDetached(["uwsm-app", "--", "xdg-open", /^[a-z]+:\/\//.test(url) ? url : "https://" + url]);
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                    }
                }

                // Nothing selected (empty list): keep the split's width.
                Item {
                    visible: (root.mode === "clip" && root.current === null) || (root.mode === "pass" && detail.item === null)
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    UiText {
                        anchors.centerIn: parent
                        visible: root.mode === "pass" && pw.authorizing
                        text: "Waiting for 1Password to authorize…"
                        dim: true
                    }
                }
            }

            Hairline {}

            // Footer: count or status, then the keys.
            RowLayout {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                spacing: 14

                UiText {
                    text: root.status
                    color: root.mode === "pass" && pw.error && !pw.loading ? Theme.warnText : Theme.textDim
                    font.pixelSize: Theme.small
                    Layout.maximumWidth: 280
                }

                PillButton {
                    visible: root.mode === "clip" && clip.entries.length > 0
                    text: "Wipe"
                    pillHeight: 22
                    onClicked: clip.wipe()
                }

                Item {
                    Layout.fillWidth: true
                }

                Repeater {
                    model: root.hints

                    KeyHint {
                        required property var modelData
                        keys: modelData.keys
                        text: modelData.text
                    }
                }
            }
        }
    }

    // ---- Processes

    Process {
        id: finder

        // The search this run is for, and the last one whose results are shown.
        property string key: ""
        property string done: ""

        stdout: StdioCollector {
            onStreamFinished: {
                // A search stopped for a newer one still ends here; drop its output.
                if (finder.key !== root.fileKey)
                    return;
                root.files = root.parseFiles(text, finder.key);
                root.sel = 0;
                finder.done = finder.key;
            }
        }
        onRunningChanged: if (!running)
            Qt.callLater(root.runSearch)
    }

    Process {
        command: ["sh", "-c", "command -v fd"]
        running: true
        onExited: code => root.hasFd = code === 0
    }

    FileView {
        id: store

        path: Quickshell.statePath("launcher.json")
        // A missing file just means nothing has been launched yet.
        printErrors: false
        onLoaded: {
            try {
                root.launches = JSON.parse(text()) ?? {};
            } catch (err) {
                root.launches = {};
            }
        }
    }
}
