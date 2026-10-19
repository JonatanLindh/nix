import qs
import QtQuick
import QtQuick.Layouts

// Text field on glass. `password` adds a show/hide eye.
Rectangle {
    id: root

    property alias text: input.text
    property alias input: input
    property string placeholder: ""
    property bool password: false
    property bool revealed: false

    signal accepted

    Layout.fillWidth: true
    implicitHeight: 32
    radius: 9
    color: Qt.rgba(0, 0, 0, 0.25)
    border.width: 1
    border.color: input.activeFocus ? Qt.rgba(1, 1, 1, 0.4) : Theme.glassBorder

    TextInput {
        id: input
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: root.password ? 36 : 12
        verticalAlignment: TextInput.AlignVCenter
        color: Theme.text
        selectionColor: Theme.accent
        selectedTextColor: Theme.accentInk
        font.family: Theme.font
        font.pixelSize: Theme.fontSize
        echoMode: root.password && !root.revealed ? TextInput.Password : TextInput.Normal
        clip: true
        renderType: Text.NativeRendering
        onAccepted: root.accepted()

        UiText {
            anchors.fill: parent
            visible: input.text === ""
            text: root.placeholder
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.5)
        }
    }

    CircleButton {
        visible: root.password
        anchors.right: parent.right
        anchors.rightMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        size: 24
        iconSize: 16
        icon: root.revealed ? "eye-off" : "eye"
        onClicked: root.revealed = !root.revealed
    }
}
