pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

// Session menu on super+m (replaces wleave): two rows of three boxes, each with its wleave key.
// Press the key, or move with the arrows and press enter. No confirmation, by request.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: bar.panel === "session"
    readonly property var actions: [
        {
            name: "lock",
            key: "l",
            command: ["loginctl", "lock-session"]
        },
        {
            name: "hibernate",
            key: "h",
            command: ["systemctl", "hibernate"],
            enabled: Power.canHibernate
        },
        {
            name: "logout",
            key: "e",
            command: ["sh", "-c", 'loginctl terminate-user "$USER"']
        },
        {
            name: "shutdown",
            key: "s",
            command: ["systemctl", "poweroff"]
        },
        {
            name: "suspend",
            key: "u",
            command: ["systemctl", "suspend"]
        },
        {
            name: "reboot",
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
    // No anchors: the compositor centres it on the screen.
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: grid.implicitWidth + 2 * Theme.panelPadding
    implicitHeight: grid.implicitHeight + 2 * Theme.panelPadding
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

    // Clicking anywhere else closes it.
    HyprlandFocusGrab {
        windows: [root]
        active: root.grabReady && root.open
        onCleared: root.close()
    }

    Rectangle {
        id: keys

        visible: root.open

        anchors.fill: parent
        color: Theme.barBg
        focus: true
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

        GridLayout {
            id: grid

            anchors.centerIn: parent
            columns: 3
            rowSpacing: 8
            columnSpacing: 8

            Repeater {
                model: root.actions

                MouseArea {
                    id: box

                    required property var modelData
                    required property int index
                    readonly property bool usable: modelData.enabled !== false
                    readonly property bool current: root.selected === index

                    implicitWidth: 132
                    implicitHeight: 84
                    hoverEnabled: true
                    cursorShape: usable ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onEntered: root.selected = index
                    onClicked: root.run(modelData)

                    Rectangle {
                        anchors.fill: parent
                        color: box.current && box.usable ? Theme.hover : "transparent"
                        border.width: 1
                        border.color: box.current && box.usable ? Theme.accent : Theme.border
                    }

                    BarText {
                        anchors.centerIn: parent
                        text: box.modelData.name
                        color: !box.usable ? Theme.off : box.current ? Theme.bright : Theme.text
                    }

                    BarText {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: 8
                        text: box.modelData.key
                        color: !box.usable ? Theme.off : box.current ? Theme.accent : Theme.label
                    }
                }
            }
        }
    }
}
