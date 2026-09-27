pragma ComponentBehavior: Bound
import qs
import QtQuick
import Quickshell
import Quickshell.Hyprland

Row {
    id: root

    required property ShellScreen screen
    readonly property HyprlandMonitor monitor: Hyprland.monitorFor(screen)

    spacing: 16

    Repeater {
        // Only this monitor's workspaces; Hyprland rules pin them per output on the desktop.
        model: Hyprland.workspaces.values.filter(ws => ws.id > 0 && ws.monitor === root.monitor).sort((a, b) => a.id - b.id)

        MouseArea {
            id: ws

            required property HyprlandWorkspace modelData
            // Shown on this monitor; accented when this monitor also has focus.
            readonly property bool shown: modelData.active
            readonly property bool focused: shown && modelData.focused

            implicitWidth: label.implicitWidth
            implicitHeight: Theme.barHeight
            cursorShape: Qt.PointingHandCursor
            onClicked: modelData.activate()

            BarText {
                id: label
                anchors.verticalCenter: parent.verticalCenter
                text: ws.modelData.name
                font.bold: ws.focused
                color: ws.modelData.urgent ? Theme.warn : ws.focused ? Theme.accent : ws.shown ? Theme.bright : Theme.label
            }

            Rectangle {
                visible: ws.focused
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 5
                width: parent.width
                height: 1
                color: Theme.accent
            }
        }
    }
}
