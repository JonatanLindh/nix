pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts

// "label            64" over a block bar. Click or drag to set when interactive.
ColumnLayout {
    id: root

    property string label: ""
    property string valueText: Math.round(value * 100)
    property real value: 0
    property bool interactive: true
    property color fill: Theme.accent
    readonly property int cells: 25

    signal moved(real value)

    Layout.fillWidth: true
    Layout.leftMargin: Theme.panelPadding
    Layout.rightMargin: Theme.panelPadding
    Layout.topMargin: 18
    spacing: 8

    RowLayout {
        BarText {
            text: root.label
            color: Theme.label
        }

        Item {
            Layout.fillWidth: true
        }

        BarText {
            text: root.valueText
            color: Theme.bright
        }
    }

    Item {
        implicitWidth: meter.implicitWidth
        implicitHeight: meter.implicitHeight

        // Cells drawn as rectangles rather than "█" glyphs, which get subpixel colour fringes.
        Row {
            id: meter

            readonly property int filled: Math.round(Math.max(0, Math.min(1, root.value)) * root.cells)

            spacing: 2

            Repeater {
                model: root.cells

                Rectangle {
                    required property int index

                    width: cell.advanceWidth - 1
                    height: Theme.fontSize
                    color: index < meter.filled ? root.fill : Theme.track
                }
            }

            TextMetrics {
                id: cell
                font.family: Theme.font
                font.pixelSize: Theme.fontSize
                text: "0"
            }
        }

        // Sibling of the row, not a child, so it doesn't feed back into the row's width.
        MouseArea {
            anchors.fill: parent
            enabled: root.interactive
            cursorShape: Qt.PointingHandCursor
            onPressed: mouse => root.moved(mouse.x / width)
            onPositionChanged: mouse => {
                if (pressed)
                    root.moved(Math.max(0, Math.min(1, mouse.x / width)));
            }
            onWheel: wheel => root.moved(root.value + (wheel.angleDelta.y > 0 ? 0.02 : -0.02))
        }
    }
}
