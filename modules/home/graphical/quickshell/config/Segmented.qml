pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts

// Segmented control, e.g. the power profile. Emits activated(index).
Rectangle {
    id: root

    property var model: []
    property int currentIndex: 0

    signal activated(int index)

    Layout.fillWidth: true
    implicitHeight: 36
    radius: 12
    color: Theme.fill

    RowLayout {
        anchors.fill: parent
        anchors.margins: 3
        spacing: 2

        Repeater {
            model: root.model

            MouseArea {
                id: seg

                required property string modelData
                required property int index
                readonly property bool on: index === root.currentIndex

                Layout.fillWidth: true
                Layout.fillHeight: true
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.activated(index)

                Rectangle {
                    anchors.fill: parent
                    radius: 9
                    color: seg.on ? Theme.fillActive : seg.containsMouse ? Theme.fill : "transparent"
                }
                UiText {
                    anchors.centerIn: parent
                    text: seg.modelData
                    color: seg.on ? "white" : Theme.textDim
                    font.pixelSize: Theme.small
                    font.weight: Font.Medium
                }
            }
        }
    }
}
