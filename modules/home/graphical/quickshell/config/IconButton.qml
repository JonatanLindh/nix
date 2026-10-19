import qs
import QtQuick

// A bar item: an icon (and optionally text or other content after it) on a rounded hover
// target. `active` keeps the highlight while its panel is open. Clicked with any button.
MouseArea {
    id: root

    property string icon: ""
    property color iconColor: Theme.text
    property int iconSize: 18
    property string text: ""
    property color textColor: Theme.text
    property bool active: false
    // Small count bubble in the top-right corner ("" = none).
    property string badge: ""
    // Background when not hovered/active (e.g. a tinted "on" state).
    property color restColor: "transparent"
    default property alias content: row.data

    implicitWidth: row.implicitWidth + 16
    implicitHeight: 26
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton

    Rectangle {
        anchors.fill: parent
        radius: Theme.buttonRadius
        color: root.active ? Theme.fillActive : root.containsMouse ? Theme.fillHover : root.restColor
        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6

        Icon {
            visible: root.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            size: root.iconSize
            color: root.iconColor
        }

        UiText {
            visible: root.text !== ""
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            color: root.textColor
            font.weight: Font.Medium
            font.features: {
                "tnum": 1
            }
        }
    }

    Rectangle {
        visible: root.badge !== ""
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.topMargin: 1
        anchors.rightMargin: 1
        width: Math.max(14, badgeText.implicitWidth + 6)
        height: 14
        radius: 7
        color: Theme.white

        UiText {
            id: badgeText
            anchors.centerIn: parent
            text: root.badge
            color: Theme.ink
            font.pixelSize: 10 // 9.5 on the canvas; pixelSize is an int
            font.weight: Font.DemiBold
        }
    }
}
