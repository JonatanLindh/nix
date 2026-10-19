pragma ComponentBehavior: Bound
import qs
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Workspace pills: the one shown on this monitor is a grey pill, the rest are numbers,
// bright when they hold windows and faint when empty.
Row {
    id: root

    required property ShellScreen screen
    readonly property HyprlandMonitor monitor: Hyprland.monitorFor(screen)

    // Slots: the block of five around this monitor's workspace (1-5, 6-10; the desktop pins
    // one block per output), empty ones included so there's somewhere to click, plus any
    // other workspace that lives here. Empty slots that exist on another monitor are left out.
    readonly property var slots: {
        const all = Hyprland.workspaces.values.filter(ws => ws.id > 0);
        const here = all.filter(ws => ws.monitor === root.monitor);
        const current = root.monitor?.activeWorkspace?.id ?? here[0]?.id ?? 1;
        const first = Math.floor((Math.max(1, current) - 1) / 5) * 5 + 1;
        const ids = [first, first + 1, first + 2, first + 3, first + 4].filter(id => !all.some(ws => ws.id === id && ws.monitor !== root.monitor));
        for (const ws of here)
            if (!ids.includes(ws.id))
                ids.push(ws.id);
        return ids.sort((a, b) => a - b).map(id => ({
                    id: id,
                    ws: here.find(ws => ws.id === id) ?? null
                }));
    }

    leftPadding: 4
    rightPadding: 4
    spacing: 4

    Repeater {
        model: root.slots

        MouseArea {
            id: pill

            required property var modelData
            // qmllint disable unresolved-type
            readonly property HyprlandWorkspace ws: modelData.ws
            // qmllint enable unresolved-type
            readonly property bool shown: ws?.active ?? false
            readonly property bool occupied: (ws?.toplevels.values.length ?? 0) > 0

            anchors.verticalCenter: parent?.verticalCenter
            implicitWidth: shown ? 34 : Math.max(22, label.implicitWidth + 10)
            implicitHeight: 20
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            // By number, not ws.activate(): that sends a hyprlang dispatch, which the Lua config
            // rejects. Also works for empty slots, which have no workspace object yet.
            onClicked: Hyprland.dispatch(`hl.dsp.focus({ workspace = ${modelData.id} })`)

            Behavior on implicitWidth {
                NumberAnimation {
                    duration: Theme.medium
                    easing.type: Easing.OutCubic
                }
            }

            Rectangle {
                anchors.fill: parent
                radius: height / 2
                color: pill.shown ? Theme.fillActive : pill.containsMouse ? Theme.fillHover : "transparent"
                Behavior on color {
                    ColorAnimation {
                        duration: Theme.fast
                    }
                }
            }

            UiText {
                id: label
                anchors.centerIn: parent
                text: pill.ws?.name ?? pill.modelData.id
                font.pixelSize: Theme.small
                font.weight: Font.Medium
                font.features: {
                    "tnum": 1
                }
                color: pill.shown ? Theme.text : pill.ws?.urgent ? Theme.warn : Qt.alpha(Theme.text, pill.occupied ? 0.92 : 0.42)
            }
        }
    }
}
