import qs
import QtQuick

// Small rounded text button, optionally with an icon. `accent` for the primary action.
MouseArea {
    id: root

    property string text: ""
    property string icon: ""
    property bool accent: false
    property bool checked: false
    property int pillHeight: 26

    implicitWidth: row.implicitWidth + 24
    implicitHeight: pillHeight
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.accent ? Theme.accent : root.checked || root.containsMouse ? Theme.fillActive : Theme.fillHover
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6

        Icon {
            visible: root.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            size: 13
            strokeWidth: 2.1
            color: root.accent ? Theme.accentInk : Theme.text
        }
        UiText {
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            color: root.accent ? Theme.accentInk : Theme.text
            font.pixelSize: Theme.small
            font.weight: Font.Medium
        }
    }
}
