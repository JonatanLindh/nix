pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    property string device: ""
    property int max: 1
    property int current: 0
    readonly property bool available: device !== ""
    readonly property int percent: Math.round(current / max * 100)

    function step(up) {
        Quickshell.execDetached(["brightnessctl", "set", up ? "+5%" : "5%-"]);
    }

    Process {
        running: true
        command: ["sh", "-c", "ls /sys/class/backlight 2>/dev/null | head -n1"]
        stdout: StdioCollector {
            onStreamFinished: root.device = text.trim()
        }
    }

    FileView {
        path: root.device ? `/sys/class/backlight/${root.device}/max_brightness` : ""
        onLoaded: root.max = parseInt(text()) || 1
    }

    FileView {
        id: brightness
        path: root.device ? `/sys/class/backlight/${root.device}/brightness` : ""
        watchChanges: true
        onFileChanged: reload()
        onLoaded: root.current = parseInt(text()) || 0
    }

    // sysfs doesn't always emit change events (e.g. hardware brightness keys).
    Timer {
        interval: 2000
        running: root.available
        repeat: true
        onTriggered: brightness.reload()
    }
}
