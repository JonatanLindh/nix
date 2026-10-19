import qs
import QtQuick

// One pill in the bar while something records the camera, microphone or screen, with an
// icon for each. Each in its own colour, so they stand out from the settings the user
// turned on (which use the accent) and from each other.
Rectangle {
    visible: Privacy.mic || Privacy.camera || Privacy.screen
    anchors.verticalCenter: parent?.verticalCenter
    implicitWidth: row.implicitWidth + (row.visibleChildren.length > 1 ? 16 : 18)
    implicitHeight: 22
    radius: 11
    color: Qt.rgba(1, 1, 1, 0.12)

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 7

        Icon {
            visible: Privacy.camera
            anchors.verticalCenter: parent.verticalCenter
            name: "camera"
            size: 17
            strokeWidth: 2.1
            color: Theme.camera
        }

        Icon {
            visible: Privacy.mic
            anchors.verticalCenter: parent.verticalCenter
            name: "mic"
            size: 14
            strokeWidth: 2.2
            color: Theme.mic
        }

        Icon {
            visible: Privacy.screen
            anchors.verticalCenter: parent.verticalCenter
            name: "monitor"
            size: 15
            strokeWidth: 2.2
            color: Theme.screen
        }
    }
}
