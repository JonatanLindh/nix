pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Widgets
import Quickshell.Services.SystemTray

Row {
    id: root

    required property var bar

    visible: SystemTray.items.values.length > 0
    spacing: 2

    Repeater {
        model: SystemTray.items

        IconButton {
            id: button

            required property SystemTrayItem modelData

            anchors.verticalCenter: parent?.verticalCenter

            onClicked: mouse => {
                const item = button.modelData;
                if (mouse.button === Qt.MiddleButton) {
                    item.secondaryActivate();
                } else if (mouse.button === Qt.RightButton || item.onlyMenu) {
                    if (item.hasMenu) {
                        // Hangs from the bar's bottom edge, like the panels.
                        const pos = button.mapToItem(root.bar.contentItem, 0, 0);
                        item.display(root.bar, pos.x, root.bar.height);
                    }
                } else {
                    item.activate();
                }
            }
            onWheel: wheel => button.modelData.scroll(wheel.angleDelta.y, false)

            IconImage {
                id: icon
                anchors.verticalCenter: parent.verticalCenter
                implicitSize: 15
                source: {
                    // Some apps name icons only present as -symbolic in Adwaita.
                    const src = button.modelData.icon;
                    const prefix = "image://icon/";
                    if (!src.startsWith(prefix) || src.includes("?"))
                        return src;
                    const name = src.slice(prefix.length);
                    return Quickshell.iconPath(name, true) || Quickshell.iconPath(name + "-symbolic", "image-missing");
                }
                opacity: button.modelData.status === Status.Passive ? 0.5 : 0.85
                // Mostly grey so coloured app icons sit with the monochrome bar icons; one
                // asking for attention keeps its colour.
                layer.enabled: button.modelData.status !== Status.NeedsAttention
                layer.effect: MultiEffect {
                    saturation: -0.85
                }
            }
        }
    }
}
