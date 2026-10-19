import qs
import QtQuick
import QtQuick.Layouts

// Small dim heading above a group in a panel, with optional controls on the right.
RowLayout {
    id: root

    property string text: ""
    default property alias controls: controlsRow.data

    Layout.fillWidth: true
    Layout.leftMargin: 10
    Layout.rightMargin: 4
    Layout.topMargin: 4
    spacing: 8

    UiText {
        Layout.fillWidth: true
        text: root.text
        dim: true
        font.pixelSize: Theme.small
        font.weight: Font.Medium
    }

    Row {
        id: controlsRow
        spacing: 6
    }
}
