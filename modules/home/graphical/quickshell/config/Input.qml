import qs
import QtQuick

// Single-line text field with a hairline underline and dim placeholder.
TextInput {
    id: root

    property string placeholder: ""

    topPadding: 4
    bottomPadding: 4
    color: Theme.bright
    selectionColor: Theme.accent
    font.family: Theme.font
    font.pixelSize: Theme.fontSize
    verticalAlignment: TextInput.AlignVCenter
    clip: true
    renderType: Text.CurveRendering

    BarText {
        anchors.fill: parent
        visible: root.text === "" && !root.activeFocus
        text: root.placeholder
        color: Theme.off
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: root.activeFocus ? Theme.accent : Theme.border
    }
}
