pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland

// The capture overlay on one monitor, GNOME-style: a frozen frame of the screen, dimmed
// outside what will be captured, and a toolbar (selection / screen / window, photo / video,
// pointer, the capture button). State and the capture itself live in Screenshot.qml.
// Keys: esc closes, enter/space captures, s/c/w pick the mode, v toggles video, p the pointer.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var modelData
    readonly property bool focusedHere: Hyprland.focusedMonitor?.name === modelData.name
    // Shown once the frozen frame is up and grim has the stills screenshots are cut from.
    readonly property bool shown: Screenshot.active && frame.hasContent && stills.done

    // Selection in this monitor's logical coordinates, or null.
    property var sel: null
    // Window under the pointer (window mode), in local coordinates, or null.
    property var hovered: null

    // What a capture would take right now, local coordinates (null = nothing yet).
    readonly property var target: Screenshot.mode === "screen" ? {
        x: 0,
        y: 0,
        w: width,
        h: height
    } : Screenshot.mode === "window" ? (Screenshot.pointerMonitor === modelData.name ? hovered : null) : sel

    screen: modelData
    // Created once and kept, like the launcher: opening just shows the content, so the
    // frozen frame can be grabbed before anything of ours is on screen.
    visible: true
    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    mask: shown ? null : noInput
    WlrLayershell.layer: WlrLayer.Overlay
    // Not "quickshell:…": the blur rule would blur the whole screen behind an opaque frame.
    WlrLayershell.namespace: "quickshell-screenshot"
    WlrLayershell.keyboardFocus: shown && focusedHere ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    Region {
        id: noInput
    }

    function local(w) {
        return {
            x: w.x - modelData.x,
            y: w.y - modelData.y,
            w: w.w,
            h: w.h
        };
    }

    function windowAt(px, py) {
        for (const w of Screenshot.windows) {
            const l = local(w);
            if (px >= l.x && px < l.x + l.w && py >= l.y && py < l.y + l.h)
                return l;
        }
        return null;
    }

    function capture() {
        const t = target;
        if (!t)
            return;
        if (Screenshot.mode === "selection")
            Screenshot.selections = Object.assign({}, Screenshot.selections, {
                [modelData.name]: sel
            });
        if (!Screenshot.video) {
            // Screenshots are cut from the stills taken as the overlay opened rather than
            // grabbed again once we're gone, so menus and hovers that close when the overlay
            // takes focus still make it in. The buffer's own scale: Qt's devicePixelRatio
            // rounds fractional scales up.
            const scale = frame.sourceSize.width / width;
            const r = v => Math.round(v * scale);
            Screenshot.saveFrozen(Screenshot.pointer ? stills.withPointer : stills.plain, `${r(t.w)}x${r(t.h)}+${r(t.x)}+${r(t.y)}`);
        } else if (Screenshot.mode === "screen")
            Screenshot.capture(0, 0, width, height, modelData.name);
        else
            Screenshot.capture(t.x + modelData.x, t.y + modelData.y, t.w, t.h);
    }

    Connections {
        target: Screenshot
        function onActiveChanged() {
            if (Screenshot.active) {
                root.sel = Screenshot.selections[root.modelData.name] ?? null;
                root.hovered = null;
                frame.captureFrame();
                frameCursor.captureFrame();
                stills.done = false;
                stills.running = true;
            }
        }
    }

    // This monitor at full resolution, with and without the pointer, before we're on screen.
    Process {
        id: stills
        property bool done: false
        readonly property string plain: `${Screenshot.stillsDir}/${root.modelData.name}.ppm`
        readonly property string withPointer: `${Screenshot.stillsDir}/${root.modelData.name}-pointer.ppm`
        command: ["sh", "-c", 'mkdir -p "$1" || exit; grim -t ppm -o "$2" "$3" & grim -c -t ppm -o "$2" "$4" & wait', "sh", Screenshot.stillsDir, root.modelData.name, plain, withPointer]
        onExited: done = true
    }

    ScreencopyView {
        id: frame
        anchors.fill: parent
        visible: root.shown && !Screenshot.pointer
        captureSource: root.modelData
        live: false
        paintCursor: false
    }

    // The same frame with the pointer in it, shown while "include pointer" is on.
    ScreencopyView {
        id: frameCursor
        anchors.fill: parent
        visible: root.shown && Screenshot.pointer
        captureSource: root.modelData
        live: false
        paintCursor: true
    }

    Item {
        id: content
        anchors.fill: parent
        visible: root.shown
        focus: true

        Keys.onPressed: event => {
            const k = event.key;
            if (k === Qt.Key_Escape)
                Screenshot.close();
            else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space)
                root.capture();
            else if (k === Qt.Key_S)
                Screenshot.mode = "selection";
            else if (k === Qt.Key_C)
                Screenshot.mode = "screen";
            else if (k === Qt.Key_W)
                Screenshot.mode = "window";
            else if (k === Qt.Key_V)
                Screenshot.video = !Screenshot.video;
            else if (k === Qt.Key_P)
                Screenshot.pointer = !Screenshot.pointer;
            else
                return;
            event.accepted = true;
        }

        // Dim everything outside the target (all of it when there's none yet).
        readonly property var cut: root.target ?? {
            x: 0,
            y: 0,
            w: 0,
            h: 0
        }
        readonly property color dim: Qt.rgba(0, 0, 0, 0.45)

        Rectangle {
            x: 0
            y: 0
            width: parent.width
            height: content.cut.y
            color: content.dim
        }
        Rectangle {
            x: 0
            y: content.cut.y + content.cut.h
            width: parent.width
            height: parent.height - y
            color: content.dim
        }
        Rectangle {
            x: 0
            y: content.cut.y
            width: content.cut.x
            height: content.cut.h
            color: content.dim
        }
        Rectangle {
            x: content.cut.x + content.cut.w
            y: content.cut.y
            width: parent.width - x
            height: content.cut.h
            color: content.dim
        }

        // Outline of the target (not for the whole screen).
        Rectangle {
            visible: root.target !== null && Screenshot.mode !== "screen"
            x: content.cut.x - 1
            y: content.cut.y - 1
            width: content.cut.w + 2
            height: content.cut.h + 2
            color: "transparent"
            border.width: 2
            border.color: "white"
        }

        // Size of the selection, just under its top-left corner.
        Rectangle {
            visible: Screenshot.mode === "selection" && root.sel !== null && !drag.pressed
            x: content.cut.x
            y: content.cut.y + content.cut.h + 10 + height < content.height ? content.cut.y + content.cut.h + 10 : content.cut.y - height - 10
            width: sizeText.implicitWidth + 16
            height: 24
            radius: 12
            color: Qt.rgba(14 / 255, 26 / 255, 30 / 255, 0.85)

            UiText {
                id: sizeText
                anchors.centerIn: parent
                text: `${Math.round(content.cut.w)} × ${Math.round(content.cut.h)}`
                tabular: true
                font.pixelSize: Theme.small
            }
        }

        MouseArea {
            id: drag

            // "new" | "move" | "tl" | "tr" | "bl" | "br"
            property string action: ""
            property point start
            property var startSel: null

            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Screenshot.mode === "selection" ? Qt.CrossCursor : Qt.PointingHandCursor

            function handleAt(px, py) {
                const s = root.sel;
                if (!s)
                    return "";
                const near = (ax, ay) => Math.abs(px - ax) <= 12 && Math.abs(py - ay) <= 12;
                if (near(s.x, s.y))
                    return "tl";
                if (near(s.x + s.w, s.y))
                    return "tr";
                if (near(s.x, s.y + s.h))
                    return "bl";
                if (near(s.x + s.w, s.y + s.h))
                    return "br";
                if (px > s.x && px < s.x + s.w && py > s.y && py < s.y + s.h)
                    return "move";
                return "";
            }

            onPositionChanged: mouse => {
                Screenshot.pointerMonitor = root.modelData.name;
                if (Screenshot.mode === "window") {
                    root.hovered = root.windowAt(mouse.x, mouse.y);
                    return;
                }
                if (!pressed || Screenshot.mode !== "selection")
                    return;
                const dx = mouse.x - start.x, dy = mouse.y - start.y;
                const s0 = startSel;
                const clampX = v => Math.max(0, Math.min(width, v));
                const clampY = v => Math.max(0, Math.min(height, v));
                if (action === "move") {
                    root.sel = {
                        x: Math.max(0, Math.min(width - s0.w, s0.x + dx)),
                        y: Math.max(0, Math.min(height - s0.h, s0.y + dy)),
                        w: s0.w,
                        h: s0.h
                    };
                    return;
                }
                // Corner drags (and a new selection) move one corner, the opposite stays.
                let x1 = s0.x, y1 = s0.y, x2 = s0.x + s0.w, y2 = s0.y + s0.h;
                if (action === "new") {
                    x2 = clampX(mouse.x);
                    y2 = clampY(mouse.y);
                } else {
                    if (action === "tl" || action === "bl")
                        x1 = clampX(s0.x + dx);
                    if (action === "tr" || action === "br")
                        x2 = clampX(s0.x + s0.w + dx);
                    if (action === "tl" || action === "tr")
                        y1 = clampY(s0.y + dy);
                    if (action === "bl" || action === "br")
                        y2 = clampY(s0.y + s0.h + dy);
                }
                root.sel = {
                    x: Math.min(x1, x2),
                    y: Math.min(y1, y2),
                    w: Math.abs(x2 - x1),
                    h: Math.abs(y2 - y1)
                };
            }

            onPressed: mouse => {
                start = Qt.point(mouse.x, mouse.y);
                if (Screenshot.mode !== "selection")
                    return;
                action = handleAt(mouse.x, mouse.y) || "new";
                startSel = action === "new" ? {
                    x: mouse.x,
                    y: mouse.y,
                    w: 0,
                    h: 0
                } : root.sel;
            }

            onReleased: mouse => {
                if (Screenshot.mode === "selection") {
                    // A click without dragging doesn't leave a zero-size selection behind.
                    if (root.sel && (root.sel.w < 4 || root.sel.h < 4))
                        root.sel = null;
                    return;
                }
                // Screen and window modes capture on click.
                if (Screenshot.mode === "window")
                    root.hovered = root.windowAt(mouse.x, mouse.y);
                root.capture();
            }
        }

        // Corner handles of the selection.
        Repeater {
            model: Screenshot.mode === "selection" && root.sel ? [[0, 0], [1, 0], [0, 1], [1, 1]] : []

            Rectangle {
                required property var modelData
                x: content.cut.x + content.cut.w * modelData[0] - width / 2
                y: content.cut.y + content.cut.h * modelData[1] - height / 2
                width: 16
                height: 16
                radius: 8
                color: "white"
                border.width: 1
                border.color: Qt.rgba(0, 0, 0, 0.25)
            }
        }

        // Close, top right.
        MouseArea {
            visible: root.focusedHere
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: 24
            width: 48
            height: 48
            cursorShape: Qt.PointingHandCursor
            hoverEnabled: true
            onClicked: Screenshot.close()

            Rectangle {
                anchors.fill: parent
                radius: 24
                color: parent.containsMouse ? Qt.rgba(40 / 255, 52 / 255, 56 / 255, 0.92) : Qt.rgba(14 / 255, 26 / 255, 30 / 255, 0.85)
                border.width: 1
                border.color: Theme.glassBorder
            }
            Icon {
                anchors.centerIn: parent
                name: "close"
                size: 20
                strokeWidth: 2.4
            }
        }

        // Toolbar, bottom centre of the focused monitor.
        Rectangle {
            id: toolbar
            visible: root.focusedHere
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 48
            width: bar.implicitWidth + 32
            height: bar.implicitHeight + 32
            radius: 28
            // More solid than panels: there's no live blur over the frozen frame.
            color: Qt.rgba(14 / 255, 26 / 255, 30 / 255, 0.88)
            border.width: 1
            border.color: Theme.glassBorder

            // Clicks on the toolbar don't reach the selection underneath.
            MouseArea {
                anchors.fill: parent
            }

            ColumnLayout {
                id: bar
                anchors.centerIn: parent
                spacing: 14

                RowLayout {
                    spacing: 8

                    Repeater {
                        model: [
                            {
                                id: "selection",
                                icon: "select",
                                text: "Selection"
                            },
                            {
                                id: "screen",
                                icon: "monitor",
                                text: "Screen"
                            },
                            {
                                id: "window",
                                icon: "windows",
                                text: "Window"
                            }
                        ]

                        MouseArea {
                            id: modeTile
                            required property var modelData
                            readonly property bool on: Screenshot.mode === modelData.id

                            implicitWidth: 92
                            implicitHeight: 72
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Screenshot.mode = modelData.id

                            Rectangle {
                                anchors.fill: parent
                                radius: 16
                                color: modeTile.on ? Theme.fillActive : modeTile.containsMouse ? Theme.fill : "transparent"
                            }
                            Column {
                                anchors.centerIn: parent
                                spacing: 8
                                Icon {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    name: modeTile.modelData.icon
                                    size: 26
                                    strokeWidth: 1.7
                                }
                                UiText {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: modeTile.modelData.text
                                    font.pixelSize: Theme.small
                                }
                            }
                        }
                    }
                }

                Item {
                    Layout.fillWidth: true
                    implicitHeight: 64

                    // Photo / video.
                    Rectangle {
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        width: 84
                        height: 36
                        radius: 18
                        color: Theme.fill

                        Row {
                            anchors.centerIn: parent
                            spacing: 2
                            Repeater {
                                model: [
                                    {
                                        video: false,
                                        icon: "photo"
                                    },
                                    {
                                        video: true,
                                        icon: "video"
                                    }
                                ]
                                MouseArea {
                                    id: kind
                                    required property var modelData
                                    readonly property bool on: Screenshot.video === modelData.video
                                    width: 40
                                    height: 30
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Screenshot.video = modelData.video
                                    Rectangle {
                                        anchors.fill: parent
                                        radius: 15
                                        color: kind.on ? Theme.white : "transparent"
                                    }
                                    Icon {
                                        anchors.centerIn: parent
                                        name: kind.modelData.icon
                                        size: 17
                                        strokeWidth: 2
                                        color: kind.on ? Theme.ink : Theme.text
                                    }
                                }
                            }
                        }
                    }

                    // The capture button: a white ring around a disc (red for video).
                    MouseArea {
                        id: shutter
                        anchors.centerIn: parent
                        width: 64
                        height: 64
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        enabled: root.target !== null
                        opacity: enabled ? 1 : 0.5
                        onClicked: root.capture()

                        Rectangle {
                            anchors.fill: parent
                            radius: 32
                            color: "transparent"
                            border.width: 4
                            border.color: "white"
                        }
                        Rectangle {
                            anchors.centerIn: parent
                            width: shutter.pressed ? 42 : 48
                            height: width
                            radius: width / 2
                            color: Screenshot.video ? Theme.warn : "white"
                        }
                    }

                    // Include the pointer (screenshots only; recordings always show it).
                    MouseArea {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: 40
                        height: 40
                        enabled: !Screenshot.video
                        opacity: enabled ? 1 : 0.4
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Screenshot.pointer = !Screenshot.pointer

                        Rectangle {
                            anchors.fill: parent
                            radius: 20
                            color: Screenshot.pointer ? Theme.white : Theme.fill
                        }
                        Icon {
                            anchors.centerIn: parent
                            name: "pointer"
                            size: 17
                            strokeWidth: 2
                            color: Screenshot.pointer ? Theme.ink : Theme.text
                        }
                    }
                }
            }
        }
    }
}
