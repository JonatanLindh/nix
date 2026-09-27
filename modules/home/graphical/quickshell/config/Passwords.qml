pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland

// 1Password on super+shift+p, through qs-op (scripts/qs-op.sh) and the app integration.
// Secrets never reach the bar: the list and field views are metadata only, and picking
// something has qs-op put it straight onto the clipboard (sensitive, so cliphist skips it,
// and cleared after 30s). Enter pastes the password, ctrl+enter the username, alt+enter
// the one-time code; add shift to only copy. Tab opens an item's fields.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool open: bar.panel === "op"

    // Item metadata (no secrets), kept in memory between opens so the list shows at once and
    // refreshed in the background on every open (about a second once 1Password has
    // authorized the CLI). Only the first open after the bar starts has to wait.
    // [{ id, title, category, vault, user, urls }]
    property var items: []
    // Waiting for the first list (nothing to show yet).
    property bool loading: false
    // Loading for a while means 1Password is showing its authorization dialog: let go of the
    // keyboard so it can be typed into, and take it back once the list arrives.
    property bool authorizing: false
    property string error: ""
    // Item whose fields are shown (field view), or null for the item list.
    property var item: null
    property var fields: []

    readonly property var shown: item ? matchFields(search.text) : rank(search.text)

    // Picks per item id: { id: { n: count, t: last pick in ms } }, kept in the state dir.
    property var uses: ({})

    // Frequency, weighted towards recent use (same as the launcher).
    function frecency(id) {
        const u = uses[id];
        if (!u)
            return 0;
        const days = (Date.now() - u.t) / 86400000;
        return u.n * (days < 1 ? 4 : days < 7 ? 2 : days < 30 ? 1 : 0.5);
    }

    function record(id) {
        const all = Object.assign({}, uses);
        const u = all[id] ?? {
            n: 0,
            t: 0
        };
        all[id] = {
            n: u.n + 1,
            t: Date.now()
        };
        uses = all;
        usageStore.setText(JSON.stringify(all));
    }

    // Title prefix > word prefix > substring (title, username, urls) > subsequence of the
    // title; ties (and an empty query) go to the most used, then alphabetical. At most 50 rows.
    function rank(text) {
        const q = text.trim().toLowerCase();
        if (!q)
            return items.slice().sort((a, b) => frecency(b.id) - frecency(a.id) || a.title.localeCompare(b.title)).slice(0, 50);
        const scored = [];
        for (const i of items) {
            const t = i.title.toLowerCase();
            let s = 0;
            if (t.startsWith(q))
                s = 4;
            else if (t.split(/[\s\-_.@]+/).some(w => w.startsWith(q)))
                s = 3;
            else if (t.includes(q) || i.user.toLowerCase().includes(q) || i.urls.toLowerCase().includes(q))
                s = 2;
            else if (subsequence(q, t))
                s = 1;
            if (s > 0)
                scored.push({
                    item: i,
                    score: s
                });
        }
        return scored.sort((a, b) => b.score - a.score || frecency(b.item.id) - frecency(a.item.id) || a.item.title.localeCompare(b.item.title)).slice(0, 50).map(x => x.item);
    }

    function subsequence(q, s) {
        let i = 0;
        for (const c of s) {
            if (c === q[i])
                i++;
            if (i === q.length)
                return true;
        }
        return false;
    }

    function matchFields(text) {
        const q = text.trim().toLowerCase();
        return q ? fields.filter(f => f.label.toLowerCase().includes(q)) : fields;
    }

    function close() {
        bar.panel = "";
    }

    // what: a field id, "otp", or one of the shortcuts "password" / "username".
    function take(target, what, paste) {
        if (!target)
            return;
        record(target.id);
        close();
        copier.command = ["qs-op", "copy", target.vault, target.id, what].concat(paste ? ["paste"] : []);
        copier.running = true;
    }

    function showFields(target) {
        if (!target)
            return;
        item = target;
        fields = [];
        search.text = "";
        list.currentIndex = 0;
        fieldQuery.command = ["qs-op", "fields", target.vault, target.id];
        fieldQuery.running = true;
    }

    function back() {
        item = null;
        fields = [];
        search.text = "";
        list.currentIndex = 0;
    }

    screen: bar.screen
    // Created once and kept: opening just flips state, with no new surface and no layer
    // open animation. While closed it draws nothing, takes no clicks and no keyboard.
    visible: true
    anchors.top: true
    // qmllint disable unqualified unresolved-type
    margins.top: Theme.barHeight + 4
    // qmllint enable unqualified unresolved-type
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: 560
    // Fixed height: resizing the layer surface while typing makes it jitter (Hyprland
    // animates layer geometry). The frame sizes itself inside; input only reaches the frame.
    implicitHeight: 560
    mask: open ? contentMask : noInput
    color: "transparent"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:op"
    WlrLayershell.keyboardFocus: open ? (authorizing ? WlrKeyboardFocus.None : WlrKeyboardFocus.Exclusive) : WlrKeyboardFocus.None

    onOpenChanged: {
        item = null;
        fields = [];
        error = "";
        search.text = "";
        list.currentIndex = 0;
        if (!open)
            return;
        loading = items.length === 0;
        authorizing = false;
        if (loading)
            slowLoad.restart();
        listing.running = true;
        Qt.callLater(() => search.forceActiveFocus());
    }

    onLoadingChanged: {
        if (loading)
            return;
        slowLoad.stop();
        authorizing = false;
        Qt.callLater(() => search.forceActiveFocus());
    }

    Timer {
        id: slowLoad
        interval: 1500
        onTriggered: root.authorizing = root.loading
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

    // Clicking anywhere else closes it, except while loading: 1Password's authorization
    // dialog may need the click.
    HyprlandFocusGrab {
        windows: [root]
        active: root.grabReady && root.open && !root.loading
        onCleared: root.close()
    }

    PanelFrame {
        id: frame

        visible: root.open

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: implicitHeight
        title: root.item ? root.item.title.toLowerCase() : "1password"
        onDismissed: root.close()

        actions: [
            BarText {
                text: root.authorizing ? "authorize in 1password…" : root.loading ? "loading…" : root.item ? root.shown.length + " fields" : root.items.length + " items"
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
                text: root.item ? "field" : "find"
                color: Theme.label
            }

            Input {
                id: search

                Layout.fillWidth: true
                placeholder: root.item ? "enter pastes · shift copies · esc back" : "enter password · ctrl user · alt code · +shift copies · tab fields"
                onTextChanged: list.currentIndex = 0
                Keys.onUpPressed: list.decrementCurrentIndex()
                Keys.onDownPressed: list.incrementCurrentIndex()
                Keys.onPressed: event => {
                    const current = root.shown[list.currentIndex];
                    const paste = !(event.modifiers & Qt.ShiftModifier);
                    if (event.key === Qt.Key_Escape) {
                        if (root.item)
                            root.back();
                        else
                            root.close();
                    } else if (event.key === Qt.Key_Backspace && root.item && search.text === "") {
                        root.back();
                    } else if (event.key === Qt.Key_Tab || (event.key === Qt.Key_Right && !root.item && search.cursorPosition === search.text.length)) {
                        if (!root.item)
                            root.showFields(current);
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        if (root.item)
                            root.take(root.item, current?.type === "OTP" ? "otp" : current?.id, paste);
                        else if (event.modifiers & Qt.ControlModifier)
                            root.take(current, "username", paste);
                        else if (event.modifiers & Qt.AltModifier)
                            root.take(current, "otp", paste);
                        else
                            root.take(current, "password", paste);
                    } else {
                        return;
                    }
                    event.accepted = true;
                }
            }
        }

        Note {
            visible: root.error !== ""
            Layout.topMargin: 4
            Layout.bottomMargin: 6
            text: root.error
            color: Theme.warn
        }

        ListView {
            id: list

            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 360)
            clip: true
            model: root.shown
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0

            delegate: MouseArea {
                id: row

                required property var modelData
                required property int index
                readonly property bool current: ListView.isCurrentItem
                // Field rows carry a type; item rows a category.
                readonly property bool isField: modelData.type !== undefined

                width: list.width
                height: 30
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: list.currentIndex = index
                // Click pastes (the password, or this field); shift+click only copies.
                onClicked: mouse => {
                    const paste = !(mouse.modifiers & Qt.ShiftModifier);
                    if (row.isField)
                        root.take(root.item, row.modelData.type === "OTP" ? "otp" : row.modelData.id, paste);
                    else
                        root.take(row.modelData, "password", paste);
                }

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
                        Layout.preferredWidth: row.isField ? 140 : -1
                        Layout.fillWidth: !row.isField
                        text: row.isField ? (row.modelData.type === "OTP" ? "one-time code" : row.modelData.label) : row.modelData.title
                        elide: Text.ElideRight
                        color: row.current ? Theme.bright : Theme.text
                    }

                    BarText {
                        Layout.fillWidth: row.isField
                        Layout.maximumWidth: row.isField ? -1 : 200
                        text: {
                            const m = row.modelData;
                            if (row.isField)
                                return m.value === null ? "••••••••" : m.value.replace(/\s+/g, " ");
                            return m.category === "LOGIN" ? m.user : m.category.toLowerCase().replace("_", " ");
                        }
                        elide: Text.ElideRight
                        color: Theme.label
                    }

                    Action {
                        visible: !row.isField && row.containsMouse
                        text: "›"
                        onClicked: root.showFields(row.modelData)
                    }
                }
            }
        }

        Note {
            visible: !root.loading && root.shown.length === 0 && root.error === ""
            Layout.topMargin: 6
            text: root.item ? "no fields" : root.items.length === 0 ? "no items" : "no matches"
        }
    }

    FileView {
        id: usageStore

        path: Quickshell.statePath("passwords.json")
        // A missing file just means nothing has been picked yet.
        printErrors: false
        onLoaded: {
            try {
                root.uses = JSON.parse(text()) ?? {};
            } catch (err) {
                root.uses = {};
            }
        }
    }

    Process {
        id: listing
        command: ["qs-op", "list"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.loading = false;
                try {
                    root.items = JSON.parse(text).sort((a, b) => a.title.localeCompare(b.title));
                } catch (e) {
                    root.items = [];
                }
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim())
                root.error = text.trim().split("\n").pop()
        }
        onRunningChanged: if (!running)
            root.loading = false
    }

    Process {
        id: fieldQuery
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    root.fields = JSON.parse(text);
                } catch (e) {
                    root.fields = [];
                }
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim())
                root.error = text.trim().split("\n").pop()
        }
    }

    // Runs after the window has closed, so failures go out as a notification.
    Process {
        id: copier
        stderr: StdioCollector {
            onStreamFinished: {
                const message = text.trim().split("\n").pop();
                if (message)
                    Quickshell.execDetached(["notify-send", "-a", "1password", "couldn't copy", message]);
            }
        }
    }
}
