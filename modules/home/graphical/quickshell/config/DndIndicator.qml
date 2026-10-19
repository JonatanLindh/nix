import qs
import QtQuick

// Notifications button: a bell, or a moon while do-not-disturb is on, with a dot while
// dnd has held notifications back that haven't been seen. Click opens the history, right
// click toggles dnd.
IconButton {
    id: root

    required property var bar

    icon: Notifs.dnd ? "moon" : "bell"
    iconSize: 17
    iconColor: Notifs.dnd ? Theme.screen : Theme.text
    active: bar.panel === "notif"
    onClicked: mouse => {
        if (mouse.button === Qt.RightButton)
            Notifs.dnd = !Notifs.dnd;
        else
            root.bar.toggle("notif");
    }

    Rectangle {
        visible: Notifs.unread > 0
        parent: root
        // A 7px dot with a 1.5px ring around it (borders draw inwards).
        x: root.width - 4.5 - width
        y: 2.5
        width: 10
        height: 10
        radius: 5
        color: Theme.accent
        border.width: 1.5
        border.color: Qt.rgba(8 / 255, 18 / 255, 22 / 255, 0.8)
    }
}
