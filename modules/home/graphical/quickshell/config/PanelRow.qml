import qs
import QtQuick
import QtQuick.Layouts

// Selectable list row: "* name          detail".
MouseArea {
    id: root

    property string text: ""
    property string detail: ""
    property bool selected: false
    property bool dimmed: false
    property color detailColor: Theme.label
    default property alias extra: rightRow.data

    Layout.fillWidth: true
    implicitHeight: 30
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    Rectangle {
        anchors.fill: parent
        color: root.containsMouse ? Theme.hover : "transparent"
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.panelPadding
        anchors.rightMargin: Theme.panelPadding
        spacing: 8

        BarText {
            Layout.preferredWidth: 8
            text: root.selected ? "*" : ""
            color: Theme.accent
        }

        BarText {
            Layout.fillWidth: true
            text: root.text
            elide: Text.ElideRight
            color: root.dimmed ? Theme.off : root.selected ? Theme.bright : Theme.text
        }

        BarText {
            visible: root.detail !== ""
            text: root.detail
            color: root.detailColor
        }

        Row {
            id: rightRow
            spacing: 12
        }
    }
}
