import qs
import QtQuick

// "dnd" in the bar while do-not-disturb is on, with how many notifications it has held
// back. Click opens the history, right click turns dnd off.
Field {
    id: root

    property var bar: null

    visible: Notifs.dnd
    value: "dnd"
    valueColor: Theme.accent
    open: bar?.panel === "notif"
    onClicked: mouse => {
        if (mouse.button === Qt.RightButton)
            Notifs.dnd = false;
        else
            root.bar?.toggle("notif");
    }

    BarText {
        visible: Notifs.unread > 0
        text: " " + Notifs.unread
        color: Theme.label
    }
}
