import qs
import QtQuick
import Quickshell

// "Mon 19 Oct  14:21" in the middle of the bar; a button for the calendar.
MouseArea {
    id: root

    property bool active: false

    implicitWidth: row.implicitWidth + 24
    implicitHeight: 26
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.buttonRadius
        color: root.active ? Theme.fillActive : root.containsMouse ? Theme.fillHover : "transparent"
        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 10

        UiText {
            text: Qt.formatDateTime(clock.date, "ddd d MMM")
            color: Qt.alpha(Theme.text, 0.72)
            font.weight: Font.Medium
        }

        UiText {
            text: Qt.formatDateTime(clock.date, "HH:mm")
            font.weight: Font.DemiBold
            font.letterSpacing: 0.13
            font.features: {
                "tnum": 1
            }
        }
    }
}
