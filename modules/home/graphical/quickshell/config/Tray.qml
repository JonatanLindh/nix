pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Widgets
import Quickshell.Services.SystemTray

Row {
    id: root

    required property var bar

    visible: SystemTray.items.values.length > 0
    spacing: 10

    Repeater {
        model: SystemTray.items

        IconImage {
            id: icon

            required property SystemTrayItem modelData

            anchors.verticalCenter: parent.verticalCenter
            implicitSize: 14
            source: {
                // Some apps name icons only present as -symbolic in Adwaita.
                const src = modelData.icon;
                const prefix = "image://icon/";
                if (!src.startsWith(prefix) || src.includes("?"))
                    return src;
                const name = src.slice(prefix.length);
                return Quickshell.iconPath(name, true) || Quickshell.iconPath(name + "-symbolic", "image-missing");
            }
            opacity: modelData.status === Status.Passive ? 0.5 : 0.85

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                cursorShape: Qt.PointingHandCursor
                onClicked: mouse => {
                    const item = icon.modelData;
                    if (mouse.button === Qt.MiddleButton) {
                        item.secondaryActivate();
                    } else if (mouse.button === Qt.RightButton || item.onlyMenu) {
                        if (item.hasMenu) {
                            const pos = icon.mapToItem(root.bar.contentItem, 0, icon.height);
                            item.display(root.bar, pos.x, pos.y);
                        }
                    } else {
                        item.activate();
                    }
                }
                onWheel: wheel => icon.modelData.scroll(wheel.angleDelta.y, false)
            }
        }
    }
}
