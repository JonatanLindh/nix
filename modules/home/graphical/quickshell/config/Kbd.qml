import qs
import QtQuick

// A key cap for hints: "esc", "↵", "ctrl".
Rectangle {
    property alias text: label.text

    implicitWidth: Math.max(18, label.implicitWidth + 10)
    implicitHeight: 20
    radius: 5
    color: Qt.rgba(1, 1, 1, 0.1)
    border.width: 1
    border.color: Qt.rgba(1, 1, 1, 0.1)

    UiText {
        id: label
        anchors.centerIn: parent
        mono: true
        font.pixelSize: Theme.tiny
    }
}
