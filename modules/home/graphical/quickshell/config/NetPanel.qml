pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Networking

Panel {
    id: root

    name: "net"
    title: "net"

    function securityLabel(s) {
        switch (s) {
        case WifiSecurityType.Open:
            return "open";
        case WifiSecurityType.Owe:
            return "owe";
        case WifiSecurityType.StaticWep:
        case WifiSecurityType.DynamicWep:
            return "wep";
        case WifiSecurityType.WpaPsk:
            return "wpa";
        case WifiSecurityType.Wpa2Psk:
            return "wpa2";
        case WifiSecurityType.Sae:
            return "wpa3";
        case WifiSecurityType.WpaEap:
        case WifiSecurityType.Wpa2Eap:
        case WifiSecurityType.Wpa3SuiteB192:
        case WifiSecurityType.Leap:
            return "802.1x";
        default:
            return "";
        }
    }

    // Scan only while the panel is open.
    onVisibleChanged: if (Net.wifi)
        Net.wifi.scannerEnabled = visible

    actions: [
        BarText {
            visible: Net.wifi !== null
            text: "wifi"
            color: Theme.label
        },
        Toggle {
            visible: Net.wifi !== null
            checked: Networking.wifiEnabled
            onToggled: Networking.wifiEnabled = !Networking.wifiEnabled
        }
    ]

    Section {
        visible: Net.wired.length > 0
        text: "wired"
    }

    Repeater {
        model: Net.wired

        PanelRow {
            required property var modelData
            text: modelData.name
            selected: modelData.connected
            detail: modelData.connected ? "up" : "down"
            cursorShape: Qt.ArrowCursor
        }
    }

    Section {
        visible: Net.wifi !== null && Networking.wifiEnabled
        text: "wifi"
    }

    Repeater {
        model: Net.wifi && Networking.wifiEnabled ? Net.wifi.networks.values.slice().sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 8) : []

        ColumnLayout {
            id: entry

            required property WifiNetwork modelData
            // qmllint disable unresolved-type
            readonly property bool secured: modelData.security !== WifiSecurityType.Open && modelData.security !== WifiSecurityType.Owe
            // qmllint enable unresolved-type
            property bool asking: false
            property bool failed: false

            Layout.fillWidth: true
            spacing: 0

            Connections {
                target: entry.modelData
                function onConnectionFailed() {
                    entry.failed = true;
                }
            }

            PanelRow {
                text: entry.modelData.name
                selected: entry.modelData.connected
                // qmllint disable unresolved-type
                detail: entry.modelData.stateChanging ? "…" : entry.failed ? "failed" : entry.modelData.connected ? "" : entry.modelData.known ? "saved" : root.securityLabel(entry.modelData.security)
                // qmllint enable unresolved-type
                detailColor: entry.failed ? Theme.warn : Theme.label
                onClicked: {
                    entry.failed = false;
                    if (entry.modelData.connected)
                        return;
                    if (entry.modelData.known || !entry.secured)
                        entry.modelData.connect();
                    else
                        entry.asking = !entry.asking;
                }

                Action {
                    visible: entry.modelData.connected
                    text: "disconnect"
                    onClicked: entry.modelData.disconnect()
                }

                BarText {
                    text: Net.bars(entry.modelData.signalStrength)
                    color: entry.modelData.connected ? Theme.accent : Theme.text
                }
            }

            RowLayout {
                visible: entry.asking
                Layout.fillWidth: true
                Layout.leftMargin: Theme.panelPadding + 16
                Layout.rightMargin: Theme.panelPadding
                Layout.bottomMargin: 6
                spacing: 8

                BarText {
                    text: "psk"
                    color: Theme.label
                }

                TextInput {
                    id: psk

                    Layout.fillWidth: true
                    color: Theme.bright
                    selectionColor: Theme.accent
                    font.family: Theme.font
                    font.pixelSize: Theme.fontSize
                    echoMode: TextInput.Password
                    renderType: Text.CurveRendering
                    focus: entry.asking
                    onAccepted: {
                        entry.modelData.connectWithPsk(text);
                        text = "";
                        entry.asking = false;
                    }

                    Rectangle {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.bottom
                        height: 1
                        color: Theme.border
                    }
                }
            }
        }
    }

    footer: [
        Item {
            Layout.fillWidth: true
        },
        Action {
            text: "settings →"
            onClicked: {
                Quickshell.execDetached(["nm-connection-editor"]);
                root.close();
            }
        }
    ]
}
