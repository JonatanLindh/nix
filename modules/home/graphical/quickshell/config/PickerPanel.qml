pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts

// The colour picker's panel: the current colour in four copyable formats, the history,
// and a way to take the slot back out of the bar.
Panel {
    id: root

    // Which format row was just copied ("" = none).
    property string copied: ""

    name: "picker"
    side: "left"
    panelWidth: 340

    onVisibleChanged: copied = ""

    Connections {
        target: ColorPicker
        function onHexChanged() {
            root.copied = "";
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.leftMargin: 6
        Layout.rightMargin: 2
        Layout.topMargin: 2
        spacing: 8

        Icon {
            name: "dropper"
            size: 17
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.75)
        }
        UiText {
            Layout.fillWidth: true
            text: "Colour picker"
            font.pixelSize: Theme.title
            font.weight: Font.DemiBold
        }
        PillButton {
            icon: "dropper"
            text: "Pick"
            onClicked: ColorPicker.pick()
        }
    }

    Tile {
        padding: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 14

            Rectangle {
                implicitWidth: 56
                implicitHeight: 56
                radius: 14
                color: ColorPicker.hex
                border.width: 1
                border.color: Qt.rgba(1, 1, 1, 0.18)
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                UiText {
                    text: ColorPicker.hex.toUpperCase()
                    mono: true
                    font.pixelSize: 20
                    font.weight: Font.Medium
                    font.letterSpacing: 0.4
                }
                UiText {
                    Layout.fillWidth: true
                    text: ColorPicker.error !== "" ? ColorPicker.error : ColorPicker.history.length === 0 ? "Nothing picked yet" : ColorPicker.pickedAt > 0 ? "Picked just now · copied as hex" : "From history"
                    color: ColorPicker.error !== "" ? Theme.warnText : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                    font.pixelSize: Theme.small
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Repeater {
                model: ColorPicker.formats

                MouseArea {
                    id: format

                    required property var modelData

                    Layout.fillWidth: true
                    implicitHeight: 34
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        ColorPicker.copy(modelData[1]);
                        root.copied = modelData[0];
                    }

                    Rectangle {
                        anchors.fill: parent
                        radius: Theme.rowRadius
                        color: format.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
                    }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 12

                        UiText {
                            Layout.preferredWidth: 44
                            text: format.modelData[0]
                            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
                            font.pixelSize: Theme.tiny
                            font.weight: Font.DemiBold
                        }
                        UiText {
                            Layout.fillWidth: true
                            text: format.modelData[1]
                            mono: true
                            font.pixelSize: 13
                        }
                        UiText {
                            visible: root.copied === format.modelData[0]
                            text: "Copied"
                            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                            font.pixelSize: Theme.tiny
                        }
                        Icon {
                            name: root.copied === format.modelData[0] ? "check" : "copy"
                            size: 13
                            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, format.containsMouse || root.copied === format.modelData[0] ? 0.8 : 0.45)
                        }
                    }
                }
            }
        }
    }

    RowLayout {
        visible: ColorPicker.history.length > 0
        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 4
        Layout.topMargin: 4

        UiText {
            Layout.fillWidth: true
            text: "History"
            color: Theme.textDim
            font.pixelSize: Theme.small
            font.weight: Font.Medium
        }
        MouseArea {
            id: clear

            implicitWidth: clearText.implicitWidth + 12
            implicitHeight: clearText.implicitHeight + 4
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: ColorPicker.clear()

            Rectangle {
                anchors.fill: parent
                radius: 6
                color: clear.containsMouse ? Theme.fillHover : "transparent"
            }
            UiText {
                id: clearText
                anchors.centerIn: parent
                text: "Clear"
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.72)
                font.pixelSize: Theme.small
            }
        }
    }

    GridLayout {
        id: historyGrid

        visible: ColorPicker.history.length > 0
        Layout.fillWidth: true
        Layout.leftMargin: 6
        Layout.rightMargin: 6
        columns: 8
        rowSpacing: 6
        columnSpacing: 6

        Repeater {
            model: ColorPicker.history

            MouseArea {
                id: swatch

                required property string modelData
                required property int index
                readonly property bool on: index === ColorPicker.current

                // Eight to a row, however many there are.
                readonly property real side: Math.floor((root.panelWidth - Theme.panelPadding * 2 - 12 - historyGrid.columnSpacing * 7) / 8)

                Layout.preferredWidth: side
                Layout.preferredHeight: side
                cursorShape: Qt.PointingHandCursor
                onClicked: ColorPicker.select(index)

                // Selected: a dark gap, then a light ring.
                Rectangle {
                    visible: swatch.on
                    anchors.fill: parent
                    anchors.margins: -4
                    radius: 13
                    color: "transparent"
                    border.width: 2
                    border.color: Theme.white
                }
                Rectangle {
                    anchors.fill: parent
                    radius: 9
                    color: swatch.modelData
                    border.width: swatch.on ? 0 : 1
                    border.color: Qt.rgba(1, 1, 1, 0.18)
                }
            }
        }
    }

    footer: [
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 10
            Layout.rightMargin: 4
            spacing: 8

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4

                UiText {
                    Layout.fillWidth: true
                    text: "Right-click the swatch to pick"
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                    font.pixelSize: Theme.tiny
                }
                Row {
                    spacing: 4
                    Kbd {
                        text: "super"
                    }
                    Kbd {
                        text: "⇧"
                    }
                    Kbd {
                        text: "C"
                    }
                }
            }
            PillButton {
                text: "Remove from bar"
                onClicked: {
                    root.close();
                    ColorPicker.remove();
                }
            }
        }
    ]
}
