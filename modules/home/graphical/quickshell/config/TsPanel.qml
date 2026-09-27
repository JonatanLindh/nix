pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell

Panel {
    id: root

    property bool adding: false
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
        targetInput.forceActiveFocus();
    }

    // Value most recently copied; its row shows "copied" for a moment.
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

    name: "ts"
    title: "ts"

    onVisibleChanged: {
        Ts.watching = visible;
        if (visible) {
            Ts.error = "";
            Ts.refresh();
        } else {
            adding = false;
        }
    }

    actions: Toggle {
        checked: Ts.up
        onToggled: Ts.setUp(!Ts.up)
    }

    Note {
        visible: Ts.error !== ""
        Layout.topMargin: 12
        text: Ts.error
        color: Theme.warn
    }

    Note {
        visible: Ts.backend !== "" && !Ts.up
        Layout.topMargin: 12
        text: Ts.backend === "NeedsLogin" ? "needs login: run `tailscale up`" : "tailscale is " + Ts.backend.toLowerCase()
    }

    Section {
        text: "this device"
    }

    PanelRow {
        text: Ts.self?.name ?? ""
        detail: root.copied !== "" && root.copied === Ts.self?.ip ? "copied" : Ts.self?.ip ?? ""
        detailColor: detail === "copied" ? Theme.accent : Theme.label
        selected: Ts.up
        onClicked: root.copy(Ts.self?.ip ?? "")
    }

    Section {
        visible: Ts.up && Ts.exitOptions.length > 0
        text: "exit node"
    }

    PanelRow {
        visible: Ts.up && Ts.exitOptions.length > 0
        text: "none"
        selected: Ts.exitNode === null
        onClicked: Ts.setExitNode(null)
    }

    Repeater {
        model: Ts.up ? Ts.exitOptions : []

        PanelRow {
            required property var modelData
            text: modelData.name
            detail: modelData.online ? "" : "offline"
            selected: modelData.exitActive
            dimmed: !modelData.online
            onClicked: Ts.setExitNode(modelData)
        }
    }

    Section {
        visible: Ts.up
        text: "serve"

        Action {
            text: root.adding ? "cancel" : "+ add"
            onClicked: root.adding = !root.adding
        }
    }

    Repeater {
        model: Ts.up ? Ts.serves : []

        PanelRow {
            id: entry

            required property var modelData
            text: ":" + modelData.port + modelData.path + " → " + root.shortTarget(modelData.target)
            detail: root.copied === modelData.url ? "copied" : ""
            detailColor: Theme.accent
            selected: true
            // Click copies the public/tailnet URL.
            onClicked: root.copy(modelData.url)

            Action {
                text: entry.modelData.funnel ? "public" : "tailnet"
                onClicked: Ts.setPublic(entry.modelData, !entry.modelData.funnel)

                BarText {
                    // Accent the word when the entry is on the public internet.
                    anchors.fill: parent
                    visible: entry.modelData.funnel
                    text: parent.text
                    color: Theme.accent
                }
            }

            Action {
                text: "remove"
                onClicked: Ts.removeServe(entry.modelData)
            }
        }
    }

    Note {
        visible: Ts.up && Ts.serves.length === 0 && !root.adding
        text: "nothing served"
    }

    // New serve/funnel entry.
    ColumnLayout {
        visible: Ts.up && root.adding
        Layout.fillWidth: true
        Layout.leftMargin: Theme.panelPadding
        Layout.rightMargin: Theme.panelPadding
        Layout.topMargin: 6
        spacing: 10

        RowLayout {
            spacing: 12

            BarText {
                Layout.preferredWidth: 40
                text: "port"
                color: Theme.label
            }

            Repeater {
                // The only ports funnel allows.
                model: ["443", "8443", "10000"]

                MouseArea {
                    required property string modelData
                    implicitWidth: portLabel.implicitWidth
                    implicitHeight: portLabel.implicitHeight
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.newPort = modelData

                    BarText {
                        id: portLabel
                        text: parent.modelData
                        color: root.newPort === parent.modelData ? Theme.accent : Theme.text
                        font.underline: root.newPort === parent.modelData
                    }
                }
            }
        }

        RowLayout {
            spacing: 12

            BarText {
                Layout.preferredWidth: 40
                text: "to"
                color: Theme.label
            }

            Input {
                id: targetInput
                Layout.fillWidth: true
                placeholder: "3000, localhost:8080, /some/dir"
                onAccepted: root.submit()
            }
        }

        RowLayout {
            spacing: 12

            BarText {
                Layout.preferredWidth: 40
                text: "path"
                color: Theme.label
            }

            Input {
                id: pathInput
                Layout.fillWidth: true
                placeholder: "/"
                onAccepted: root.submit()
            }
        }

        RowLayout {
            spacing: 12

            BarText {
                Layout.preferredWidth: 40
                text: "who"
                color: Theme.label
            }

            Toggle {
                checked: root.newPublic
                onText: "public"
                offText: "tailnet"
                onToggled: root.newPublic = !root.newPublic
            }

            Item {
                Layout.fillWidth: true
            }

            Action {
                text: "add →"
                onClicked: root.submit()
            }
        }
    }

    Section {
        visible: Ts.up && Ts.peers.length > 0
        text: "peers"
    }

    Repeater {
        model: Ts.up ? Ts.peers.slice(0, 8) : []

        PanelRow {
            required property var modelData
            text: modelData.name
            detail: root.copied === modelData.dns ? "copied" : modelData.online ? modelData.ip : modelData.os
            detailColor: root.copied === modelData.dns ? Theme.accent : Theme.label
            dimmed: !modelData.online
            // Click copies the MagicDNS name.
            onClicked: root.copy(modelData.dns)
        }
    }

    footer: [
        Item {
            Layout.fillWidth: true
        },
        Action {
            text: "admin →"
            onClicked: {
                Quickshell.execDetached(["xdg-open", "https://login.tailscale.com/admin/machines"]);
                root.close();
            }
        }
    ]
}
