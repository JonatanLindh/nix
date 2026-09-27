import qs
import QtQuick
import Quickshell

Row {
    spacing: 12

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    BarText {
        anchors.verticalCenter: parent.verticalCenter
        text: Qt.formatDateTime(clock.date, "HH:mm")
        color: Theme.bright
        font.weight: Font.Medium
    }

    BarText {
        anchors.verticalCenter: parent.verticalCenter
        text: Qt.formatDateTime(clock.date, "ddd dd MMM").toLowerCase()
        color: Theme.label
    }
}
