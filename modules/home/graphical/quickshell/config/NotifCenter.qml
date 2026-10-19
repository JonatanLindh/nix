pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

// Notification history, opened with super+n. Its own layer window (like Clip) so it
// takes the keyboard without a click. Sits where the popups do, which hide meanwhile.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: bar.panel === "notif"
    readonly property var current: Notifs.history[list.currentIndex] ?? null

    function close() {
        bar.panel = "";
    }

    // Enter / click: the default action, which also takes it out of the history.
    function activate(entry) {
        if (!entry?.hasDefault || !Notifs.isLive(entry.id))
            return;
        // Close first so focus is back on the desktop when the app asks for it.
        close();
        Notifs.invoke(entry.id);
    }

    screen: bar.screen
    // Created once and kept: opening just flips state, with no new surface and no layer
    // open animation. While closed it draws nothing, takes no clicks and no keyboard.
    visible: true
    anchors.top: true
    anchors.right: true
    // qmllint disable unqualified unresolved-type
    margins.top: Theme.barHeight + 4
    margins.right: 8
    // qmllint enable unqualified unresolved-type
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: 380
    // Fixed height: resizing the layer surface on every keystroke makes it jitter (Hyprland
    // animates layer geometry). The frame sizes itself inside; input only reaches the frame.
    implicitHeight: 680
    mask: open ? contentMask : noInput
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:notifs"
    WlrLayershell.keyboardFocus: open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    onOpenChanged: {
        if (!open)
            return;
        list.currentIndex = 0;
        Notifs.markSeen();
        frame.forceActiveFocus();
    }

    Region {
        id: contentMask
        item: frame
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

    // Clicking anywhere else closes it.
    HyprlandFocusGrab {
        windows: [root]
        active: root.grabReady && root.open
        onCleared: root.close()
    }

    PanelFrame {
        id: frame

        visible: root.open

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: implicitHeight
        title: "Notifications"
        onDismissed: root.close()

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Up) {
                list.decrementCurrentIndex();
            } else if (event.key === Qt.Key_Down) {
                list.incrementCurrentIndex();
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.activate(root.current);
            } else if (event.key === Qt.Key_Delete) {
                if (root.current)
                    Notifs.remove(root.current.id);
            } else {
                return;
            }
            event.accepted = true;
        }

        actions: [
            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "moon"
                    size: 14
                    color: Theme.textDim
                }
                UiText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "DND"
                    dim: true
                    font.pixelSize: Theme.small
                }
            },
            Switch {
                anchors.verticalCenter: parent.verticalCenter
                checked: Notifs.dnd
                checkedColor: Theme.screen
                onToggled: Notifs.dnd = !Notifs.dnd
            },
            PillButton {
                anchors.verticalCenter: parent.verticalCenter
                visible: Notifs.history.length > 0
                text: "Clear"
                onClicked: Notifs.clear()
            }
        ]

        ListView {
            id: list

            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 540)
            visible: count > 0
            clip: true
            spacing: 8
            model: Notifs.history
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0

            delegate: MouseArea {
                id: row

                required property var modelData
                required property int index
                readonly property bool current: ListView.isCurrentItem
                readonly property bool live: Notifs.isLive(modelData.id)
                readonly property var when: new Date(modelData.time)

                width: list.width
                height: card.implicitHeight + 24
                hoverEnabled: true
                cursorShape: live && modelData.hasDefault ? Qt.PointingHandCursor : Qt.ArrowCursor
                onEntered: list.currentIndex = index
                onClicked: root.activate(modelData)

                // The selected card (hovered or picked with the arrows) is a step lighter.
                Rectangle {
                    anchors.fill: parent
                    radius: Theme.tileRadius
                    color: row.current ? Theme.fillHover : Theme.fill
                    border.width: row.modelData.critical ? 1 : 0
                    border.color: Qt.alpha(Theme.warn, 0.5)
                }

                NotifCard {
                    id: card

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 12
                    entry: row.modelData
                    // Older than today: the day too.
                    time: new Date().toDateString() === row.when.toDateString() ? Qt.formatTime(row.when, "HH:mm") : Qt.formatDateTime(row.when, "ddd HH:mm")
                    headerInset: row.current ? 22 : 0
                }

                CircleButton {
                    visible: row.current
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.margins: 8
                    size: 22
                    icon: "close"
                    iconSize: 12
                    restColor: Theme.fillHover
                    onClicked: Notifs.remove(row.modelData.id)
                }
            }
        }

        // Centred with anchors: inside the frame's layout a nested ColumnLayout kept its
        // implicit width and sat off to the left.
        Item {
            visible: Notifs.history.length === 0
            Layout.fillWidth: true
            implicitHeight: empty.implicitHeight + 60

            Column {
                id: empty
                anchors.centerIn: parent
                spacing: 10

                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: "bell"
                    size: 28
                    strokeWidth: 1.6
                    color: Theme.textDim
                }
                UiText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: Notifs.dnd ? "No notifications · Do not disturb is on" : "No notifications"
                    dim: true
                }
            }
        }

        footer: [
            Item {
                Layout.fillWidth: true
            },
            Row {
                spacing: 16

                KeyHint {
                    keys: ["↵"]
                    text: "Open"
                }
                KeyHint {
                    keys: ["del"]
                    text: "Remove"
                }
                KeyHint {
                    keys: ["esc"]
                    text: "Close"
                }
            },
            Item {
                Layout.fillWidth: true
            }
        ]
    }
}
