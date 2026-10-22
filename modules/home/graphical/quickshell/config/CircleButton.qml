import qs
import QtQuick

// Round icon-only button (back arrow, copy, media controls, power menu, ...).
MouseArea {
    id: root

    property string icon: ""
    property int size: 28
    property int iconSize: 15
    property color iconColor: Theme.textDim
    // Theme.warnText for buttons that delete something.
    property color hoverIconColor: Theme.text
    property color restColor: "transparent"

    implicitWidth: size
    implicitHeight: size
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: root.containsMouse ? Theme.fillHover : root.restColor
    }
    Icon {
        anchors.centerIn: parent
        name: root.icon
        size: root.iconSize
        strokeWidth: 2.1
        color: root.containsMouse ? root.hoverIconColor : root.iconColor
    }
}
