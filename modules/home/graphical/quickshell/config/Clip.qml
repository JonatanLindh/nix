pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io

// Clipboard history from cliphist (services.cliphist): the data behind the launcher's
// clipboard mode (`"`, or super+v). No window of its own; the launcher draws it.
// Bar.qml still creates one of these next to the launcher's own; that one stays idle.
Scope {
    id: root

    required property var bar

    // [{ id, line, text, title, kind: "text" | "link" | "code" | "image", mime, size }]
    property var entries: []
    readonly property string thumbDir: Quickshell.env("XDG_RUNTIME_DIR") + "/quickshell-clip"
    // Bumped when thumbnails are written so image sources reload.
    property int thumbGeneration: 0

    // Full text of entries, decoded on demand for the preview pane: { id: text }. The
    // listing only carries a one-line preview.
    property var texts: ({})
    // Longest text decoded for a preview, in bytes.
    readonly property int previewLimit: 8000
    property string wantText: ""

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
        paster.command = ["sh", "-c", 'cliphist decode "$1" | wl-copy && [ "$2" = paste ] && qs-paste', "sh", entry.id, paste ? "paste" : "copy"];
        paster.running = true;
    }

    function remove(entry) {
        if (!entry)
            return;
        run.command = ["sh", "-c", 'printf "%s" "$1" | cliphist delete', "sh", entry.line];
        run.running = true;
    }

    function wipe() {
        run.command = ["cliphist", "wipe"];
        run.running = true;
    }

    // Makes sure the preview pane has what it needs for `entry`: the full text, or the
    // image decoded to a thumbnail file.
    function preview(entry) {
        if (!entry)
            return;
        if (entry.kind === "image") {
            thumbs.queue([entry.id]);
        } else if (texts[entry.id] === undefined) {
            wantText = entry.id;
            nextText();
        }
    }

    function nextText() {
        if (decoder.running || !wantText || texts[wantText] !== undefined)
            return;
        decoder.id = wantText;
        decoder.command = ["sh", "-c", 'cliphist decode "$1" | head -c "$2"', "sh", wantText, String(previewLimit)];
        decoder.running = true;
    }

    // One-line title, and a guess at what the text is.
    function kindOf(text) {
        const t = text.trim();
        if (/^(https?:\/\/|www\.)\S+$/.test(t))
            return "link";
        if (/[{};]\s*$|=>|&&|\|\||^\s*(\$ |sudo |nix |git |cd |just |hl\.|import |const |let |def |fn )/m.test(t) || /\n[ \t]+\S/.test(t))
            return "code";
        return "text";
    }

    // "Text · 27 characters", "Text · 5 lines", "Link · github.com", "PNG · 1920×1080 · 2 MiB".
    function meta(entry) {
        if (!entry)
            return "";
        if (entry.kind === "image")
            return entry.mime.toUpperCase() + " · " + entry.size;
        const full = texts[entry.id] ?? entry.text;
        if (entry.kind === "link") {
            const host = full.trim().replace(/^https?:\/\//, "").split(/[/?#]/)[0];
            return "Link · " + host;
        }
        const lines = full.replace(/\n$/, "").split("\n").length;
        const more = full.length >= previewLimit ? "+" : "";
        const label = entry.kind === "code" ? "Code" : "Text";
        return lines > 1 ? `${label} · ${lines}${more} lines` : `${label} · ${full.length}${more} characters`;
    }

    Process {
        id: listing
        command: ["cliphist", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.entries = text.split("\n").filter(l => l.includes("\t")).map(line => {
                    const tab = line.indexOf("\t");
                    const preview = line.slice(tab + 1);
                    // "[[ binary data 1.2 MiB png 1920x1080 ]]"
                    const img = preview.match(/^\[\[ binary data (.+?) (\w+) (\d+)x(\d+) \]\]$/);
                    return {
                        id: line.slice(0, tab),
                        line: line,
                        text: img ? "image " + img[2] + " " + img[3] + "x" + img[4] : preview,
                        title: img ? `Image · ${img[3]} × ${img[4]}` : preview.trim().replace(/\s+/g, " ") || "Whitespace",
                        kind: img ? "image" : root.kindOf(preview),
                        mime: img ? img[2] : "",
                        size: img ? `${img[3]} × ${img[4]} · ${img[1]}` : ""
                    };
                });
                thumbs.queue(root.entries.filter(e => e.kind === "image").slice(0, 30).map(e => e.id));
            }
        }
    }

    // Decode image entries to files once so they can be shown as thumbnails.
    Process {
        id: thumbs

        property var pending: []

        function queue(ids) {
            pending = pending.concat(ids.filter(id => !pending.includes(id)));
            start();
        }

        function start() {
            if (running || pending.length === 0)
                return;
            command = ["sh", "-c", 'dir="$1"; shift; mkdir -p "$dir"; for id; do [ -s "$dir/$id" ] || cliphist decode "$id" > "$dir/$id"; done', "sh", root.thumbDir].concat(pending);
            pending = [];
            running = true;
        }

        onRunningChanged: if (!running) {
            root.thumbGeneration++;
            Qt.callLater(start);
        }
    }

    Process {
        id: decoder

        property string id: ""

        stdout: StdioCollector {
            onStreamFinished: {
                const all = Object.assign({}, root.texts);
                all[decoder.id] = text;
                root.texts = all;
                Qt.callLater(root.nextText);
            }
        }
        onRunningChanged: if (!running)
            Qt.callLater(root.nextText)
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
