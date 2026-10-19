import qs
import QtQuick

// A glass surface: translucent fill (blurred by Hyprland behind it) and a hairline border.
Rectangle {
    id: root

    radius: Theme.panelRadius
    color: Theme.glass
    border.width: 1
    border.color: Theme.glassBorder
}
