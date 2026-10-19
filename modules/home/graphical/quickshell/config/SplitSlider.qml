import qs
import QtQuick
import QtQuick.Layouts

// Slider whose track breaks around a bar-shaped handle, with an icon riding in the fill.
// White fill by design (not the accent). Drag, click or scroll; emits moved(value 0..1).
Item {
    id: root

    property real value: 0
    property string icon: ""
    property color fillColor: Theme.white
    property bool enabled: true
    readonly property int gap: 6

    signal moved(real value)

    Layout.fillWidth: true
    implicitWidth: 200
    implicitHeight: 46
    opacity: enabled ? 1 : 0.5

    readonly property real clamped: Math.max(0, Math.min(1, value))
    readonly property real usable: width - gap * 2 - 4
    // The icon rides in the fill while it fits there, otherwise it moves to the empty track
    // just past the handle (like Android's sliders).
    readonly property bool iconInFill: fill.width >= 34

    Rectangle {
        id: fill
        anchors.verticalCenter: parent.verticalCenter
        x: 0
        width: Math.max(0, root.usable * root.clamped)
        height: 36
        // Round outer end, nearly square inner end against the handle.
        topLeftRadius: 12
        bottomLeftRadius: 12
        topRightRadius: 4
        bottomRightRadius: 4
        color: root.fillColor
        clip: true

        Icon {
            visible: root.icon !== "" && root.iconInFill
            anchors.verticalCenter: parent.verticalCenter
            x: 10
            name: root.icon
            size: 16
            strokeWidth: 2.2
            color: Theme.ink
        }
    }

    Rectangle {
        id: handle
        anchors.verticalCenter: parent.verticalCenter
        x: fill.width + root.gap
        width: 4
        height: 46
        radius: 2
        color: Theme.white
    }

    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        x: handle.x + handle.width + root.gap
        width: Math.max(0, root.width - x)
        height: 36
        topLeftRadius: 4
        bottomLeftRadius: 4
        topRightRadius: 12
        bottomRightRadius: 12
        color: Theme.track
        clip: true

        Icon {
            visible: root.icon !== "" && !root.iconInFill
            anchors.verticalCenter: parent.verticalCenter
            x: 10
            name: root.icon
            size: 16
            strokeWidth: 2.2
            color: Theme.text
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.enabled
        cursorShape: Qt.PointingHandCursor
        function at(x) {
            return Math.max(0, Math.min(1, (x - root.gap) / root.usable));
        }
        onPressed: mouse => root.moved(at(mouse.x))
        onPositionChanged: mouse => {
            if (pressed)
                root.moved(at(mouse.x));
        }
        onWheel: wheel => root.moved(Math.max(0, Math.min(1, root.value + (wheel.angleDelta.y > 0 ? 0.02 : -0.02))))
    }
}
