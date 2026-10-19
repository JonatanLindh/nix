import qs
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Drop-down glass panel hanging under the bar. One open at a time per bar.
// side: "right" (under the status icons), "center" (under the clock) or "left".
// backTo: a panel this one can be opened from (e.g. "net"); while it was, the header
// shows a back arrow that returns there.
PopupWindow {
    id: root

    required property var bar
    required property string name
    property alias title: frame.title
    property string side: "right"
    property int sideMargin: 8
    property int panelWidth: Theme.panelWidth
    property string backTo: ""
    default property alias content: frame.content
    property alias actions: frame.actions
    property alias footer: frame.footer
    property alias spacing: frame.spacing

    visible: bar.panel === name
    anchor.window: bar
    anchor.rect.x: side === "center" ? Math.round((bar.width - implicitWidth) / 2) : side === "left" ? sideMargin : bar.width - implicitWidth - sideMargin
    anchor.rect.y: bar.height + Theme.panelGap
    implicitWidth: panelWidth
    implicitHeight: frame.implicitHeight
    color: "transparent"

    function close() {
        bar.panel = "";
    }

    // Activating the grab in the same tick the popup maps leaves the popup out of it,
    // which blocks all input to the panel; give the surface a moment first.
    Timer {
        id: grabDelay
        interval: 50
        onTriggered: grab.active = root.visible
    }

    onVisibleChanged: {
        if (visible)
            grabDelay.restart();
        else
            grab.active = false;
    }

    HyprlandFocusGrab {
        id: grab
        windows: [root, root.bar]
        onCleared: root.close()
    }

    PanelFrame {
        id: frame
        anchors.fill: parent
        showBack: root.backTo !== "" && (root.bar.panelFrom ?? "") === root.backTo
        onDismissed: root.close()
        onBack: root.bar.open(root.backTo)
    }
}
