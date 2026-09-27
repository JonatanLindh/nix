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
    implicitWidth: 460
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
        title: "notifications"
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
            BarText {
                text: "dnd"
                color: Theme.label
            },
            Toggle {
                checked: Notifs.dnd
                onToggled: Notifs.dnd = !Notifs.dnd
            },
            Action {
                visible: Notifs.history.length > 0
                text: "clear"
                onClicked: Notifs.clear()
            }
        ]

        ListView {
            id: list

            Layout.fillWidth: true
            Layout.topMargin: 8
            Layout.preferredHeight: Math.min(contentHeight, 480)
            clip: true
            model: Notifs.history
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0

            delegate: MouseArea {
                id: row

                required property var modelData
                required property int index
                readonly property bool current: ListView.isCurrentItem
                readonly property bool live: Notifs.isLive(modelData.id)

                width: list.width
                height: 50
                hoverEnabled: true
                cursorShape: live && modelData.hasDefault ? Qt.PointingHandCursor : Qt.ArrowCursor
                onEntered: list.currentIndex = index
                onClicked: root.activate(modelData)

                Rectangle {
                    anchors.fill: parent
                    color: row.current ? Theme.hover : "transparent"
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.panelPadding
                    anchors.rightMargin: Theme.panelPadding
                    spacing: 8

                    BarText {
                        Layout.preferredWidth: 8
                        Layout.alignment: Qt.AlignTop
                        Layout.topMargin: 8
                        text: row.current ? ">" : ""
                        color: Theme.accent
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            BarText {
                                text: Qt.formatTime(new Date(row.modelData.time), "hh:mm")
                                color: Theme.off
                            }

                            BarText {
                                Layout.fillWidth: true
                                text: row.modelData.app
                                elide: Text.ElideRight
                                color: row.modelData.critical ? Theme.warn : Theme.label
                            }

                            Action {
                                visible: row.containsMouse
                                text: "×"
                                onClicked: Notifs.remove(row.modelData.id)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            BarText {
                                Layout.maximumWidth: list.width * 0.6
                                visible: text !== ""
                                text: row.modelData.summary.replace(/\s+/g, " ")
                                elide: Text.ElideRight
                                color: row.current ? Theme.bright : Theme.text
                            }

                            BarText {
                                Layout.fillWidth: true
                                text: row.modelData.body.replace(/\s+/g, " ")
                                elide: Text.ElideRight
                                color: Theme.label
                            }
                        }
                    }
                }
            }
        }

        Note {
            visible: Notifs.history.length === 0
            Layout.topMargin: 14
            text: Notifs.dnd ? "nothing yet · do not disturb is on" : "no notifications"
        }

        footer: [
            BarText {
                Layout.fillWidth: true
                text: "enter opens · del removes · esc closes"
                elide: Text.ElideRight
                color: Theme.off
            }
        ]
    }
}
