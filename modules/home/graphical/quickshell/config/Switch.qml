import qs
import QtQuick

// On/off switch. Emits toggled(); the owner flips `checked` from its own state.
MouseArea {
    id: root

    property bool checked: false
    property color checkedColor: Theme.accent

    signal toggled

    implicitWidth: 38
    implicitHeight: 22
    cursorShape: Qt.PointingHandCursor
    onClicked: toggled()

    Accessible.role: Accessible.CheckBox
    Accessible.checked: checked

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.checked ? root.checkedColor : Qt.rgba(1, 1, 1, 0.2)
        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }

        Rectangle {
            width: 18
            height: 18
            radius: 9
            y: 2
            x: root.checked ? parent.width - width - 2 : 2
            // Dark on the white "on" track, white on the grey "off" one.
            color: root.checked ? Theme.ink : "white"
            Behavior on x {
                NumberAnimation {
                    duration: Theme.fast
                    easing.type: Easing.OutCubic
                }
            }
        }
    }
}
