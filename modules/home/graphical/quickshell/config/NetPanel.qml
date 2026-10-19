pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Networking

Panel {
    id: root

    // Interface -> IPv4 address, and the connected network's band; Quickshell has neither,
    // so nmcli fills them in while the panel is open.
    property var addresses: ({})
    property string band: ""

    readonly property bool wifiOn: Net.wifi !== null && Networking.wifiEnabled
    // Wired links are shown when up, plugged in, or when there's no Wi-Fi to show instead.
    readonly property var wired: Net.wired.filter(d => d.connected || d.hasLink || Net.wifi === null)
    readonly property var others: wifiOn ? Net.wifi.networks.values.filter(n => !n.connected).sort((a, b) => (b.known - a.known) || (b.signalStrength - a.signalStrength)).slice(0, 8) : []

    function securityLabel(s) {
        switch (s) {
        case WifiSecurityType.Open:
            return "Open";
        case WifiSecurityType.Owe:
            return "OWE";
        case WifiSecurityType.StaticWep:
        case WifiSecurityType.DynamicWep:
            return "WEP";
        case WifiSecurityType.WpaPsk:
            return "WPA";
        case WifiSecurityType.Wpa2Psk:
            return "WPA2";
        case WifiSecurityType.Sae:
            return "WPA3";
        case WifiSecurityType.WpaEap:
        case WifiSecurityType.Wpa2Eap:
        case WifiSecurityType.Wpa3SuiteB192:
        case WifiSecurityType.Leap:
            return "802.1X";
        default:
            return "";
        }
    }

    function signalIcon(strength) {
        return strength >= 0.6 ? "wifi" : strength >= 0.35 ? "wifi-2" : "wifi-1";
    }

    function ip(device) {
        return (root.addresses[device?.name ?? ""] ?? "").replace(/\/\d+$/, "");
    }

    function speed(mbps) {
        return mbps >= 1000 ? (mbps / 1000) + " Gb/s" : mbps + " Mb/s";
    }

    name: "net"
    title: Net.wifi !== null ? "Wi-Fi" : "Network"

    // Scan only while the panel is open.
    onVisibleChanged: {
        if (Net.wifi)
            Net.wifi.scannerEnabled = visible;
        if (visible)
            details.running = true;
    }

    Process {
        id: details
        command: ["sh", "-c", "nmcli -t -f GENERAL.DEVICE,IP4.ADDRESS device show; echo; nmcli -t -f IN-USE,FREQ device wifi list --rescan no 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: {
                const map = {};
                let dev = "", freq = 0;
                for (const line of text.split("\n")) {
                    if (line.startsWith("GENERAL.DEVICE:"))
                        dev = line.slice(15);
                    else if (line.startsWith("IP4.ADDRESS[1]:") && dev)
                        map[dev] = line.slice(15);
                    else if (line.startsWith("*:"))
                        freq = parseInt(line.slice(2)) || 0;
                }
                root.addresses = map;
                root.band = freq >= 5925 ? "6 GHz" : freq >= 4900 ? "5 GHz" : freq > 0 ? "2.4 GHz" : "";
            }
        }
    }

    // Re-read when the connection changes while open.
    Connections {
        target: Net
        function onWifiNetworkChanged() {
            if (root.visible)
                details.running = true;
        }
        function onWiredUpChanged() {
            if (root.visible)
                details.running = true;
        }
    }

    actions: Switch {
        visible: Net.wifi !== null
        checked: Networking.wifiEnabled
        onToggled: Networking.wifiEnabled = !Networking.wifiEnabled
    }

    Repeater {
        model: root.wired

        Tile {
            id: link

            required property var modelData

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                Rectangle {
                    implicitWidth: 34
                    implicitHeight: 34
                    radius: 17
                    color: link.modelData.connected ? Theme.accent : Theme.fillHover

                    Icon {
                        anchors.centerIn: parent
                        name: "ethernet"
                        size: 17
                        strokeWidth: 2.2
                        color: link.modelData.connected ? Theme.accentInk : Theme.text
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    UiText {
                        Layout.fillWidth: true
                        text: "Wired"
                        font.weight: Font.DemiBold
                    }
                    UiText {
                        Layout.fillWidth: true
                        text: link.modelData.connected ? ["Connected"].concat(link.modelData.linkSpeed > 0 ? [root.speed(link.modelData.linkSpeed)] : []).join(" · ") : link.modelData.hasLink ? "Not connected" : "Cable unplugged"
                        dim: true
                        font.pixelSize: Theme.small
                    }
                }
            }
            Row {
                visible: link.modelData.connected
                Layout.leftMargin: 46
                spacing: 16

                UiText {
                    visible: text !== ""
                    text: root.ip(link.modelData)
                    mono: true
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                    font.pixelSize: Theme.tiny
                }
                UiText {
                    text: link.modelData.name
                    mono: true
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                    font.pixelSize: Theme.tiny
                }
            }
        }
    }

    // The connected network.
    Tile {
        id: current

        readonly property var network: root.wifiOn ? Net.wifiNetwork : null

        visible: network !== null

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Rectangle {
                implicitWidth: 34
                implicitHeight: 34
                radius: 17
                color: Theme.accent

                Icon {
                    anchors.centerIn: parent
                    name: root.signalIcon(current.network?.signalStrength ?? 1)
                    size: 17
                    strokeWidth: 2.3
                    color: Theme.accentInk
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                UiText {
                    Layout.fillWidth: true
                    text: current.network?.name ?? ""
                    font.weight: Font.DemiBold
                }
                UiText {
                    Layout.fillWidth: true
                    // qmllint disable unresolved-type
                    text: ["Connected", root.band, root.securityLabel(current.network?.security)].filter(s => s).join(" · ")
                    // qmllint enable unresolved-type
                    dim: true
                    font.pixelSize: Theme.small
                }
            }
            PillButton {
                text: "Disconnect"
                onClicked: current.network?.disconnect()
            }
        }
        Row {
            Layout.leftMargin: 46
            spacing: 16

            UiText {
                visible: text !== ""
                text: root.ip(Net.wifi)
                mono: true
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                font.pixelSize: Theme.tiny
            }
            UiText {
                text: "Signal " + Math.round((current.network?.signalStrength ?? 0) * 100) + "%"
                mono: true
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                font.pixelSize: Theme.tiny
            }
        }
    }

    SectionLabel {
        visible: root.wifiOn
        text: Net.wifiNetwork ? "Other networks" : "Networks"

        CircleButton {
            icon: "refresh"
            onClicked: Quickshell.execDetached(["nmcli", "device", "wifi", "rescan"])
        }
    }

    ColumnLayout {
        visible: root.wifiOn
        Layout.fillWidth: true
        Layout.topMargin: -4
        spacing: 2

        Repeater {
            model: root.others

            ColumnLayout {
                id: entry

                required property WifiNetwork modelData
                // qmllint disable unresolved-type
                readonly property bool secured: modelData.security !== WifiSecurityType.Open && modelData.security !== WifiSecurityType.Owe
                // qmllint enable unresolved-type
                property bool asking: false
                property bool failed: false

                function join() {
                    if (psk.text === "")
                        return;
                    modelData.connectWithPsk(psk.text);
                    psk.text = "";
                    asking = false;
                }

                Layout.fillWidth: true
                spacing: 0

                Connections {
                    target: entry.modelData
                    function onConnectionFailed() {
                        entry.failed = true;
                    }
                }

                ListRow {
                    icon: root.signalIcon(entry.modelData.signalStrength)
                    text: entry.modelData.name
                    onClicked: {
                        entry.failed = false;
                        if (entry.modelData.known || !entry.secured)
                            entry.modelData.connect();
                        else
                            entry.asking = !entry.asking;
                    }

                    UiText {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: text !== ""
                        text: entry.modelData.stateChanging ? "Connecting…" : entry.failed ? "Failed" : entry.modelData.known ? "Saved" : ""
                        color: entry.failed ? Theme.warnText : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
                        font.pixelSize: Theme.tiny
                    }
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: entry.secured
                        name: "lock"
                        size: 14
                        color: Theme.textDim
                    }
                }

                // Password for a secured network that isn't saved yet.
                RowLayout {
                    visible: entry.asking
                    Layout.fillWidth: true
                    Layout.leftMargin: 40
                    Layout.rightMargin: 10
                    Layout.topMargin: 4
                    Layout.bottomMargin: 8
                    spacing: 8

                    GlassField {
                        id: psk
                        password: true
                        placeholder: "Password"
                        onAccepted: entry.join()
                    }

                    MouseArea {
                        implicitWidth: 32
                        implicitHeight: 32
                        cursorShape: Qt.PointingHandCursor
                        onClicked: entry.join()

                        Rectangle {
                            anchors.fill: parent
                            radius: 9
                            color: Theme.accent
                        }
                        Icon {
                            anchors.centerIn: parent
                            name: "check"
                            size: 16
                            strokeWidth: 2.6
                            color: Theme.accentInk
                        }
                    }

                    onVisibleChanged: if (visible)
                        psk.input.forceActiveFocus()
                }
            }
        }
    }

    UiText {
        visible: Net.wifi !== null && !Networking.wifiEnabled
        Layout.fillWidth: true
        Layout.topMargin: 18
        Layout.bottomMargin: 18
        horizontalAlignment: Text.AlignHCenter
        text: "Wi-Fi is off"
        dim: true
    }

    // Tailscale lives here now: the switch, its state, and a way into its own panel.
    Tile {
        visible: Ts.available
        padding: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Rectangle {
                implicitWidth: 34
                implicitHeight: 34
                radius: 17
                color: Ts.up ? Theme.accent : Theme.fillHover

                Icon {
                    anchors.centerIn: parent
                    name: "tailscale"
                    size: 17
                    color: Ts.up ? Theme.accentInk : Theme.text
                }
            }

            MouseArea {
                id: tsOpen
                Layout.fillWidth: true
                implicitHeight: tsText.implicitHeight
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.bar.open("ts", "net")

                ColumnLayout {
                    id: tsText
                    anchors.left: parent.left
                    anchors.right: parent.right
                    spacing: 1

                    RowLayout {
                        spacing: 2
                        UiText {
                            text: "Tailscale"
                            font.weight: Font.DemiBold
                        }
                        Icon {
                            name: "chevron-right"
                            size: 14
                            strokeWidth: 2
                            color: tsOpen.containsMouse ? Theme.text : Theme.textDim
                        }
                    }
                    UiText {
                        Layout.fillWidth: true
                        text: Ts.backend === "NeedsLogin" ? "Needs login" : !Ts.up ? "Off" : Ts.exitNode ? `Exit node · ${Ts.exitNode.name}` : Ts.funnelActive ? "On · funnel public" : "On"
                        color: Ts.backend === "NeedsLogin" ? Theme.warnText : Theme.textDim
                        font.pixelSize: Theme.small
                    }
                }
            }

            Switch {
                checked: Ts.up
                onToggled: Ts.setUp(!Ts.up)
            }
        }
    }

    footer: ListRow {
        implicitHeight: 34
        text: "Network settings…"
        onClicked: {
            Quickshell.execDetached(["nm-connection-editor"]);
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
