import qs
import QtQuick

// "mic cam share" in the bar while something records the microphone, camera or screen.
// In Theme.warn rather than the accent: the accent marks settings the user turned on
// (caf, fx, exit node); this marks something another app is doing, which should stand out.
Row {
    visible: Privacy.mic || Privacy.camera || Privacy.screen
    height: parent?.height ?? Theme.barHeight
    spacing: Theme.gap

    BarText {
        visible: Privacy.mic
        height: parent.height
        text: "mic"
        color: Theme.warn
    }

    BarText {
        visible: Privacy.camera
        height: parent.height
        text: "cam"
        color: Theme.warn
    }

    BarText {
        visible: Privacy.screen
        height: parent.height
        text: "share"
        color: Theme.warn
    }
}
