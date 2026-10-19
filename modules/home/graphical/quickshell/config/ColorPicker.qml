pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Colour picker state: hyprpicker picks (and copies the hex), this keeps the history and
// whether the bar shows the slot. The slot appears the first time the picker is used and
// stays until removed from its panel. Saved in the state dir.
Singleton {
    id: root

    property bool slotShown: false
    // Newest first, "#rrggbb", at most 16.
    property var history: []
    property int current: 0
    // When `current` was picked (0 = picked before this session / chosen from history).
    property real pickedAt: 0
    property string error: ""

    readonly property string hex: history[current] ?? "#6fd3c9"
    readonly property int r: parseInt(hex.slice(1, 3), 16)
    readonly property int g: parseInt(hex.slice(3, 5), 16)
    readonly property int b: parseInt(hex.slice(5, 7), 16)

    // [[name, value]] in the formats offered for copying.
    readonly property var formats: {
        const rn = r / 255, gn = g / 255, bn = b / 255;
        const mx = Math.max(rn, gn, bn), mn = Math.min(rn, gn, bn);
        const l = (mx + mn) / 2, d = mx - mn;
        let h = 0, s = 0;
        if (d) {
            s = d / (1 - Math.abs(2 * l - 1));
            h = mx === rn ? ((gn - bn) / d) % 6 : mx === gn ? (bn - rn) / d + 2 : (rn - gn) / d + 4;
            h = Math.round(h * 60);
            if (h < 0)
                h += 360;
        }
        const pct = x => Math.round(x * 100) + "%";
        return [["HEX", hex.toUpperCase()], ["RGB", `rgb(${r}, ${g}, ${b})`], ["HSL", `hsl(${h}, ${pct(s)}, ${pct(l)})`], ["QML", `Qt.rgba(${[rn, gn, bn].map(x => x.toFixed(3)).join(", ")}, 1)`]];
    }

    function pick() {
        error = "";
        if (!picker.running)
            picker.running = true;
    }

    function record(color) {
        const c = color.toLowerCase();
        history = [c].concat(history.filter(x => x !== c)).slice(0, 16);
        current = 0;
        pickedAt = Date.now();
        slotShown = true;
        save();
    }

    function select(i) {
        current = i;
        pickedAt = 0;
    }

    function clear() {
        history = [hex];
        current = 0;
        save();
    }

    function remove() {
        slotShown = false;
        save();
    }

    function copy(text) {
        Quickshell.execDetached(["wl-copy", "--", text]);
    }

    function save() {
        store.setText(JSON.stringify({
            slotShown: slotShown,
            history: history
        }));
    }

    Process {
        id: picker

        property string out: ""

        // Copies the hex itself (-a); prints it, or nothing when cancelled with escape.
        command: ["sh", "-c", "exec hyprpicker -a -f hex"]
        stdout: StdioCollector {
            onStreamFinished: {
                const m = text.match(/#[0-9a-fA-F]{6}/);
                if (m)
                    root.record(m[0]);
            }
        }
        onExited: code => {
            if (code === 127)
                root.error = "hyprpicker isn't installed";
        }
    }

    FileView {
        id: store

        path: Quickshell.statePath("picker.json")
        // Missing until the picker is first used.
        printErrors: false
        onLoaded: {
            try {
                const saved = JSON.parse(text());
                root.slotShown = !!saved.slotShown;
                root.history = (saved.history ?? []).filter(c => /^#[0-9a-f]{6}$/.test(c));
            } catch (e) {}
        }
    }

    // `qs ipc call picker pick` does what the keybind does.
    IpcHandler {
        target: "picker"

        function pick(): void {
            root.pick();
        }
    }
}
