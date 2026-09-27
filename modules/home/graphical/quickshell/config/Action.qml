import qs
import QtQuick

// Dim clickable text, e.g. "rescan" or "pavucontrol →".
MouseArea {
    id: root

    property string text: ""

    implicitWidth: label.implicitWidth
    implicitHeight: label.implicitHeight
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    BarText {
        id: label
        text: root.text
        color: root.containsMouse ? Theme.bright : Theme.label
    }
}
