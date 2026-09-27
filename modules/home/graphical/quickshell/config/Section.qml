import qs
import QtQuick
import QtQuick.Layouts

// Dim heading inside a panel, with an optional value or control on the right.
RowLayout {
    id: root

    property string text: ""
    default property alias controls: controlsRow.data

    Layout.fillWidth: true
    Layout.leftMargin: Theme.panelPadding
    Layout.rightMargin: Theme.panelPadding
    Layout.topMargin: 18
    Layout.bottomMargin: 6

    BarText {
        text: root.text
        color: Theme.label
    }

    Item {
        Layout.fillWidth: true
    }

    Row {
        id: controlsRow
        spacing: 14
    }
}
