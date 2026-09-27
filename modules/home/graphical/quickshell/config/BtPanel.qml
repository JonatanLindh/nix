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
    readonly property var connected: devices.filter(d => d.connected)
    readonly property var paired: devices.filter(d => !d.connected && (d.paired || d.bonded))
    readonly property var nearby: adapter?.discovering ? devices.filter(d => !d.connected && !d.paired && !d.bonded) : []

    name: "bt"
    title: "bt"

    onVisibleChanged: if (!visible && adapter?.discovering)
        adapter.discovering = false

    actions: [
        BarText {
            text: "power"
            color: Theme.label
        },
        Toggle {
            checked: root.adapter?.enabled ?? false
            onToggled: root.adapter.enabled = !root.adapter.enabled
        }
    ]

    Section {
        visible: root.connected.length > 0
        text: "connected"
    }

    Repeater {
        model: root.connected

        PanelRow {
            id: device

            required property BluetoothDevice modelData
            text: modelData.name
            selected: true
            detail: modelData.batteryAvailable ? Math.round(modelData.battery * 100) + "%" : ""
            cursorShape: Qt.ArrowCursor

            Action {
                text: "disconnect"
                onClicked: device.modelData.disconnect()
            }
        }
    }

    Section {
        visible: root.paired.length > 0
        text: "paired"
    }

    Repeater {
        model: root.paired

        PanelRow {
            required property BluetoothDevice modelData
            text: modelData.name
            // qmllint disable unresolved-type
            detail: modelData.state === BluetoothDeviceState.Connecting ? "…" : "connect"
            // qmllint enable unresolved-type
            onClicked: modelData.connect()
        }
    }

    Section {
        visible: root.adapter?.discovering ?? false
        text: "nearby"
    }

    Repeater {
        model: root.nearby

        PanelRow {
            required property BluetoothDevice modelData
            text: modelData.name
            detail: modelData.pairing ? "…" : "pair"
            onClicked: modelData.pair()
        }
    }

    Note {
        visible: !(root.adapter?.enabled ?? false)
        Layout.topMargin: 18
        text: "bluetooth is off"
    }

    footer: [
        Action {
            visible: root.adapter?.enabled ?? false
            text: root.adapter?.discovering ? "stop scan" : "scan"
            onClicked: root.adapter.discovering = !root.adapter.discovering
        },
        Item {
            Layout.fillWidth: true
        },
        Action {
            text: "blueman →"
            onClicked: {
                Quickshell.execDetached(["blueman-manager"]);
                root.close();
            }
        }
    ]
}
