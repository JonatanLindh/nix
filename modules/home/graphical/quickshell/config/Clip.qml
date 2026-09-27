pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland

// Clipboard history from cliphist (services.cliphist), opened with super+v. A layer
// window of its own rather than a popup of the bar: opened from a keybind, it has to
// take the keyboard without a click, which a popup of the bar can't.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: bar.panel === "clip"

    // [{ id, line, text, image, mime, size }]
    property var entries: []
    readonly property var shown: {
        const q = search.text.trim().toLowerCase();
        return q ? entries.filter(e => e.text.toLowerCase().includes(q)) : entries;
    }
    readonly property string thumbDir: Quickshell.env("XDG_RUNTIME_DIR") + "/quickshell-clip"
    // Bumped when thumbnails are written so image sources reload.
    property int thumbGeneration: 0

    function load() {
        listing.running = true;
    }

    // Copies the entry, and with `paste` also pastes it into the window that was focused
    // (qs-paste sends that app's paste shortcut once focus is back; see scripts/qs-paste.sh).
    function pick(entry, paste) {
        if (!entry)
            return;
        close();
        paster.command = ["sh", "-c", 'cliphist decode "$1" | wl-copy && [ "$2" = paste ] && qs-paste', "sh", entry.id, paste ? "paste" : "copy"];
        paster.running = true;
    }

    function close() {
        bar.panel = "";
    }

    function remove(entry) {
        if (!entry)
            return;
        run.command = ["sh", "-c", 'printf "%s" "$1" | cliphist delete', "sh", entry.line];
        run.running = true;
    }

    function wipe() {
        run.command = ["cliphist", "wipe"];
        run.running = true;
    }

    screen: bar.screen
    // Created once and kept: opening just flips state, with no new surface and no layer
    // open animation. While closed it draws nothing, takes no clicks and no keyboard.
    visible: true
    // Anchored to the top edge only, so the compositor centres it horizontally.
    anchors.top: true
    // qmllint disable unqualified unresolved-type
    margins.top: Theme.barHeight + 4
    // qmllint enable unqualified unresolved-type
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: 560
    // Fixed height: resizing the layer surface on every keystroke makes it jitter (Hyprland
    // animates layer geometry). The frame sizes itself inside; input only reaches the frame.
    implicitHeight: 640
    mask: open ? contentMask : noInput
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:clip"
    WlrLayershell.keyboardFocus: open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    onOpenChanged: {
        if (!open)
            return;
        search.text = "";
        list.currentIndex = 0;
        load();
        Qt.callLater(() => search.forceActiveFocus());
    }

    Region {
        id: contentMask
        item: frame
    }

    Region {
        id: noInput
    }

    // Activating the grab in the same tick the window opens gets it cleared right away (which
    // closes the window); give it a moment, like Panel.qml does.
    property bool grabReady: false

    Timer {
        id: grabDelay
        interval: 50
        running: root.open
        onRunningChanged: if (running)
            root.grabReady = false
        onTriggered: root.grabReady = true
    }

    // Clicking anywhere else closes it.
    HyprlandFocusGrab {
        windows: [root]
        active: root.grabReady && root.open
        onCleared: root.close()
    }

    PanelFrame {
        id: frame

        visible: root.open

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: implicitHeight
        title: "clip"
        onDismissed: root.close()

        actions: [
            BarText {
                text: root.shown.length + (root.shown.length === 1 ? " item" : " items")
                color: Theme.label
            },
            Action {
                text: "wipe"
                onClicked: root.wipe()
            }
        ]

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Theme.panelPadding
            Layout.rightMargin: Theme.panelPadding
            Layout.topMargin: 14
            Layout.bottomMargin: 8
            spacing: 12

            BarText {
                text: "find"
                color: Theme.label
            }

            Input {
                id: search

                Layout.fillWidth: true
                placeholder: "filter · enter pastes · shift+enter copies · shift+del deletes"
                onTextChanged: list.currentIndex = 0
                Keys.onEscapePressed: root.close()
                Keys.onUpPressed: list.decrementCurrentIndex()
                Keys.onDownPressed: list.incrementCurrentIndex()
                Keys.onPressed: event => {
                    const shift = event.modifiers & Qt.ShiftModifier;
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        root.pick(root.shown[list.currentIndex], !shift);
                        event.accepted = true;
                    } else if (event.key === Qt.Key_Delete && shift) {
                        root.remove(root.shown[list.currentIndex]);
                        event.accepted = true;
                    }
                }
            }
        }

        ListView {
            id: list

            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 420)
            clip: true
            model: root.shown
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0

            delegate: MouseArea {
                id: row

                required property var modelData
                required property int index
                readonly property bool current: ListView.isCurrentItem

                width: list.width
                height: modelData.image ? 64 : 30
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: list.currentIndex = index
                // Click pastes like Enter; shift+click only copies.
                onClicked: mouse => root.pick(modelData, !(mouse.modifiers & Qt.ShiftModifier))

                Rectangle {
                    anchors.fill: parent
                    color: row.current ? Theme.hover : "transparent"
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.panelPadding
                    anchors.rightMargin: Theme.panelPadding
                    spacing: 8

                    BarText {
                        Layout.preferredWidth: 8
                        text: row.current ? ">" : ""
                        color: Theme.accent
                    }

                    Image {
                        visible: row.modelData.image
                        Layout.preferredWidth: 80
                        Layout.preferredHeight: 52
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        cache: false
                        sourceSize.width: 160
                        source: row.modelData.image ? `file://${root.thumbDir}/${row.modelData.id}?${root.thumbGeneration}` : ""
                    }

                    BarText {
                        Layout.fillWidth: true
                        text: row.modelData.image ? `${row.modelData.mime} ${row.modelData.size}` : row.modelData.text.replace(/\s+/g, " ")
                        elide: Text.ElideRight
                        color: row.current ? Theme.bright : Theme.text
                    }

                    Action {
                        visible: row.containsMouse
                        text: "×"
                        onClicked: root.remove(row.modelData)
                    }
                }
            }
        }

        Note {
            visible: root.shown.length === 0
            Layout.topMargin: 6
            text: root.entries.length === 0 ? "clipboard history is empty" : "no matches"
        }
    }

    Process {
        id: listing
        command: ["cliphist", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.entries = text.split("\n").filter(l => l.includes("\t")).map(line => {
                    const tab = line.indexOf("\t");
                    const preview = line.slice(tab + 1);
                    // "[[ binary data 1.2 MiB png 1920x1080 ]]"
                    const img = preview.match(/^\[\[ binary data (.+?) (\w+) (\d+x\d+) \]\]$/);
                    return {
                        id: line.slice(0, tab),
                        line: line,
                        text: img ? "image " + img[2] + " " + img[3] : preview,
                        image: !!img,
                        mime: img ? img[2] : "",
                        size: img ? img[3] + " · " + img[1] : ""
                    };
                });
                const ids = root.entries.filter(e => e.image).slice(0, 30).map(e => e.id);
                if (ids.length > 0) {
                    thumbs.command = ["sh", "-c", 'dir="$1"; shift; mkdir -p "$dir"; for id; do [ -s "$dir/$id" ] || cliphist decode "$id" > "$dir/$id"; done', "sh", root.thumbDir].concat(ids);
                    thumbs.running = true;
                }
            }
        }
    }

    // Decode image entries to files once so they can be shown as thumbnails.
    Process {
        id: thumbs
        onRunningChanged: if (!running)
            root.thumbGeneration++
    }

    // Its own process: the paste happens after the window has closed.
    Process {
        id: paster
    }

    Process {
        id: run
        onRunningChanged: if (!running && root.open)
            root.load()
    }
}
