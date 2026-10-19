pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io

// Screenshots and screen recordings: the capture overlay's state (ScreenshotOverlay.qml, one
// per monitor), saving the shot from its frozen frame or recording with wf-recorder, and the
// notification afterwards (click opens it in Gradia; Copy text runs OCR via the shelf
// daemon; Add to shelf puts it on the shelf).
Singleton {
    id: root

    // The overlay is up (on every monitor; the focused one takes the keyboard).
    property bool active: false
    // "selection" | "screen" | "window"
    property string mode: "selection"
    property bool video: false
    property bool pointer: false
    // The monitor whose overlay last saw the pointer move; only it highlights a window, as
    // leaving one overlay for another doesn't reliably tell the first.
    property string pointerMonitor: ""
    // The last selection per monitor name, in that monitor's logical coordinates, so the next
    // capture starts from it (like GNOME's).
    property var selections: ({})

    // Recording in progress (wf-recorder), and when it started.
    property bool recording: recorder.running
    property date recordingSince: new Date()
    property string recordingFile: ""

    // Window rectangles for window mode, global logical coordinates, topmost first:
    // [{ x, y, w, h, title, monitor }]
    property var windows: []

    readonly property string picturesDir: Quickshell.env("HOME") + "/Pictures/Screenshots"
    readonly property string videosDir: Quickshell.env("HOME") + "/Videos/Screencasts"
    // Full-resolution stills of each monitor, taken as the overlay opens.
    readonly property string stillsDir: Quickshell.env("XDG_RUNTIME_DIR") + "/quickshell-screenshot"

    function open(m) {
        if (recording) {
            stop();
            return;
        }
        mode = m ?? "selection";
        windowList.running = true;
        active = true;
    }

    function close() {
        active = false;
        Quickshell.execDetached(["rm", "-rf", stillsDir]);
    }

    // "Screenshot from 2026-10-19 20-31-07", like GNOME.
    function stamp() {
        return Qt.formatDateTime(new Date(), "yyyy-MM-dd HH-mm-ss");
    }

    // Records a global logical rectangle (or a whole output). The overlay is
    // hidden first and given a moment to leave the screen, so it isn't in the picture.
    function capture(x, y, w, h, output) {
        if (w < 2 || h < 2)
            return;
        pending = {
            geometry: `${Math.round(x)},${Math.round(y)} ${Math.round(w)}x${Math.round(h)}`,
            output: output ?? ""
        };
        close();
        settle.restart();
    }

    property var pending: null

    Timer {
        id: settle
        interval: 180
        onTriggered: root.startRecording()
    }

    // Saves a screenshot cut from an overlay still (crop in physical pixels, "WxH+X+Y"), copies
    // it to the clipboard, then announces it.
    function saveFrozen(src, crop) {
        active = false;
        const file = `${picturesDir}/Screenshot from ${stamp()}.png`;
        shooter.file = file;
        shooter.command = ["bash", "-c", 'mkdir -p "$1" && magick "$3" -crop "$4" +repage "$2"; code=$?; rm -rf "$5"; [ $code = 0 ] && wl-copy --type image/png < "$2"', "bash", picturesDir, file, src, crop, stillsDir];
        shooter.running = true;
    }

    Process {
        id: shooter
        property string file: ""
        onExited: code => {
            if (code === 0)
                root.announce(file, false);
            else
                Quickshell.execDetached(["notify-send", "-a", "Screenshot", "Screenshot failed", "Saving the capture exited with " + code]);
        }
    }

    function startRecording() {
        const file = `${videosDir}/Screencast from ${stamp()}.mp4`;
        recordingFile = file;
        const target = pending.output ? ["-o", pending.output] : ["-g", pending.geometry];
        recorder.command = ["sh", "-c", 'mkdir -p "$1" && shift && exec "$@"', "sh", videosDir, "wf-recorder", "-f", file].concat(target);
        recordingSince = new Date();
        recorder.running = true;
    }

    // Stops a recording; wf-recorder finishes the file on SIGINT.
    function stop() {
        if (recorder.running)
            recorder.signal(2);
    }

    Process {
        id: recorder
        onExited: code => {
            // 0, or 130/255 after SIGINT, still leave a complete file.
            root.announce(root.recordingFile, true);
        }
    }

    // The notification: thumbnail, click opens it, actions for the rest. notify-send waits
    // and prints the action picked.
    function announce(file, isVideo) {
        const n = notifier.createObject(root, {
            file: file,
            isVideo: isVideo
        });
        n.running = true;
    }

    Component {
        id: notifier

        Process {
            id: p
            property string file
            property bool isVideo
            readonly property string name: file.split("/").pop()

            command: ["notify-send", "--wait", "-a", isVideo ? "Screen recording" : "Screenshot", "-i", isVideo ? "video-x-generic" : file].concat(isVideo ? ["-A", "default=Open", "-A", "folder=Show in Files"] : ["-h", "string:image-path:" + file, "-A", "default=Edit", "-A", "ocr=Copy text"], ["-A", "shelf=Add to shelf", isVideo ? "Screen recording saved" : "Screenshot captured", isVideo ? name : "Copied to the clipboard · click to edit"])
            stdout: StdioCollector {
                onStreamFinished: root.act(text.trim(), p.file, p.isVideo)
            }
            onExited: destroy()
        }
    }

    function act(action, file, isVideo) {
        if (action === "default")
            Quickshell.execDetached(isVideo ? ["uwsm-app", "--", "xdg-open", file] : ["uwsm-app", "--", "gradia", "--screenshot-file=" + file]);
        else if (action === "ocr")
            Quickshell.execDetached(["sh", "-c", 'shelf run ocr "$1" >/dev/null || notify-send -a Screenshot "Copy text failed" "The shelf daemon is not running"', "sh", file]);
        else if (action === "folder")
            Quickshell.execDetached(["dbus-send", "--session", "--dest=org.freedesktop.FileManager1", "--type=method_call", "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems", `array:string:file://${file}`, "string:"]);
        else if (action === "shelf")
            Shelf.add([file]);
    }

    // Window mode: the visible windows' rectangles, topmost first (by focus history, which
    // is close to the stacking order for tiled and floating windows alike).
    Process {
        id: windowList
        command: ["sh", "-c", "hyprctl -j monitors; printf '\\n---\\n'; hyprctl -j clients"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const [m, c] = text.split("\n---\n");
                    const monitors = JSON.parse(m);
                    // No flatMap in QML's JavaScript.
                    const shown = [].concat(...monitors.map(mon => [mon.activeWorkspace?.id, mon.specialWorkspace?.id])).filter(id => id !== undefined && id !== 0);
                    root.windows = JSON.parse(c).filter(w => w.mapped && !w.hidden && shown.includes(w.workspace?.id)).sort((a, b) => a.focusHistoryID - b.focusHistoryID).map(w => ({
                                x: w.at[0],
                                y: w.at[1],
                                w: w.size[0],
                                h: w.size[1],
                                title: w.title,
                                monitor: monitors.find(mon => mon.id === w.monitor)?.name ?? ""
                            }));
                } catch (e) {
                    root.windows = [];
                }
            }
        }
    }

    // `qs ipc call screenshot open` (or window / screen / stop / close) from scripts.
    IpcHandler {
        target: "screenshot"

        function open(): void {
            root.open("selection");
        }
        function window(): void {
            root.open("window");
        }
        function screen(): void {
            root.open("screen");
        }
        function stop(): void {
            root.stop();
        }
        function close(): void {
            root.close();
        }
    }
}
