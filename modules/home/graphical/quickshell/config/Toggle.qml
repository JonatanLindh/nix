import qs
import QtQuick

// "on / off" text switch; the active side is highlighted.
MouseArea {
    id: root

    property bool checked: false
    property string onText: "on"
    property string offText: "off"

    signal toggled

    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight
    cursorShape: Qt.PointingHandCursor
    onClicked: toggled()

    Row {
        id: row

        BarText {
            text: root.onText
            color: root.checked ? Theme.accent : Theme.label
        }

        BarText {
            text: " / "
            color: Theme.label
        }

        BarText {
            text: root.offText
            color: root.checked ? Theme.label : Theme.bright
        }
    }
}
