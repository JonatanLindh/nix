import qs
import QtQuick

// One "label value" item in the bar. Extra children go after the value.
MouseArea {
    id: root

    property string label: ""
    property string value: ""
    property color valueColor: Theme.text
    // Fixed width in characters (0 = natural), so changing values don't shift the bar.
    property int cols: 0
    property bool alignRight: true
    property bool bold: false
    // Underlined while its panel is open.
    property bool open: false
    default property alias extra: row.data

    implicitWidth: row.implicitWidth
    implicitHeight: Theme.barHeight
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    cursorShape: Qt.PointingHandCursor

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter

        BarText {
            visible: root.label !== ""
            text: root.label + " "
            color: Theme.label
        }

        BarText {
            id: valueText

            width: root.cols > 0 ? Math.ceil(metrics.advanceWidth * root.cols) : implicitWidth
            horizontalAlignment: root.alignRight ? Text.AlignRight : Text.AlignLeft
            elide: Text.ElideRight
            text: root.value
            color: root.valueColor
            font.bold: root.bold

            TextMetrics {
                id: metrics
                font: valueText.font
                text: "0"
            }
        }
    }

    Rectangle {
        visible: root.open
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 5
        width: parent.width
        height: 1
        color: Theme.accent
    }
}
