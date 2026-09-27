import qs
import QtQuick
import Quickshell
import Quickshell.Hyprland

// Drop-down hanging under the right end of the bar. One open at a time per bar.
PopupWindow {
    id: root

    required property var bar
    required property string name
    property alias title: frame.title
    // Hang under the middle of the bar instead of its right end.
    property bool centered: false
    property int panelWidth: Theme.panelWidth
    default property alias content: frame.content
    property alias actions: frame.actions
    property alias footer: frame.footer

    visible: bar.panel === name
    anchor.window: bar
    anchor.rect.x: centered ? Math.round((bar.width - implicitWidth) / 2) : bar.width - implicitWidth - 8
    anchor.rect.y: bar.height + 4
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
        onDismissed: root.close()
    }
}
