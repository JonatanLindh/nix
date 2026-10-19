pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io

// 1Password through qs-op (scripts/qs-op.sh) and the app integration: the data behind the
// launcher's passwords mode (`*`, or super+shift+p). No window of its own.
// Secrets never reach the bar: the list and field views are metadata only, and picking
// something has qs-op put it straight onto the clipboard (sensitive, so cliphist skips it,
// and cleared after 30s). Enter pastes the password, ctrl+enter the username, alt+enter
// the one-time code; add shift to only copy. Tab opens an item's fields.
// Bar.qml still creates one of these next to the launcher's own; that one stays idle.
Scope {
    id: root

    required property var bar

    // Item metadata (no secrets), kept in memory between opens so the list shows at once and
    // refreshed in the background on every open (about a second once 1Password has
    // authorized the CLI). Only the first open after the bar starts has to wait.
    // [{ id, title, category, vault, vaultName?, user, urls }]
    property var items: []
    // Waiting for the first list (nothing to show yet).
    property bool loading: false
    // Loading for a while means 1Password is showing its authorization dialog: the launcher
    // lets go of the keyboard so it can be typed into, and takes it back once the list arrives.
    property bool authorizing: false
    property string error: ""
    // Item whose fields are shown (field view), or null for the item list.
    property var item: null
    readonly property var fields: item ? (fieldCache[item.id] ?? []) : []

    // Field metadata per item id, fetched for the selected item's detail pane (to know if it
    // has a one-time code) and for the field view: { id: [{ id, label, type, value|null }] }.
    // Cleared on every open, like the list.
    property var fieldCache: ({})
    property var wantFields: null

    // Picks per item id: { id: { n: count, t: last pick in ms } }, kept in the state dir.
    property var uses: ({})

    // Frequency, weighted towards recent use (same as the launcher).
    function frecency(id) {
        const u = uses[id];
        if (!u)
            return 0;
        const days = (Date.now() - u.t) / 86400000;
        return u.n * (days < 1 ? 4 : days < 7 ? 2 : days < 30 ? 1 : 0.5);
    }

    function record(id) {
        const all = Object.assign({}, uses);
        const u = all[id] ?? {
            n: 0,
            t: 0
        };
        all[id] = {
            n: u.n + 1,
            t: Date.now()
        };
        uses = all;
        usageStore.setText(JSON.stringify(all));
    }

    // Starts a refresh of the list; called when the launcher enters passwords mode.
    function refresh() {
        item = null;
        error = "";
        fieldCache = {};
        wantFields = null;
        loading = items.length === 0;
        authorizing = false;
        if (loading)
            slowLoad.restart();
        listing.running = true;
    }

    // Title prefix > word prefix > substring (title, username, urls) > subsequence of the
    // title; ties (and an empty query) go to the most used, then alphabetical. At most 50 rows.
    function rank(text) {
        const q = text.trim().toLowerCase();
        if (!q)
            return items.slice().sort((a, b) => frecency(b.id) - frecency(a.id) || a.title.localeCompare(b.title)).slice(0, 50);
        const scored = [];
        for (const i of items) {
            const t = i.title.toLowerCase();
            let s = 0;
            if (t.startsWith(q))
                s = 4;
            else if (t.split(/[\s\-_.@]+/).some(w => w.startsWith(q)))
                s = 3;
            else if (t.includes(q) || i.user.toLowerCase().includes(q) || i.urls.toLowerCase().includes(q))
                s = 2;
            else if (subsequence(q, t))
                s = 1;
            if (s > 0)
                scored.push({
                    item: i,
                    score: s
                });
        }
        return scored.sort((a, b) => b.score - a.score || frecency(b.item.id) - frecency(a.item.id) || a.item.title.localeCompare(b.item.title)).slice(0, 50).map(x => x.item);
    }

    function subsequence(q, s) {
        let i = 0;
        for (const c of s) {
            if (c === q[i])
                i++;
            if (i === q.length)
                return true;
        }
        return false;
    }

    function matchFields(text) {
        const q = text.trim().toLowerCase();
        return q ? fields.filter(f => f.label.toLowerCase().includes(q)) : fields;
    }

    // Has a one-time code: true / false, or undefined while its fields aren't known yet.
    function hasOtp(target) {
        const f = target ? fieldCache[target.id] : undefined;
        return f === undefined ? undefined : f.some(x => x.type === "OTP");
    }

    // what: a field id, "otp", or one of the shortcuts "password" / "username". Call it
    // after closing the launcher.
    function take(target, what, paste) {
        if (!target)
            return;
        record(target.id);
        copier.command = ["qs-op", "copy", target.vault, target.id, what].concat(paste ? ["paste"] : []);
        copier.running = true;
    }

    // Fetches an item's field metadata in the background (once per open).
    function prefetch(target) {
        if (!target || fieldCache[target.id] !== undefined)
            return;
        wantFields = target;
        nextFields();
    }

    function nextFields() {
        const t = wantFields;
        if (fieldQuery.running || loading || !t || fieldCache[t.id] !== undefined)
            return;
        fieldQuery.itemId = t.id;
        fieldQuery.command = ["qs-op", "fields", t.vault, t.id];
        fieldQuery.running = true;
    }

    function showFields(target) {
        if (!target)
            return;
        item = target;
        prefetch(target);
    }

    function back() {
        item = null;
    }

    onLoadingChanged: {
        if (loading)
            return;
        slowLoad.stop();
        authorizing = false;
        Qt.callLater(nextFields);
    }

    Timer {
        id: slowLoad
        interval: 1500
        onTriggered: root.authorizing = root.loading
    }

    FileView {
        id: usageStore

        path: Quickshell.statePath("passwords.json")
        // A missing file just means nothing has been picked yet.
        printErrors: false
        onLoaded: {
            try {
                root.uses = JSON.parse(text()) ?? {};
            } catch (err) {
                root.uses = {};
            }
        }
    }

    Process {
        id: listing
        command: ["qs-op", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.loading = false;
                try {
                    root.items = JSON.parse(text).sort((a, b) => a.title.localeCompare(b.title));
                } catch (e) {
                    root.items = [];
                }
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim())
                root.error = text.trim().split("\n").pop()
        }
        onRunningChanged: if (!running)
            root.loading = false
    }

    Process {
        id: fieldQuery

        property string itemId: ""

        stdout: StdioCollector {
            onStreamFinished: {
                let parsed = [];
                try {
                    parsed = JSON.parse(text);
                } catch (e) {}
                const all = Object.assign({}, root.fieldCache);
                all[fieldQuery.itemId] = parsed;
                root.fieldCache = all;
                Qt.callLater(root.nextFields);
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim())
                root.error = text.trim().split("\n").pop()
        }
        onRunningChanged: if (!running)
            Qt.callLater(root.nextFields)
    }

    // Runs after the launcher has closed, so failures go out as a notification.
    Process {
        id: copier
        stderr: StdioCollector {
            onStreamFinished: {
                const message = text.trim().split("\n").pop();
                if (message)
                    Quickshell.execDetached(["notify-send", "-a", "1password", "couldn't copy", message]);
            }
        }
    }
}
