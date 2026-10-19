pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Services.Notifications

// The notification daemon (org.freedesktop.Notifications), shared by all bars. Popups
// (NotifPopups) show what's new; everything that isn't transient also goes to the history
// (NotifCenter) and stays tracked there, so its actions still work, until it's removed.
//
// Entries are plain data so they survive the notification closing and config reloads:
// { id, time, app, summary, body, critical, actions: [{ id, text }], hasDefault, resident, transient, held,
//   icon (app icon: theme name, path or url), image (the notification's picture, a url) }
Singleton {
    id: root

    readonly property int historySize: 50
    readonly property int maxPopups: 4

    // Newest first.
    property var history: []
    // Newest first; also carry `timeout` and `remaining` (ms, timeout 0 = sticky).
    property var popups: []
    // Do not disturb: no popups (critical ones still show), but history is kept.
    property alias dnd: persist.dnd
    // Held back by dnd and not yet seen in the history.
    readonly property int unread: history.filter(e => e.held).length
    // Set by the visible popups while hovered or covered by a panel; stops expiry.
    property bool paused: false

    // Ids of notifications still open on the bus (their actions can still be invoked).
    readonly property var liveIds: server.trackedNotifications.values.map(n => n.id)
    // A notification closed by its app takes its popup with it.
    readonly property var shown: popups.filter(p => liveIds.includes(p.id)).slice(0, maxPopups)

    readonly property int tick: 200

    function find(id) {
        return server.trackedNotifications.values.find(n => n.id === id) ?? null;
    }

    function isLive(id) {
        return liveIds.includes(id);
    }

    // Markup is advertised so clients send it predictably, then flattened to plain text:
    // the bar is monospace text only, and history rows need one plain line anyway.
    function plain(s) {
        return (s ?? "").replace(/<br\s*\/?>/gi, "\n").replace(/<[^>]*>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&#(\d+);/g, (m, c) => String.fromCharCode(c)).replace(/&#x([0-9a-f]+);/gi, (m, c) => String.fromCharCode(parseInt(c, 16))).replace(/&amp;/g, "&").trim();
    }

    // mako's old config: 5s, critical sticky; low is quicker. The app's own timeout (ms)
    // wins when it sets one, except for critical.
    function timeoutFor(n) {
        if (n.urgency === NotificationUrgency.Critical)
            return 0;
        if (n.expireTimeout > 0)
            return n.expireTimeout;
        return n.urgency === NotificationUrgency.Low ? 3000 : 5000;
    }

    function entryFor(n) {
        const actions = n.actions.map(a => ({
                    id: a.identifier,
                    text: a.text
                }));
        return {
            id: n.id,
            time: Date.now(),
            app: n.appName || "notification",
            summary: plain(n.summary),
            body: plain(n.body),
            critical: n.urgency === NotificationUrgency.Critical,
            actions: actions.filter(a => a.id !== "default"),
            hasDefault: actions.some(a => a.id === "default"),
            resident: n.resident,
            transient: n.transient,
            icon: n.appIcon || (n.desktopEntry ? DesktopEntries.byId(n.desktopEntry)?.icon ?? "" : ""),
            image: n.image
        };
    }

    // New notifications, and updates to existing ones (same id): both pop up again.
    function receive(n) {
        const entry = entryFor(n);
        if (!entry.transient) {
            const old = history.find(e => e.id === entry.id);
            if (old)
                entry.time = old.time;
            entry.held = dnd && !entry.critical;
            const rest = history.filter(e => e.id !== entry.id);
            for (const e of rest.slice(historySize - 1))
                find(e.id)?.dismiss();
            history = [entry].concat(rest.slice(0, historySize - 1));
        }

        const others = popups.filter(p => p.id !== entry.id);
        if (dnd && !entry.critical) {
            popups = others;
            if (entry.transient)
                n.expire();
            return;
        }
        const timeout = timeoutFor(n);
        popups = [Object.assign({}, entry, {
                timeout: timeout,
                remaining: timeout
            })].concat(others);
    }

    function hidePopup(id) {
        const p = popups.find(p => p.id === id);
        popups = popups.filter(p => p.id !== id);
        // Transient ones never reach the history; they're done once off screen.
        if (p?.transient)
            find(id)?.expire();
    }

    // Invokes an action ("default" when not given). Unless the notification is resident,
    // that closes it, so it leaves the history too.
    function invoke(id, actionId) {
        const n = find(id);
        const action = n?.actions.find(a => a.identifier === (actionId ?? "default"));
        if (!action)
            return false;
        const resident = n.resident;
        action.invoke();
        popups = popups.filter(p => p.id !== id);
        if (!resident)
            history = history.filter(e => e.id !== id);
        return true;
    }

    // Clicking a popup: run its default action if it has one, then get it out of the way.
    function activate(id) {
        if (!invoke(id))
            hidePopup(id);
    }

    // The history has been looked at.
    function markSeen() {
        if (unread > 0)
            history = history.map(e => Object.assign({}, e, {
                    held: false
                }));
    }

    function remove(id) {
        find(id)?.dismiss();
        history = history.filter(e => e.id !== id);
        popups = popups.filter(p => p.id !== id);
    }

    function clear() {
        for (const e of history)
            find(e.id)?.dismiss();
        for (const p of popups)
            find(p.id)?.dismiss();
        history = [];
        popups = [];
    }

    onDndChanged: if (dnd)
        popups = popups.filter(p => p.critical)
    onHistoryChanged: persist.history = JSON.stringify(history)

    NotificationServer {
        id: server

        keepOnReload: true
        persistenceSupported: true
        bodySupported: true
        bodyMarkupSupported: true
        bodyHyperlinksSupported: false
        bodyImagesSupported: false
        actionsSupported: true
        actionIconsSupported: false
        imageSupported: true
        inlineReplySupported: false

        onNotification: n => {
            n.tracked = true;
            // After a config reload the server hands over the notifications it kept;
            // they're already in the history and have had their popup.
            if (n.lastGeneration) {
                if (!root.history.some(e => e.id === n.id))
                    root.history = root.history.concat([root.entryFor(n)]);
                return;
            }
            root.receive(n);
        }
    }

    // Replacements (same id, e.g. progress or volume) update the notification in place.
    Instantiator {
        model: server.trackedNotifications

        delegate: Connections {
            id: conn

            required property Notification modelData

            target: modelData

            function onSummaryChanged() {
                root.receive(conn.modelData);
            }

            function onBodyChanged() {
                root.receive(conn.modelData);
            }
        }
    }

    // History and dnd survive config reloads (the server keeps the notifications themselves).
    PersistentProperties {
        id: persist

        reloadableId: "notifs"

        property bool dnd: false
        property string history: "[]"

        onLoaded: {
            try {
                root.history = JSON.parse(history);
            } catch (e) {
                root.history = [];
            }
        }
    }

    Timer {
        interval: root.tick
        repeat: true
        running: root.shown.length > 0 && !root.paused
        onTriggered: {
            const done = root.shown.filter(p => p.timeout > 0 && (p.remaining -= root.tick) <= 0);
            for (const p of done)
                root.hidePopup(p.id);
        }
    }
}
