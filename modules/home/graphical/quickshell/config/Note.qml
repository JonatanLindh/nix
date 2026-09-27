import qs
import QtQuick
import QtQuick.Layouts

// Dim secondary line inside a panel, indented under a row.
BarText {
    Layout.fillWidth: true
    Layout.leftMargin: Theme.panelPadding + 16
    Layout.rightMargin: Theme.panelPadding
    color: Theme.label
    lineHeight: 1.5
    wrapMode: Text.WordWrap
}
