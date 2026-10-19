pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts

// 20 segments in 5% steps (the OSD). `muted` dims the filled ones.
RowLayout {
    id: root

    property real value: 0
    property bool muted: false
    property int segments: 20
    property int segmentHeight: 18

    spacing: 3

    Repeater {
        model: root.segments

        Rectangle {
            required property int index
            readonly property bool on: (index + 1) / root.segments <= root.value + 0.0001

            Layout.fillWidth: true
            implicitHeight: root.segmentHeight
            radius: index === 0 || index === root.segments - 1 ? 7 : 3
            color: on ? (root.muted ? Qt.rgba(244 / 255, 247 / 255, 246 / 255, 0.32) : Theme.white) : Theme.track
        }
    }
}
