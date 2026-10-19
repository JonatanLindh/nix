pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

// Session menu on super+m (replaces wleave): the screen dims and a glass card shows two rows
// of three tiles, each with its wleave key. Press the key, or move with the arrows (or the
// mouse) and press enter. No confirmation, by request.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: bar.panel === "session"
    readonly property var actions: [
        {
            name: "lock",
            label: "Lock",
            icon: "lock",
            key: "l",
            command: ["loginctl", "lock-session"]
        },
        {
            name: "hibernate",
            label: "Hibernate",
            icon: "hibernate",
            key: "h",
            command: ["systemctl", "hibernate"],
            enabled: Power.canHibernate
        },
        {
            name: "logout",
            label: "Log out",
            icon: "logout",
            key: "e",
            command: ["sh", "-c", 'loginctl terminate-user "$USER"']
        },
        {
            name: "shutdown",
            label: "Shut down",
            icon: "power",
            key: "s",
            command: ["systemctl", "poweroff"]
        },
        {
            name: "suspend",
            label: "Suspend",
            icon: "moon-outline",
            key: "u",
            command: ["systemctl", "suspend"]
        },
        {
            name: "reboot",
            label: "Reboot",
            icon: "reboot",
            key: "r",
            command: ["systemctl", "reboot"]
        }
    ]
    property int selected: 0

    function close() {
        bar.panel = "";
    }

    function run(action) {
        if (!action || action.enabled === false)
            return;
        close();
        Quickshell.execDetached(action.command);
    }

    screen: bar.screen
    // Created once and kept: opening just flips state, with no new surface and no layer
    // open animation. While closed it draws nothing, takes no clicks and no keyboard.
    visible: true
    mask: open ? contentMask : noInput
    // Covers the whole screen (the dimmed backdrop); the card sits in the middle.
    anchors.top: true
    anchors.bottom: true
    anchors.left: true
    anchors.right: true
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:session"
    WlrLayershell.keyboardFocus: open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    onOpenChanged: {
        if (!open)
            return;
        selected = 0;
        Qt.callLater(() => keys.forceActiveFocus());
    }

    Region {
        id: contentMask
        item: keys
    }

    Region {
        id: noInput
    }

    // Activating the grab in the same tick the window opens gets it cleared right away (which
    // closes the window); give it a moment, like Panel.qml does.
    property bool grabReady: false

    Timer {
        id: grabDelay
        interval: 50
        running: root.open
        onRunningChanged: if (running)
            root.grabReady = false
        onTriggered: root.grabReady = true
    }

    // Clicking another monitor closes it.
    HyprlandFocusGrab {
        windows: [root]
        active: root.grabReady && root.open
        onCleared: root.close()
    }

    // The backdrop: dims the screen, and a click on it (outside the card) closes.
    MouseArea {
        id: keys

        visible: root.open

        anchors.fill: parent
        focus: true
        onClicked: root.close()
        Keys.onPressed: event => {
            const cols = 3;
            const n = root.actions.length;
            if (event.key === Qt.Key_Escape) {
                root.close();
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.run(root.actions[root.selected]);
            } else if (event.key === Qt.Key_Left) {
                root.selected = (root.selected + n - 1) % n;
            } else if (event.key === Qt.Key_Right) {
                root.selected = (root.selected + 1) % n;
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                root.selected = (root.selected + cols) % n;
            } else {
                const action = root.actions.find(a => a.key === event.text.toLowerCase());
                if (!action)
                    return;
                root.run(action);
            }
            event.accepted = true;
        }

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(4 / 255, 10 / 255, 12 / 255, 0.42)
        }

        Glass {
            anchors.centerIn: parent
            width: 560
            height: card.implicitHeight + 36
            radius: 28

            // Clicks between the tiles stay on the card instead of closing.
            MouseArea {
                anchors.fill: parent
            }

            ColumnLayout {
                id: card

                anchors.fill: parent
                anchors.margins: 18
                spacing: 14

                GridLayout {
                    Layout.fillWidth: true
                    columns: 3
                    rowSpacing: 10
                    columnSpacing: 10

                    Repeater {
                        model: root.actions

                        MouseArea {
                            id: box

                            required property var modelData
                            required property int index
                            readonly property bool usable: modelData.enabled !== false
                            readonly property bool current: root.selected === index && usable
                            readonly property color ink: current ? Theme.ink : usable ? Theme.text : Theme.textFaint

                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            implicitHeight: 128
                            hoverEnabled: true
                            cursorShape: usable ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onEntered: root.selected = index
                            onClicked: root.run(modelData)

                            Rectangle {
                                anchors.fill: parent
                                radius: 18
                                color: box.current ? Theme.white : Theme.fill
                                Behavior on color {
                                    ColorAnimation {
                                        duration: Theme.fast
                                    }
                                }
                            }

                            Column {
                                anchors.centerIn: parent
                                spacing: 12

                                Icon {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    name: box.modelData.icon
                                    size: 26
                                    strokeWidth: 1.8
                                    color: box.ink
                                }
                                UiText {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: box.modelData.label
                                    color: box.ink
                                    font.pixelSize: 14
                                    font.weight: Font.Medium
                                }
                            }

                            // Key cap, tinted to sit on either fill.
                            Rectangle {
                                anchors.top: parent.top
                                anchors.right: parent.right
                                anchors.margins: 10
                                implicitWidth: Math.max(18, keyText.implicitWidth + 10)
                                implicitHeight: 20
                                radius: 5
                                color: box.current ? Qt.alpha(Theme.ink, 0.1) : Qt.rgba(1, 1, 1, 0.1)

                                UiText {
                                    id: keyText
                                    anchors.centerIn: parent
                                    text: box.modelData.key
                                    mono: true
                                    color: box.ink
                                    font.pixelSize: Theme.tiny
                                }
                            }
                        }
                    }
                }

                Row {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 16

                    UiText {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Press a letter"
                        dim: true
                        font.pixelSize: Theme.small
                    }
                    KeyHint {
                        keys: ["←", "→", "↵"]
                        text: "Pick"
                    }
                    KeyHint {
                        keys: ["esc"]
                        text: "Close"
                    }
                }
            }
        }
    }
}
