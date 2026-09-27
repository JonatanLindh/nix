pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland

// App launcher, opened with super. Same shape as Clip: a layer window of its own so it
// takes the keyboard straight away. Apps start through uwsm, each in its own scope.
// ">cmd" runs a shell command, "=1+2" is a calculator (enter copies the result).
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: bar.panel === "launcher"

    // Visible desktop entries, alphabetical, with lowercased fields for matching.
    readonly property var apps: DesktopEntries.applications.values.filter(e => !e.noDisplay).map(e => ({
                entry: e,
                id: e.id,
                name: e.name,
                lname: e.name.toLowerCase(),
                words: e.name.toLowerCase().split(/[\s\-_.]+/),
                hay: [e.genericName, e.id].concat(e.keywords).join(" ").toLowerCase(),
                detail: e.genericName || e.comment
            })).sort((a, b) => a.lname.localeCompare(b.lname)).map((a, i) => Object.assign(a, {
            order: i
        }))
    readonly property var shown: rank(search.text)

    // Launches per desktop id: { id: { n: count, t: last launch in ms } }, kept in
    // the state dir.
    property var launches: ({})

    // Frequency, weighted towards recent use.
    function frecency(id) {
        const u = launches[id];
        if (!u)
            return 0;
        const days = (Date.now() - u.t) / 86400000;
        return u.n * (days < 1 ? 4 : days < 7 ? 2 : days < 30 ? 1 : 0.5);
    }

    // Rows for a query: [{ kind: "app" | "run" | "calc", name, detail, entry?, value? }].
    // Apps rank by name prefix > word prefix > substring (name, generic name, keywords,
    // id) > subsequence of the name; ties go to the more used, then alphabetical.
    function rank(text) {
        const q = text.trim().toLowerCase();
        if (q.startsWith(">")) {
            const cmd = text.trim().slice(1).trim();
            return cmd ? [
                {
                    kind: "run",
                    name: cmd,
                    detail: "run in shell"
                }
            ] : [];
        }
        if (q.startsWith("="))
            return [calc(q.slice(1))];

        const scored = [];
        for (const a of apps) {
            let tier = -1;
            if (!q)
                tier = 0;
            else if (a.lname.startsWith(q))
                tier = 0;
            else if (a.words.some(w => w.startsWith(q)))
                tier = 1;
            else if (a.lname.includes(q) || a.hay.includes(q))
                tier = 2;
            else if (subsequence(q, a.lname))
                tier = 3;
            if (tier >= 0)
                scored.push({
                    tier: tier,
                    use: frecency(a.id),
                    app: a
                });
        }
        // The JS sort here isn't stable, so alphabetical order is the last key.
        scored.sort((x, y) => x.tier - y.tier || y.use - x.use || x.app.order - y.app.order);
        return scored.map(s => ({
                    kind: "app",
                    name: s.app.name,
                    detail: s.app.detail,
                    entry: s.app
                }));
    }

    function subsequence(q, s) {
        let i = 0;
        for (let j = 0; j < s.length && i < q.length; j++)
            if (s[j] === q[i])
                i++;
        return i === q.length;
    }

    // Arithmetic only: digits, operators, parens, dots. "^" is a power.
    function calc(expr) {
        const e = expr.trim();
        if (!e)
            return {
                kind: "calc",
                name: "=",
                detail: "type an expression",
                value: ""
            };
        if (/^[\d\s+\-*/%^().]+$/.test(e)) {
            try {
                const r = Function(`"use strict"; return (${e.replace(/\^/g, "**")});`)();
                if (typeof r === "number" && isFinite(r)) {
                    const v = String(parseFloat(r.toPrecision(12)));
                    return {
                        kind: "calc",
                        name: "= " + v,
                        detail: "enter copies",
                        value: v
                    };
                }
            } catch (err) {}
        }
        return {
            kind: "calc",
            name: "=",
            detail: "not an expression",
            value: ""
        };
    }

    function launch(item) {
        if (!item)
            return;
        close();
        if (item.kind === "app") {
            const id = item.entry.id;
            record(id);
            Quickshell.execDetached(["uwsm-app", "--", id.endsWith(".desktop") ? id : id + ".desktop"]);
        } else if (item.kind === "run") {
            Quickshell.execDetached(["uwsm-app", "--", "sh", "-c", item.name]);
        } else if (item.kind === "calc" && item.value) {
            Quickshell.clipboardText = item.value;
        }
    }

    function record(id) {
        const l = Object.assign({}, launches);
        const u = l[id] ?? {
            n: 0,
            t: 0
        };
        l[id] = {
            n: u.n + 1,
            t: Date.now()
        };
        launches = l;
        store.setText(JSON.stringify(l));
    }

    function close() {
        bar.panel = "";
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
    implicitHeight: 560
    mask: open ? contentMask : noInput
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:launcher"
    WlrLayershell.keyboardFocus: open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    onOpenChanged: {
        if (!open)
            return;
        search.text = "";
        list.currentIndex = 0;
        list.positionViewAtBeginning();
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
        title: "run"
        onDismissed: root.close()

        actions: [
            BarText {
                visible: !search.text.startsWith(">") && !search.text.startsWith("=")
                text: root.shown.length + (root.shown.length === 1 ? " app" : " apps")
                color: Theme.label
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
                placeholder: "app · >command runs · =expression calculates"
                onTextChanged: list.currentIndex = 0
                Keys.onEscapePressed: root.close()
                Keys.onUpPressed: list.decrementCurrentIndex()
                Keys.onDownPressed: list.incrementCurrentIndex()
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        root.launch(root.shown[list.currentIndex]);
                        event.accepted = true;
                    }
                }
            }
        }

        ListView {
            id: list

            Layout.fillWidth: true
            // About ten rows, then it scrolls.
            Layout.preferredHeight: Math.min(contentHeight, 300)
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
                height: 30
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: list.currentIndex = index
                onClicked: root.launch(modelData)

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

                    BarText {
                        Layout.maximumWidth: list.width * 0.55
                        text: row.modelData.name
                        elide: Text.ElideRight
                        color: row.modelData.kind === "calc" && row.modelData.value ? Theme.accent : row.current ? Theme.bright : Theme.text
                    }

                    BarText {
                        Layout.fillWidth: true
                        Layout.leftMargin: 6
                        text: row.modelData.detail ?? ""
                        elide: Text.ElideRight
                        color: Theme.label
                    }
                }
            }
        }

        Note {
            visible: root.shown.length === 0
            Layout.topMargin: 6
            text: search.text.trim().startsWith(">") ? "type a command" : "no matches"
        }
    }

    FileView {
        id: store

        path: Quickshell.statePath("launcher.json")
        // A missing file just means nothing has been launched yet.
        printErrors: false
        onLoaded: {
            try {
                root.launches = JSON.parse(text()) ?? {};
            } catch (err) {
                root.launches = {};
            }
        }
    }
}
