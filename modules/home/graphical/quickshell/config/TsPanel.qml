pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell

Panel {
    id: root

    property bool adding: false
    // The only ports funnel allows.
    readonly property var ports: ["443", "8443", "10000"]
    property string newPort: "443"
    // Public is always an explicit choice.
    property bool newPublic: false

    onAddingChanged: {
        if (!adding)
            return;
        newPort = "443";
        newPublic = false;
        targetInput.text = "";
        pathInput.text = "";
        targetInput.input.forceActiveFocus();
    }

    // Value most recently copied; its row shows "Copied" for a moment.
    property string copied: ""

    function copy(text) {
        Quickshell.clipboardText = text;
        copied = text;
        copiedReset.restart();
    }

    Timer {
        id: copiedReset
        interval: 1500
        onTriggered: root.copied = ""
    }

    // "http://127.0.0.1:3000" -> ":3000"
    function shortTarget(t) {
        return t.replace(/^https?:\/\/(127\.0\.0\.1|localhost)(?=:)/, "");
    }

    function submit() {
        const target = targetInput.text.trim();
        if (!target)
            return;
        Ts.addServe(newPort, target, newPublic, pathInput.text.trim() || "/");
        targetInput.text = "";
        pathInput.text = "";
        adding = false;
    }

    readonly property color faint: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)

    name: "ts"
    title: "Tailscale"
    backTo: "net"

    onVisibleChanged: {
        Ts.watching = visible;
        if (visible) {
            Ts.error = "";
            Ts.refresh();
        } else {
            adding = false;
            exit.open = false;
        }
    }

    actions: Switch {
        visible: Ts.available
        checked: Ts.up
        onToggled: Ts.setUp(!Ts.up)
    }

    UiText {
        visible: Ts.error !== ""
        Layout.fillWidth: true
        Layout.leftMargin: 6
        Layout.rightMargin: 6
        text: Ts.error
        color: Theme.warnText
        font.pixelSize: Theme.small
        wrapMode: Text.Wrap
    }

    // This device.
    Tile {
        visible: Ts.up && Ts.self !== null

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
                    name: "tailscale"
                    size: 17
                    color: Theme.accentInk
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    UiText {
                        Layout.fillWidth: true
                        Layout.maximumWidth: implicitWidth
                        text: Ts.self?.name ?? ""
                        font.weight: Font.DemiBold
                    }
                    UiText {
                        Layout.fillWidth: true
                        text: "· this device"
                        color: root.faint
                        font.pixelSize: Theme.small
                    }
                }
                UiText {
                    Layout.fillWidth: true
                    text: root.copied !== "" && root.copied === Ts.self?.ip ? "Copied" : Ts.self?.ip ?? ""
                    mono: true
                    color: text === "Copied" ? Theme.accent : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.75)
                    font.pixelSize: Theme.small
                }
            }
            CircleButton {
                icon: root.copied !== "" && root.copied === Ts.self?.ip ? "check" : "copy"
                onClicked: root.copy(Ts.self?.ip ?? "")
            }
        }
    }

    SectionLabel {
        visible: Ts.up && Ts.exitOptions.length > 0
        text: "Exit node"
    }

    Dropdown {
        id: exit
        visible: Ts.up && Ts.exitOptions.length > 0
        Layout.topMargin: -4
        model: [
            {
                text: "None",
                note: "Direct"
            }
        ].concat(Ts.exitOptions.map(p => ({
                    text: p.name,
                    note: p.online ? "Online" : "Offline",
                    disabled: !p.online
                })))
        currentIndex: Ts.exitOptions.findIndex(p => p.exitActive) + 1
        sublabel: Ts.exitNode ? "Routing all traffic" : ""
        onActivated: i => Ts.setExitNode(i === 0 ? null : Ts.exitOptions[i - 1])
    }

    SectionLabel {
        visible: Ts.up
        text: "Serving"

        PillButton {
            pillHeight: 24
            text: root.adding ? "Cancel" : "+ Add"
            checked: root.adding
            onClicked: root.adding = !root.adding
        }
    }

    ColumnLayout {
        visible: Ts.up && Ts.serves.length > 0
        Layout.fillWidth: true
        Layout.topMargin: -4
        spacing: 2

        Repeater {
            model: Ts.up ? Ts.serves : []

            // Click copies the public/tailnet URL.
            MouseArea {
                id: entry

                required property var modelData

                Layout.fillWidth: true
                implicitHeight: 40
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.copy(modelData.url)

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.rowRadius
                    color: entry.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 8
                    spacing: 8

                    UiText {
                        Layout.fillWidth: true
                        textFormat: Text.StyledText
                        text: root.copied === entry.modelData.url ? `<font color="${Theme.accent}">Copied</font>` : ":" + entry.modelData.port + (entry.modelData.path === "/" ? "" : entry.modelData.path) + ` <font color="${Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.5)}">→</font> ` + root.shortTarget(entry.modelData.target)
                        mono: true
                        font.pixelSize: Theme.small
                    }

                    // Who can reach it; click flips between tailnet and public (funnel).
                    MouseArea {
                        id: who
                        implicitWidth: whoText.implicitWidth + 18
                        implicitHeight: 22
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Ts.setPublic(entry.modelData, !entry.modelData.funnel)

                        Rectangle {
                            anchors.fill: parent
                            radius: height / 2
                            color: entry.modelData.funnel ? Qt.alpha(Theme.mic, who.containsMouse ? 0.38 : 0.28) : who.containsMouse ? Theme.fillActive : Theme.fillHover
                        }
                        UiText {
                            id: whoText
                            anchors.centerIn: parent
                            text: entry.modelData.funnel ? "Public" : "Tailnet"
                            color: entry.modelData.funnel ? "#ffc58a" : Theme.text
                            font.pixelSize: Theme.tiny
                            font.weight: Font.Medium
                        }
                    }

                    CircleButton {
                        size: 24
                        iconSize: 12
                        icon: "close"
                        onClicked: Ts.removeServe(entry.modelData)
                    }
                }
            }
        }
    }

    UiText {
        visible: Ts.up && Ts.serves.length === 0 && !root.adding
        Layout.leftMargin: 10
        Layout.topMargin: -4
        text: "Nothing served"
        faint: true
        font.pixelSize: Theme.small
    }

    // New serve/funnel entry.
    Tile {
        visible: Ts.up && root.adding

        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 12
            rowSpacing: 8

            UiText {
                text: "Port"
                dim: true
                font.pixelSize: Theme.small
            }
            Segmented {
                implicitHeight: 32
                model: root.ports
                currentIndex: root.ports.indexOf(root.newPort)
                onActivated: i => root.newPort = root.ports[i]
            }

            UiText {
                text: "To"
                dim: true
                font.pixelSize: Theme.small
            }
            GlassField {
                id: targetInput
                placeholder: "3000, localhost:8080, /some/dir"
                onAccepted: root.submit()
            }

            UiText {
                text: "Path"
                dim: true
                font.pixelSize: Theme.small
            }
            GlassField {
                id: pathInput
                placeholder: "/"
                onAccepted: root.submit()
            }

            UiText {
                text: "Who"
                dim: true
                font.pixelSize: Theme.small
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Segmented {
                    implicitHeight: 32
                    model: ["Tailnet", "Public"]
                    currentIndex: root.newPublic ? 1 : 0
                    onActivated: i => root.newPublic = i === 1
                }
                PillButton {
                    pillHeight: 32
                    accent: true
                    icon: "check"
                    text: "Add"
                    onClicked: root.submit()
                }
            }
        }
    }

    SectionLabel {
        visible: Ts.up && Ts.peers.length > 0
        text: "Devices"
    }

    ColumnLayout {
        visible: Ts.up && Ts.peers.length > 0
        Layout.fillWidth: true
        Layout.topMargin: -4
        spacing: 2

        Repeater {
            model: Ts.up ? Ts.peers.slice(0, 8) : []

            // Click copies the MagicDNS name.
            MouseArea {
                id: peer

                required property var modelData

                Layout.fillWidth: true
                implicitHeight: 40
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.copy(modelData.dns)

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.rowRadius
                    color: peer.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    spacing: 12

                    Rectangle {
                        implicitWidth: 8
                        implicitHeight: 8
                        radius: 4
                        color: peer.modelData.online ? Theme.online : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.3)
                    }
                    UiText {
                        Layout.fillWidth: true
                        text: peer.modelData.name
                        color: peer.modelData.online ? Theme.text : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
                    }
                    UiText {
                        readonly property bool done: root.copied === peer.modelData.dns
                        text: done ? "Copied" : peer.modelData.online ? peer.modelData.ip : "offline"
                        mono: !done
                        color: done ? Theme.accent : root.faint
                        font.pixelSize: Theme.small
                    }
                }
            }
        }
    }

    UiText {
        visible: !Ts.up
        Layout.fillWidth: true
        Layout.topMargin: 18
        Layout.bottomMargin: 18
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: !Ts.available ? "Tailscale isn't running" : Ts.backend === "NeedsLogin" ? "Needs login: run `tailscale up`" : Ts.backend === "Stopped" || Ts.backend === "" ? "Tailscale is stopped" : "Tailscale is " + Ts.backend.toLowerCase()
        dim: true
    }

    footer: ListRow {
        implicitHeight: 34
        text: "Admin console…"
        onClicked: {
            Quickshell.execDetached(["xdg-open", "https://login.tailscale.com/admin/machines"]);
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
