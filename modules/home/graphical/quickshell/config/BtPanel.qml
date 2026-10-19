pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Bluetooth

Panel {
    id: root

    // qmllint disable unresolved-type
    readonly property BluetoothAdapter adapter: Bluetooth.defaultAdapter
    readonly property var devices: adapter ? adapter.devices.values.filter(d => d.name) : []
    // qmllint enable unresolved-type
    readonly property bool on: adapter?.enabled ?? false
    readonly property var connected: devices.filter(d => d.connected)
    readonly property var paired: devices.filter(d => !d.connected && (d.paired || d.bonded))
    readonly property var nearby: adapter?.discovering ? devices.filter(d => !d.connected && !d.paired && !d.bonded) : []

    // BlueZ icon name (freedesktop names, e.g. "audio-headset") -> our icon.
    function deviceIcon(d) {
        const i = d?.icon ?? "";
        if (/headset|headphone/.test(i))
            return "headphones";
        if (/speaker|audio-card/.test(i))
            return "speaker";
        if (/mouse|tablet/.test(i))
            return "mouse";
        if (/keyboard/.test(i))
            return "keyboard";
        if (/gaming|joystick/.test(i))
            return "gamepad";
        if (/phone/.test(i))
            return "phone";
        if (/computer|laptop/.test(i))
            return "laptop";
        return "bluetooth";
    }

    name: "bt"
    title: "Bluetooth"

    onVisibleChanged: if (!visible && adapter?.discovering)
        adapter.discovering = false

    actions: Switch {
        visible: root.adapter !== null
        checked: root.on
        onToggled: root.adapter.enabled = !root.adapter.enabled
    }

    ColumnLayout {
        visible: root.on && root.connected.length > 0
        Layout.fillWidth: true
        spacing: 8

        Repeater {
            model: root.on ? root.connected : []

            Rectangle {
                id: device

                required property BluetoothDevice modelData

                Layout.fillWidth: true
                implicitHeight: 54
                radius: Theme.tileRadius
                color: Theme.fill

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 12

                    Rectangle {
                        implicitWidth: 34
                        implicitHeight: 34
                        radius: 17
                        color: Theme.accent

                        Icon {
                            anchors.centerIn: parent
                            name: root.deviceIcon(device.modelData)
                            size: 15
                            strokeWidth: 2.2
                            color: Theme.accentInk
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        UiText {
                            Layout.fillWidth: true
                            text: device.modelData.name
                            font.weight: Font.DemiBold
                        }
                        Row {
                            spacing: 6

                            // Battery glyph, filled to the charge.
                            Item {
                                visible: device.modelData.batteryAvailable
                                anchors.verticalCenter: parent.verticalCenter
                                width: 20
                                height: 10

                                Rectangle {
                                    width: 17
                                    height: 10
                                    radius: 3
                                    color: "transparent"
                                    border.width: 1
                                    border.color: Theme.textDim

                                    Rectangle {
                                        x: 2
                                        y: 2
                                        width: Math.max(2, Math.round(13 * device.modelData.battery))
                                        height: 6
                                        radius: 1.5
                                        color: Theme.textDim
                                    }
                                }
                                Rectangle {
                                    x: 18
                                    y: 3.5
                                    width: 2
                                    height: 3
                                    radius: 1
                                    color: Theme.textDim
                                }
                            }
                            UiText {
                                anchors.verticalCenter: parent.verticalCenter
                                text: (device.modelData.batteryAvailable ? Math.round(device.modelData.battery * 100) + "% · " : "") + "Connected"
                                dim: true
                                font.pixelSize: Theme.small
                            }
                        }
                    }
                    PillButton {
                        text: "Disconnect"
                        onClicked: device.modelData.disconnect()
                    }
                }
            }
        }
    }

    SectionLabel {
        visible: root.on && root.paired.length > 0
        text: "Paired"
    }

    ColumnLayout {
        visible: root.on && root.paired.length > 0
        Layout.fillWidth: true
        Layout.topMargin: -6
        spacing: 2

        Repeater {
            model: root.on ? root.paired : []

            ListRow {
                id: pairedRow

                required property BluetoothDevice modelData

                icon: root.deviceIcon(modelData)
                iconCircle: true
                text: modelData.name
                onClicked: modelData.connect()

                UiText {
                    // qmllint disable unresolved-type
                    visible: pairedRow.modelData.state === BluetoothDeviceState.Connecting
                    // qmllint enable unresolved-type
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Connecting…"
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                    font.pixelSize: Theme.small
                }
            }
        }
    }

    SectionLabel {
        visible: root.on
        text: "Nearby"

        // Scan toggle; a dot marks it while scanning.
        MouseArea {
            id: scan

            readonly property bool scanning: root.adapter?.discovering ?? false

            implicitWidth: scanRow.implicitWidth + 20
            implicitHeight: 24
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.adapter.discovering = !root.adapter.discovering

            Rectangle {
                anchors.fill: parent
                radius: height / 2
                color: scan.containsMouse ? Theme.fillActive : Theme.fillHover
            }
            Row {
                id: scanRow
                anchors.centerIn: parent
                spacing: 6

                Rectangle {
                    visible: scan.scanning
                    anchors.verticalCenter: parent.verticalCenter
                    width: 8
                    height: 8
                    radius: 4
                    color: Theme.accent
                    border.width: 0

                    Rectangle {
                        anchors.centerIn: parent
                        z: -1
                        width: 14
                        height: 14
                        radius: 7
                        color: Qt.alpha(Theme.accent, 0.3)
                    }
                }
                UiText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: scan.scanning ? "Scanning" : "Scan"
                    font.pixelSize: Theme.small
                    font.weight: Font.Medium
                }
            }
        }
    }

    ColumnLayout {
        visible: root.nearby.length > 0
        Layout.fillWidth: true
        Layout.topMargin: -4
        spacing: 2

        Repeater {
            model: root.on ? root.nearby : []

            ListRow {
                id: nearbyRow

                required property BluetoothDevice modelData

                icon: root.deviceIcon(modelData)
                iconCircle: true
                dimmed: modelData.pairing
                text: modelData.name
                onClicked: modelData.pair()

                UiText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: nearbyRow.modelData.pairing ? "Pairing…" : "Pair"
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                    font.pixelSize: Theme.small
                }
            }
        }
    }

    UiText {
        visible: !root.on
        Layout.fillWidth: true
        Layout.topMargin: 18
        Layout.bottomMargin: 18
        horizontalAlignment: Text.AlignHCenter
        text: root.adapter ? "Bluetooth is off" : "No Bluetooth adapter"
        dim: true
    }

    footer: ListRow {
        implicitHeight: 34
        text: "Bluetooth settings…"
        onClicked: {
            Quickshell.execDetached(["blueman-manager"]);
            root.close();
        }

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: "chevron-right"
            size: 14
            strokeWidth: 2
            color: Theme.textDim
        }
    }
}
