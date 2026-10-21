pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io

// Clipboard history from cliphist (services.cliphist) through qs-clip (helpers/): the data
// behind the launcher's clipboard mode (`"`, or super+v). No window of its own; the launcher
// draws it.
Scope {
    id: root

    required property var bar

    // [{ id, line, text, title, kind: "text" | "link" | "code" | "image", meta }]
    property var entries: []
    readonly property string thumbDir: Quickshell.env("XDG_RUNTIME_DIR") + "/quickshell-clip"
    // Bumped when thumbnails are written so image sources reload.
    property int thumbGeneration: 0

    // From the full entry, decoded on demand for the preview pane (the listing only has a
    // one-line preview): { id: text } (at most 8000 bytes) and { id: meta }.
    property var texts: ({})
    property var metas: ({})
    property string wanted: ""

    function load() {
        listing.running = true;
    }

    function filter(text) {
        const q = text.trim().toLowerCase();
        return q ? entries.filter(e => e.text.toLowerCase().includes(q)) : entries;
    }

    // Copies the entry, and with `paste` also pastes it into the window that was focused
    // (qs-paste sends that app's paste shortcut once focus is back; see scripts/qs-paste.sh).
    // Call it after closing the launcher.
    function pick(entry, paste) {
        if (!entry)
            return;
        paster.command = ["qs-clip", "pick", entry.id].concat(paste ? ["paste"] : []);
        paster.running = true;
    }

    function remove(entry) {
        if (!entry)
            return;
        run.command = ["qs-clip", "delete", entry.line];
        run.running = true;
    }

    function wipe() {
        run.command = ["qs-clip", "wipe"];
        run.running = true;
    }

    // Makes sure the preview pane has what it needs for `entry`: the full text, or the image
    // decoded to a thumbnail file.
    function preview(entry) {
        if (!entry || texts[entry.id] !== undefined)
            return;
        wanted = entry.id;
        nextPreview();
    }

    function nextPreview() {
        if (previewer.running || !wanted || texts[wanted] !== undefined)
            return;
        previewer.id = wanted;
        previewer.command = ["qs-clip", "preview", wanted, thumbDir];
        previewer.running = true;
    }

    // "Text · 27 characters", "Code · 5 lines", "Link · github.com", "PNG · 1920 × 1080 · 2 MiB".
    function meta(entry) {
        return entry ? (metas[entry.id] ?? entry.meta) : "";
    }

    // The entries, then a line per thumbnail as the newest images are decoded.
    Process {
        id: listing
        command: ["qs-clip", "list", root.thumbDir]
        stdout: SplitParser {
            onRead: line => {
                let d;
                try {
                    d = JSON.parse(line);
                } catch (e) {
                    return;
                }
                if (d.entries)
                    root.entries = d.entries;
                else if (d.thumb)
                    root.thumbGeneration++;
            }
        }
    }

    Process {
        id: previewer

        property string id: ""

        stdout: StdioCollector {
            onStreamFinished: {
                let d = {};
                try {
                    d = JSON.parse(text);
                } catch (e) {}
                const texts = Object.assign({}, root.texts);
                // Images (and entries that failed to decode) have no text; "" marks them done.
                texts[previewer.id] = d.text ?? "";
                root.texts = texts;
                if (d.meta) {
                    const metas = Object.assign({}, root.metas);
                    metas[previewer.id] = d.meta;
                    root.metas = metas;
                }
                if (d.thumb)
                    root.thumbGeneration++;
            }
        }
        onRunningChanged: if (!running)
            Qt.callLater(root.nextPreview)
    }

    // Its own process: the paste happens after the launcher has closed.
    Process {
        id: paster
    }

    Process {
        id: run
        onRunningChanged: if (!running)
            root.load()
    }
}
