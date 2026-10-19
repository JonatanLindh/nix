import qs
import QtQuick
import QtQuick.Layouts

// A clickable row in a panel list: optional leading icon (in a circle when `iconCircle`),
// text, dim detail, and trailing children. `selected` shows a check / accent circle.
MouseArea {
    id: root

    property string icon: ""
    property bool iconCircle: false
    property string text: ""
    property string detail: ""
    property bool selected: false
    property bool dimmed: false
    property bool showCheck: false
    default property alias trailing: trailingRow.data

    Layout.fillWidth: true
    implicitHeight: 40
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    Rectangle {
        anchors.fill: parent
        radius: Theme.rowRadius
        color: root.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        spacing: 12
        opacity: root.dimmed ? 0.45 : 1

        Icon {
            visible: root.showCheck
            name: "check"
            size: 16
            strokeWidth: 2.6
            color: Theme.accent
            opacity: root.selected ? 1 : 0
        }

        Item {
            visible: root.icon !== ""
            implicitWidth: root.iconCircle ? 28 : 18
            implicitHeight: implicitWidth

            Rectangle {
                visible: root.iconCircle
                anchors.fill: parent
                radius: width / 2
                color: root.selected ? Theme.accent : Theme.fillHover
            }
            Icon {
                anchors.centerIn: parent
                name: root.icon
                size: root.iconCircle ? 15 : 18
                strokeWidth: root.iconCircle ? 2.2 : 1.9
                color: root.iconCircle && root.selected ? Theme.accentInk : Theme.text
            }
        }

        UiText {
            Layout.fillWidth: true
            text: root.text
        }

        UiText {
            visible: root.detail !== ""
            text: root.detail
            dim: true
            font.pixelSize: Theme.small
        }

        Row {
            id: trailingRow
            spacing: 8
        }
    }
}
