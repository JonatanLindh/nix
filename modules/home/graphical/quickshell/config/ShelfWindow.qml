pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import QtQuick.Shapes
import Quickshell
import Quickshell.Wayland

// The shelf itself: a small glass card pinned under the left of the bar. Files dropped on it
// are kept; each row drags back out into any app; tools below work on the selection (or
// everything). Never takes the keyboard and stays up until hidden, so it can sit beside the
// app being dragged into. One per screen; only the one on Shelf.screenName shows.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var modelData
    property bool dropHover: false

    readonly property var inline: Shelf.actions.length > 6 ? Shelf.actions.slice(0, 5).concat([
            {
                id: "more",
                label: "More",
                icon: "more"
            }
        ]) : Shelf.actions
    readonly property var rest: Shelf.actions.length > 6 ? Shelf.actions.slice(5) : []
    readonly property var optAction: Shelf.actions.find(a => a.id === Shelf.open && a.options) ?? null

    screen: modelData
    visible: Shelf.shown && (Shelf.screenName === modelData.name || (Shelf.screenName === "" && Quickshell.screens[0] === modelData))
    anchors {
        top: true
        left: true
    }
    // qmllint disable unqualified unresolved-type
    // Ignoring exclusive zones (below), so clear the bar explicitly.
    margins.top: Theme.barHeight + Theme.panelGap
    margins.left: Theme.barPadding
    // qmllint enable unqualified unresolved-type
    implicitWidth: 300
    // Fixed height: resizing the layer surface whenever a card unfolds made it jiggle
    // (Hyprland animates layer geometry). The card sizes itself inside; input only reaches it.
    implicitHeight: 900
    exclusionMode: ExclusionMode.Ignore
    mask: Region {
        item: card
    }
    color: "transparent"
    WlrLayershell.namespace: "quickshell:shelf"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

    function pathOf(url) {
        return decodeURIComponent(url.toString().replace(/^file:\/\//, ""));
    }

    function isOn(id) {
        return Shelf.open === id || (id === "funnel" && !!Shelf.funnel && Shelf.funnelCard) || (id === "wormhole" && !!Shelf.wormhole && Shelf.wormholeCard);
    }

    // The icon for a shelf item's kind, on its tint.
    readonly property var tints: ({
            folder: "#3f78b8",
            file: "#b0533f",
            image: "#8c5cb0",
            code: "#2f8f7f",
            archive: "#7a6a4a",
            video: "#b0753f",
            link: "#3f8f5a"
        })

    Glass {
        id: card

        width: parent.width
        implicitHeight: column.implicitHeight + 16
        radius: 18
        border.color: root.dropHover ? "transparent" : Theme.glassBorder

        DropArea {
            anchors.fill: parent
            keys: ["text/uri-list"]
            onEntered: drag => {
                drag.accept(Qt.CopyAction);
                root.dropHover = true;
            }
            onExited: root.dropHover = false
            onDropped: drop => {
                root.dropHover = false;
                const paths = drop.urls.map(u => root.pathOf(u)).filter(p => p.startsWith("/"));
                if (paths.length) {
                    Shelf.add(paths);
                    Shelf.say(paths.length === 1 ? "Added 1 file" : `Added ${paths.length} files`);
                }
                drop.accept(Qt.CopyAction);
            }
        }

        ColumnLayout {
            id: column

            anchors.fill: parent
            anchors.margins: 8
            spacing: 2

            // Header.
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 8
                Layout.rightMargin: 4
                Layout.topMargin: 2
                Layout.bottomMargin: 4
                spacing: 8

                Icon {
                    name: "inbox"
                    size: 15
                    color: Theme.textDim
                }
                UiText {
                    Layout.fillWidth: true
                    textFormat: Text.StyledText
                    text: `Shelf <font color="${Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)}">· ${Shelf.count}</font>`
                    font.pixelSize: Theme.small
                    font.weight: Font.DemiBold
                }
                TextButton {
                    readonly property int temps: Shelf.items.filter(i => i.temp).length

                    visible: Shelf.count > 0
                    // Results are deleted with the rest; say so.
                    text: temps ? `Clear · deletes ${temps}` : "Clear"
                    onClicked: Shelf.clear()
                }
                CircleButton {
                    visible: Shelf.count === 0
                    size: 22
                    iconSize: 11
                    icon: "close"
                    onClicked: Shelf.shown = false
                }
            }

            // Items.
            Repeater {
                model: Shelf.items

                ShelfRow {}
            }

            // Drag everything out at once.
            MouseArea {
                id: dragAll

                visible: Shelf.count > 1
                Layout.fillWidth: true
                Layout.topMargin: 4
                implicitHeight: 32
                hoverEnabled: true
                cursorShape: drag.active ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                drag.target: allProxy
                drag.threshold: 6
                onPressed: allLabel.grabToImage(result => allProxy.Drag.imageSource = result.url)

                DragProxy {
                    id: allProxy
                    paths: Shelf.items.map(i => i.path)
                    active: dragAll.drag.active
                }

                // Dashed outline.
                Shape {
                    anchors.fill: parent
                    preferredRendererType: Shape.CurveRenderer
                    ShapePath {
                        strokeColor: Qt.rgba(1, 1, 1, dragAll.containsMouse ? 0.36 : 0.22)
                        strokeWidth: 1
                        strokeStyle: ShapePath.DashLine
                        dashPattern: [4, 3]
                        fillColor: "transparent"
                        PathRectangle {
                            x: 0.5
                            y: 0.5
                            width: dragAll.width - 1
                            height: dragAll.height - 1
                            radius: 10
                        }
                    }
                }
                Row {
                    id: allLabel
                    anchors.centerIn: parent
                    spacing: 8
                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "grip"
                        size: 13
                        color: Theme.textDim
                    }
                    UiText {
                        anchors.verticalCenter: parent.verticalCenter
                        text: `Drag all ${Shelf.count}`
                        color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.8)
                        font.pixelSize: Theme.small
                    }
                }
            }

            // Empty.
            Column {
                visible: Shelf.count === 0
                Layout.fillWidth: true
                Layout.topMargin: 14
                Layout.bottomMargin: 12
                spacing: 4

                UiText {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: "Drop files here from any app, or"
                    dim: true
                    font.pixelSize: Theme.small
                }
                Row {
                    anchors.horizontalCenter: parent.horizontalCenter
                    spacing: 4
                    Kbd {
                        text: "alt"
                    }
                    Kbd {
                        text: "↵"
                    }
                    UiText {
                        height: 20
                        text: " / alt-click a file in the"
                        dim: true
                        font.pixelSize: Theme.small
                    }
                }
                UiText {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: "launcher’s file search."
                    dim: true
                    font.pixelSize: Theme.small
                }
            }

            // Tools.
            Rectangle {
                visible: Shelf.count > 0
                Layout.fillWidth: true
                Layout.leftMargin: 4
                Layout.rightMargin: 4
                Layout.topMargin: 6
                Layout.bottomMargin: 4
                implicitHeight: 1
                color: Theme.divider
            }
            UiText {
                visible: Shelf.count > 0
                Layout.leftMargin: 6
                Layout.bottomMargin: 2
                text: Shelf.selected.length ? `For ${Shelf.targets.length} selected file${Shelf.targets.length === 1 ? "" : "s"}` : Shelf.count === 1 ? "For this file" : `For all ${Shelf.count} files`
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                font.pixelSize: Theme.tiny
            }
            GridLayout {
                visible: Shelf.count > 0
                Layout.fillWidth: true
                columns: 3
                rowSpacing: 4
                columnSpacing: 4

                Repeater {
                    model: root.inline

                    ToolTile {}
                }
            }

            // Options for convert / resize.
            ColumnLayout {
                visible: root.optAction !== null && Shelf.count > 0
                Layout.fillWidth: true
                Layout.leftMargin: 4
                Layout.rightMargin: 4
                Layout.topMargin: 6
                spacing: 6

                UiText {
                    Layout.leftMargin: 2
                    text: root.optAction?.options?.label ?? ""
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                    font.pixelSize: Theme.tiny
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: root.optAction?.options?.items ?? []

                        PillButton {
                            required property var modelData
                            text: modelData.label
                            onClicked: Shelf.run(root.optAction.id, modelData.id)
                        }
                    }
                }
            }

            // The tools that didn't fit.
            ColumnLayout {
                visible: Shelf.open === "more" && root.rest.length > 0
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 2

                Repeater {
                    model: root.rest

                    MenuRow {
                        required property var modelData
                        icon: modelData.icon
                        text: modelData.label
                        onClicked: Shelf.activate(modelData)
                    }
                }
            }

            // Taildrop targets.
            ColumnLayout {
                visible: Shelf.open === "taildrop"
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 2

                UiText {
                    Layout.leftMargin: 8
                    Layout.topMargin: 2
                    Layout.bottomMargin: 2
                    text: Shelf.peersLoading ? "Looking for devices…" : Shelf.peers.length ? "Send with Taildrop to" : "No devices to send to"
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                    font.pixelSize: Theme.tiny
                }
                Repeater {
                    model: Shelf.open === "taildrop" ? Shelf.peers : []

                    MenuRow {
                        required property var modelData
                        dot: modelData.online ? Theme.online : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.3)
                        text: modelData.name
                        note: modelData.online ? "" : "Offline"
                        enabled: modelData.online
                        onClicked: {
                            Shelf.run("taildrop", modelData.name);
                            Shelf.say(`Sending to ${modelData.name}…`);
                        }
                    }
                }
            }

            // Wormhole: the code (and its QR) until the other side has it.
            Rectangle {
                visible: !!Shelf.wormhole && Shelf.wormholeCard
                Layout.fillWidth: true
                Layout.topMargin: 6
                implicitHeight: holeColumn.implicitHeight + 24
                radius: 14
                color: Qt.rgba(1, 1, 1, 0.07)

                ColumnLayout {
                    id: holeColumn

                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        QrCode {
                            qr: Shelf.wormhole?.qr ?? null
                            box: 96
                            pad: 6
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            UiText {
                                text: "Wormhole code"
                                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                                font.pixelSize: Theme.tiny
                            }
                            UiText {
                                Layout.fillWidth: true
                                text: Shelf.wormhole?.code ?? ""
                                mono: true
                                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                                elide: Text.ElideNone
                                font.pixelSize: 15
                                font.weight: Font.Medium
                                lineHeight: 1.15
                            }
                            UiText {
                                Layout.fillWidth: true
                                text: "Scan with a wormhole app, or type the code"
                                wrapMode: Text.Wrap
                                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
                                font.pixelSize: Theme.tiny
                            }
                        }
                    }

                    MouseArea {
                        id: holeCopy

                        property bool copied: false

                        Layout.fillWidth: true
                        implicitHeight: 32
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Shelf.copy(`wormhole receive ${Shelf.wormhole?.code ?? ""}`);
                            copied = true;
                        }

                        Connections {
                            target: Shelf
                            function onWormholeChanged() {
                                holeCopy.copied = false;
                            }
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: 9
                            color: holeCopy.containsMouse ? Qt.rgba(0, 0, 0, 0.32) : Qt.rgba(0, 0, 0, 0.22)
                        }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8
                            spacing: 8
                            UiText {
                                Layout.fillWidth: true
                                text: `wormhole receive ${Shelf.wormhole?.code ?? ""}`
                                mono: true
                                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.8)
                                font.pixelSize: 11
                            }
                            UiText {
                                text: holeCopy.copied ? "Copied" : "Copy"
                                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
                                font.pixelSize: Theme.tiny
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Rectangle {
                            implicitWidth: 7
                            implicitHeight: 7
                            radius: 3.5
                            color: Theme.accent
                            Rectangle {
                                anchors.centerIn: parent
                                width: 13
                                height: 13
                                radius: 6.5
                                color: Qt.alpha(Theme.accent, 0.28)
                                z: -1
                            }
                        }
                        UiText {
                            Layout.fillWidth: true
                            text: Shelf.wormholeState === "sending" ? "Sending…" : "Waiting for the other side…"
                            color: Theme.textDim
                            font.pixelSize: Theme.small
                        }
                        PillButton {
                            text: "Cancel"
                            onClicked: Shelf.cancel(Shelf.wormhole)
                        }
                    }
                }
            }

            // A link's address as a QR, to open it on a phone.
            Rectangle {
                visible: Shelf.open === "qr" && !!Shelf.linkQr
                Layout.fillWidth: true
                Layout.topMargin: 6
                implicitHeight: linkColumn.implicitHeight + 24
                radius: 14
                color: Qt.rgba(1, 1, 1, 0.07)

                ColumnLayout {
                    id: linkColumn

                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    QrCode {
                        Layout.alignment: Qt.AlignHCenter
                        qr: Shelf.linkQr
                        box: 136
                        pad: 8
                    }
                    UiText {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: Shelf.linkQr?.url ?? ""
                        mono: true
                        elide: Text.ElideMiddle
                        color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.8)
                        font.pixelSize: Theme.tiny
                    }
                }
            }

            // Funnel: the public link and its QR until stopped.
            Rectangle {
                visible: !!Shelf.funnel && Shelf.funnelCard
                Layout.fillWidth: true
                Layout.topMargin: 6
                implicitHeight: funnelColumn.implicitHeight + 24
                radius: 14
                color: Qt.rgba(1, 1, 1, 0.07)

                ColumnLayout {
                    id: funnelColumn

                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10

                    QrCode {
                        Layout.alignment: Qt.AlignHCenter
                        qr: Shelf.funnel?.qr ?? null
                        box: 136
                        pad: 8
                    }
                    UiText {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: Shelf.funnel?.url ?? ""
                        mono: true
                        elide: Text.ElideMiddle
                        color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.8)
                        font.pixelSize: Theme.tiny
                    }
                    Row {
                        Layout.alignment: Qt.AlignHCenter
                        spacing: 6
                        PillButton {
                            text: "Copy link"
                            onClicked: {
                                Shelf.copy(Shelf.funnel?.url ?? "");
                                Shelf.say("Link copied");
                            }
                        }
                        PillButton {
                            text: "Stop sharing"
                            onClicked: Shelf.cancel(Shelf.funnel)
                        }
                    }
                    UiText {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: "Public via Tailscale Funnel until you stop it"
                        color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
                        font.pixelSize: Theme.tiny
                    }
                }
            }

            // Status.
            RowLayout {
                visible: Shelf.status !== "" && Shelf.count > 0
                Layout.fillWidth: true
                Layout.leftMargin: 8
                Layout.rightMargin: 8
                Layout.topMargin: 6
                Layout.bottomMargin: 2
                spacing: 8

                Rectangle {
                    id: statusDot

                    implicitWidth: 6
                    implicitHeight: 6
                    radius: 3
                    color: Shelf.statusError ? Theme.warn : Theme.accent
                    // Pulses while a tool runs.
                    SequentialAnimation on opacity {
                        running: Shelf.busy.length > 0
                        loops: Animation.Infinite
                        onRunningChanged: if (!running)
                            statusDot.opacity = 1
                        NumberAnimation {
                            to: 0.25
                            duration: 500
                        }
                        NumberAnimation {
                            to: 1
                            duration: 500
                        }
                    }
                }
                UiText {
                    Layout.fillWidth: true
                    text: Shelf.status
                    color: Shelf.statusError ? Theme.warnText : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.75)
                    font.pixelSize: Theme.small
                }
            }
        }

        // "Drop to add" over everything while files hover.
        Rectangle {
            visible: root.dropHover
            anchors.fill: parent
            anchors.margins: 6
            radius: 13
            color: Qt.tint(Qt.rgba(14 / 255, 26 / 255, 30 / 255, 0.85), Qt.alpha(Theme.accent, 0.14))

            Shape {
                anchors.fill: parent
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: Theme.accent
                    strokeWidth: 1.5
                    strokeStyle: ShapePath.DashLine
                    dashPattern: [4, 3]
                    fillColor: "transparent"
                    PathRectangle {
                        x: 0.75
                        y: 0.75
                        width: card.width - 12 - 1.5
                        height: card.height - 12 - 1.5
                        radius: 13
                    }
                }
            }
            UiText {
                anchors.centerIn: parent
                text: "Drop to add to the shelf"
                font.weight: Font.Medium
            }
        }
    }

    // ---- Pieces

    // Carries file URIs out of the shelf (a proxy, so the row itself stays put).
    // A link carries its address instead, so it drops into a browser or chat as the link.
    component DragProxy: Item {
        property var paths: []
        property string url: ""
        property bool active: false

        width: 1
        height: 1
        Drag.dragType: Drag.Automatic
        Drag.supportedActions: Qt.CopyAction
        Drag.proposedAction: Qt.CopyAction
        Drag.hotSpot.x: 13
        Drag.hotSpot.y: 13
        Drag.mimeData: url !== "" ? {
            "text/uri-list": url + "\r\n",
            "text/plain": url
        } : {
            "text/uri-list": Shelf.uriList(paths),
            "text/plain": paths.join("\n")
        }
        Drag.active: active
    }

    component ShelfRow: MouseArea {
        id: row

        required property var modelData
        readonly property bool picked: Shelf.selected.includes(modelData.path)

        Layout.fillWidth: true
        implicitHeight: 40
        hoverEnabled: true
        cursorShape: drag.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
        onClicked: Shelf.toggleSelected(modelData.path)
        drag.target: proxy
        drag.threshold: 6
        onPressed: tile.grabToImage(result => proxy.Drag.imageSource = result.url)

        DragProxy {
            id: proxy
            paths: [row.modelData.path]
            url: row.modelData.url ?? ""
            active: row.drag.active
        }

        Rectangle {
            anchors.fill: parent
            radius: Theme.rowRadius
            color: row.picked ? Theme.fillHover : row.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
            border.width: row.picked ? 1.5 : 0
            border.color: Theme.accent
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 6
            spacing: 10

            Rectangle {
                id: tile
                implicitWidth: 26
                implicitHeight: 26
                radius: 7
                color: root.tints[row.modelData.kind] ?? root.tints.file

                Icon {
                    anchors.centerIn: parent
                    name: row.modelData.kind
                    size: 15
                    strokeWidth: 2.1
                    color: "white"
                }
            }
            // Capped at its own width so the place sits right after it; shrinks (eliding)
            // before the place does.
            UiText {
                Layout.fillWidth: true
                Layout.maximumWidth: implicitWidth
                text: row.modelData.name
                font.pixelSize: 13
            }
            // Its folder, when another item has the same name.
            UiText {
                readonly property string place: Shelf.places[row.modelData.path] ?? ""
                visible: place !== ""
                Layout.maximumWidth: 120
                text: place
                dim: true
                font.pixelSize: Theme.small
            }
            Item {
                Layout.fillWidth: true
            }
            // A result: the shelf's own file, deleted when it leaves the shelf (drag it out to
            // keep it).
            Rectangle {
                visible: row.modelData.temp
                implicitWidth: tempText.implicitWidth + 14
                implicitHeight: 18
                radius: 9
                color: Theme.fill
                border.width: 1
                border.color: Theme.glassBorder
                UiText {
                    id: tempText
                    anchors.centerIn: parent
                    text: "Temp"
                    color: Theme.textDim
                    font.pixelSize: 10
                    font.weight: Font.DemiBold
                }
            }
            CircleButton {
                opacity: row.containsMouse ? 1 : 0
                size: 22
                iconSize: row.modelData.temp ? 13 : 11
                icon: row.modelData.temp ? "trash" : "close"
                hoverIconColor: row.modelData.temp ? Theme.warnText : Theme.text
                onClicked: Shelf.remove(row.modelData.path)
            }
        }
    }

    component ToolTile: MouseArea {
        id: tool

        required property var modelData
        readonly property bool on: root.isOn(modelData.id)
        readonly property bool working: Shelf.busy.includes(modelData.id)

        Layout.fillWidth: true
        Layout.preferredWidth: 1
        implicitHeight: 52
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: Shelf.activate(modelData)

        Rectangle {
            anchors.fill: parent
            radius: 12
            color: tool.on ? Qt.rgba(1, 1, 1, 0.14) : tool.containsMouse ? Qt.rgba(1, 1, 1, 0.1) : "transparent"
        }
        Column {
            anchors.centerIn: parent
            spacing: 4
            opacity: tool.working ? 0.5 : 1

            Icon {
                anchors.horizontalCenter: parent.horizontalCenter
                name: tool.modelData.icon
                size: 17
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.85)
            }
            UiText {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(implicitWidth, tool.width - 8)
                horizontalAlignment: Text.AlignHCenter
                text: tool.modelData.label
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.85)
                font.pixelSize: Theme.tiny
            }
        }
    }

    // A compact list row (more tools, Taildrop devices).
    component MenuRow: MouseArea {
        id: menuRow

        property string icon: ""
        property color dot: "transparent"
        property string text: ""
        property string note: ""

        Layout.fillWidth: true
        implicitHeight: 32
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor

        Rectangle {
            anchors.fill: parent
            radius: 9
            color: menuRow.enabled && menuRow.containsMouse ? Qt.rgba(1, 1, 1, 0.1) : "transparent"
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            spacing: 10

            Item {
                implicitWidth: 18
                implicitHeight: 18
                visible: menuRow.icon !== "" || menuRow.dot.a > 0
                Icon {
                    visible: menuRow.icon !== ""
                    anchors.centerIn: parent
                    name: menuRow.icon
                    size: 17
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.85)
                }
                Rectangle {
                    visible: menuRow.icon === ""
                    anchors.centerIn: parent
                    width: 8
                    height: 8
                    radius: 4
                    color: menuRow.dot
                }
            }
            UiText {
                Layout.fillWidth: true
                text: menuRow.text
                color: menuRow.enabled ? Theme.text : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.4)
                font.pixelSize: 13
            }
            UiText {
                visible: menuRow.note !== ""
                text: menuRow.note
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.5)
                font.pixelSize: Theme.tiny
            }
        }
    }

    // A QR code on a light card, drawn from the daemon's module path.
    component QrCode: Rectangle {
        id: qrBox

        property var qr: null
        property int box: 96
        property int pad: 6

        implicitWidth: box
        implicitHeight: box
        radius: box > 100 ? 12 : 10
        color: Theme.white

        Shape {
            x: qrBox.pad
            y: qrBox.pad
            width: qrBox.qr?.size ?? 1
            height: width
            scale: (qrBox.box - qrBox.pad * 2) / width
            transformOrigin: Item.TopLeft
            // Square modules stay crisp; the curve renderer would soften their edges.
            preferredRendererType: Shape.GeometryRenderer
            antialiasing: false

            ShapePath {
                strokeColor: "transparent"
                fillColor: Theme.ink
                PathSvg {
                    path: qrBox.qr?.path ?? ""
                }
            }
        }
    }

    // A plain text button ("Clear").
    component TextButton: MouseArea {
        id: textButton

        property string text: ""

        implicitWidth: label.implicitWidth + 12
        implicitHeight: label.implicitHeight + 4
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor

        Rectangle {
            anchors.fill: parent
            radius: 6
            color: textButton.containsMouse ? Theme.fillHover : "transparent"
        }
        UiText {
            id: label
            anchors.centerIn: parent
            text: textButton.text
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.72)
            font.pixelSize: Theme.small
        }
    }
}
