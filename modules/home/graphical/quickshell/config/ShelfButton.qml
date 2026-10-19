import qs
import QtQuick

// Bar button that shows/hides the shelf (on this bar's monitor), with a count badge.
// Also a drop target: files dropped on it go on the shelf, and holding a drag over it
// for a moment opens the shelf so the drop can land there instead (spring-loading).
IconButton {
    id: root

    required property var bar
    property bool dropHover: false

    icon: "inbox"
    iconSize: 17
    badge: Shelf.count > 0 ? String(Shelf.count) : ""
    active: !dropHover && Shelf.shown && Shelf.screenName === bar.screen?.name
    restColor: dropHover ? Qt.alpha(Theme.accent, 0.32) : "transparent"
    iconColor: dropHover ? Theme.accent : Theme.text
    onClicked: Shelf.toggle(bar.screen?.name)

    Timer {
        id: spring
        interval: 400
        onTriggered: {
            Shelf.screenName = root.bar.screen?.name ?? "";
            Shelf.shown = true;
        }
    }

    DropArea {
        // Children otherwise land in IconButton's content Row.
        parent: root
        anchors.fill: parent
        keys: ["text/uri-list"]
        onEntered: drag => {
            drag.accept(Qt.CopyAction);
            root.dropHover = true;
            spring.restart();
        }
        onExited: {
            root.dropHover = false;
            spring.stop();
        }
        onDropped: drop => {
            root.dropHover = false;
            spring.stop();
            const paths = drop.urls.map(u => decodeURIComponent(u.toString().replace(/^file:\/\//, ""))).filter(p => p.startsWith("/"));
            if (paths.length) {
                // Shown here first, so add() doesn't move it to the focused monitor.
                Shelf.screenName = root.bar.screen?.name ?? "";
                Shelf.shown = true;
                Shelf.add(paths);
                Shelf.say(paths.length === 1 ? "Added 1 file" : `Added ${paths.length} files`);
            }
            drop.accept(Qt.CopyAction);
        }
    }
}
