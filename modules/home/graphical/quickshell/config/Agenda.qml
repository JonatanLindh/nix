pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

// Read-only calendar events from `qs-calendar-events` (modules/home/graphical/quickshell/calendar),
// which fetches the iCal feeds in ~/.config/quickshell-calendar/urls and caches them itself.
Singleton {
    id: root

    // [{ start: Date, end: Date, allDay, title, location, calendar }], sorted by start.
    property var events: []
    property var errors: []
    property bool configured: true
    property bool loaded: false
    property date updated
    // "yyyy-MM-dd" -> true for every day something happens on.
    property var busy: ({})
    readonly property bool multiple: new Set(events.map(e => e.calendar)).size > 1

    function refresh() {
        if (helper.running)
            return;
        // From the start of last month, a bit over a year ahead.
        const now = new Date();
        helper.command = ["qs-calendar-events", "--from", Qt.formatDate(new Date(now.getFullYear(), now.getMonth() - 1, 1), "yyyy-MM-dd"), "--days", "430"];
        helper.output = false;
        helper.running = true;
    }

    function key(date) {
        return Qt.formatDate(date, "yyyy-MM-dd");
    }

    function dayStart(date) {
        return new Date(date.getFullYear(), date.getMonth(), date.getDate());
    }

    function addDays(date, n) {
        return new Date(date.getFullYear(), date.getMonth(), date.getDate() + n);
    }

    // All-day events are plain dates (local); timed ones carry their UTC offset.
    function parse(s) {
        const d = s.match(/^(\d{4})-(\d{2})-(\d{2})$/);
        return d ? new Date(+d[1], +d[2] - 1, +d[3]) : new Date(s);
    }

    // Events overlapping the given day, in start order.
    function on(date) {
        const from = dayStart(date), to = addDays(from, 1);
        return events.filter(e => e.start < to && (e.end > from || (e.end <= e.start && e.start >= from)));
    }

    // The next `n` events that haven't ended, leaving out `except`.
    function upcoming(n, except) {
        const now = new Date();
        return events.filter(e => (e.end > now || (e.end <= e.start && e.start >= now)) && !except.includes(e)).slice(0, n);
    }

    function load(text) {
        const d = JSON.parse(text);
        const evs = (d.events ?? []).map(e => ({
                    start: parse(e.start),
                    end: parse(e.end),
                    allDay: !!e.allDay,
                    title: e.title ?? "",
                    location: e.location ?? "",
                    calendar: e.calendar ?? ""
                }));
        const days = {};
        for (const e of evs) {
            // The end is exclusive; an event without length still marks its day.
            const last = e.end > e.start ? new Date(e.end.getTime() - 1) : e.start;
            for (let day = dayStart(e.start); day <= last; day = addDays(day, 1))
                days[key(day)] = true;
        }
        events = evs;
        busy = days;
        errors = d.errors ?? [];
        configured = d.configured ?? false;
        loaded = true;
        updated = new Date();
    }

    Process {
        id: helper

        property bool output: false

        stdout: StdioCollector {
            onStreamFinished: {
                if (!text.trim())
                    return;
                helper.output = true;
                try {
                    root.load(text);
                } catch (e) {
                    root.errors = ["can't read qs-calendar-events output"];
                }
            }
        }
        stderr: StdioCollector {
            id: helperErr
        }
        // Failed to start (not installed) or crashed without printing anything.
        onRunningChanged: if (!running)
            Qt.callLater(() => {
                if (!helper.output)
                    root.errors = [helperErr.text.trim() ? "qs-calendar-events: " + helperErr.text.trim().split("\n").pop() : "qs-calendar-events didn't run (not installed?)"];
            })
    }

    Timer {
        interval: 15 * 60 * 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }
}
