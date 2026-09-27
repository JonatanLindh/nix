pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Hairline: text only, monospace, one accent.
Singleton {
    id: root

    readonly property color bg: "#101014"
    readonly property color barBg: Qt.alpha(bg, 0.90)
    readonly property color hover: "#18181e"
    readonly property color line: "#23232a"
    readonly property color border: "#2a2a30"
    readonly property color track: "#34343c"

    readonly property color text: "#c9c7c0"
    readonly property color bright: "#ecebe6"
    readonly property color label: "#6c6a66"
    readonly property color off: "#4a4a52"

    readonly property color accent: "#e0a458"
    readonly property color warn: "#e46876"

    // fontconfig's default monospace (set in modules/nixos/common.nix). Qt doesn't apply
    // fontconfig's "monospace" alias itself, so resolve it to a family name once at startup.
    property string font: "monospace"
    readonly property int fontSize: 12

    readonly property int barHeight: 28
    readonly property int barPadding: 14
    readonly property int gap: 18
    readonly property int panelWidth: 380
    readonly property int panelPadding: 16

    Process {
        running: true
        command: ["fc-match", "-f", "%{family[0]}", "monospace"]
        stdout: StdioCollector {
            onStreamFinished: if (text.trim())
                root.font = text.trim()
        }
    }
}
