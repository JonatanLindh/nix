pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell

// Month grid with ISO week numbers and the selected day's events, opened from the clock.
Panel {
    id: root

    // Month on show, and the day whose events are listed.
    property int viewYear: today.getFullYear()
    property int viewMonth: today.getMonth()
    property var selected: today

    readonly property var today: Agenda.dayStart(clock.date)
    readonly property bool onToday: viewYear === today.getFullYear() && viewMonth === today.getMonth() && sameDay(selected, today)
    // Monday on or before the 1st; the grid always shows six weeks so its height never changes.
    readonly property var gridStart: {
        const first = new Date(viewYear, viewMonth, 1);
        return Agenda.addDays(first, -((first.getDay() + 6) % 7));
    }
    readonly property var dayEvents: Agenda.on(selected)
    readonly property var nextEvents: Agenda.upcoming(5, dayEvents)
    // Calendars in a stable order, each with its own dot colour.
    readonly property var calendars: [...new Set(Agenda.events.map(e => e.calendar))].sort()
    readonly property var palette: ["#7ec3e6", "#b4cd8a", "#e8a87c", "#b79cff", "#f2c46d", "#f28fb1"]

    function sameDay(a, b) {
        return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    }

    function shift(months) {
        const d = new Date(viewYear, viewMonth + months, 1);
        viewYear = d.getFullYear();
        viewMonth = d.getMonth();
    }

    function goToday() {
        viewYear = today.getFullYear();
        viewMonth = today.getMonth();
        selected = today;
    }

    // ISO 8601: weeks start on Monday, week 1 holds the year's first Thursday.
    function isoWeek(d) {
        const thursday = Agenda.addDays(d, 3 - (d.getDay() + 6) % 7);
        const jan1 = new Date(thursday.getFullYear(), 0, 1);
        return 1 + Math.floor(Math.round((thursday - jan1) / 86400000) / 7);
    }

    function colorFor(calendar) {
        return palette[Math.max(0, calendars.indexOf(calendar)) % palette.length];
    }

    // "Today · Monday 19 October", "Tuesday 20 October".
    function dayLabel(d) {
        const diff = Math.round((Agenda.dayStart(d) - today) / 86400000);
        const prefix = diff === 0 ? "Today · " : diff === 1 ? "Tomorrow · " : diff === -1 ? "Yesterday · " : "";
        return prefix + Qt.formatDate(d, "dddd d MMMM");
    }

    // "10:15–12:00" within `day`; "…" where the event runs past the day's edge.
    function span(e, day) {
        if (e.allDay)
            return "All day";
        const from = Agenda.dayStart(day), to = Agenda.addDays(from, 1);
        const s = e.start >= from ? Qt.formatTime(e.start, "HH:mm") : "…";
        if (e.end <= e.start)
            return s;
        return s + "–" + (e.end <= to ? Qt.formatTime(e.end, "HH:mm") : "…");
    }

    // "Today 10:15", "Wed 21 13:00", "Fri 23" (all day).
    function upcomingLabel(e) {
        const d = root.sameDay(e.start, root.today) ? "Today" : Qt.formatDate(e.start, "ddd d");
        return e.allDay ? d : d + " " + Qt.formatTime(e.start, "HH:mm");
    }

    name: "cal"
    title: Qt.formatDate(new Date(viewYear, viewMonth, 1), "MMMM yyyy")
    side: "center"
    panelWidth: 380

    onVisibleChanged: {
        if (!visible)
            return;
        goToday();
        // Opening the panel is the moment to catch up, but not on every quick reopen.
        if (!Agenda.loaded || new Date() - Agenda.updated > 60 * 1000)
            Agenda.refresh();
    }

    SystemClock {
        id: clock
        precision: SystemClock.Hours
    }

    actions: [
        PillButton {
            anchors.verticalCenter: parent.verticalCenter
            text: "Today"
            onClicked: root.goToday()
        },
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4

            CircleButton {
                icon: "chevron-left"
                onClicked: root.shift(-1)
            }
            CircleButton {
                icon: "chevron-right"
                onClicked: root.shift(1)
            }
        }
    ]

    UiText {
        visible: Agenda.errors.length > 0
        Layout.fillWidth: true
        Layout.leftMargin: 8
        Layout.rightMargin: 8
        text: Agenda.errors.join("\n")
        color: Theme.warnText
        font.pixelSize: Theme.small
        wrapMode: Text.Wrap
    }

    UiText {
        visible: Agenda.loaded && !Agenda.configured
        Layout.fillWidth: true
        Layout.leftMargin: 8
        Layout.rightMargin: 8
        text: "No calendars yet. Put Google Calendar \"secret address in iCal format\" URLs, one per line, in ~/.config/quickshell-calendar/urls"
        dim: true
        font.pixelSize: Theme.small
        wrapMode: Text.Wrap
    }

    Tile {
        id: monthTile

        readonly property real dayWidth: (width - 2 * 8 - 26) / 7

        padding: 8
        spacing: 2

        // Weekday names over the day columns; the first column is the ISO week.
        Row {
            Layout.bottomMargin: 4

            Item {
                width: 26
                height: 1
            }
            Repeater {
                model: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]

                UiText {
                    required property string modelData
                    width: monthTile.dayWidth
                    horizontalAlignment: Text.AlignHCenter
                    text: modelData
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
                    font.pixelSize: Theme.tiny
                }
            }
        }

        Repeater {
            model: 6

            Row {
                id: week

                required property int index
                readonly property var monday: Agenda.addDays(root.gridStart, index * 7)

                UiText {
                    width: 26
                    height: 34
                    horizontalAlignment: Text.AlignHCenter
                    text: root.isoWeek(week.monday)
                    mono: true
                    color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.42)
                    font.pixelSize: 10
                }

                Repeater {
                    model: 7

                    MouseArea {
                        id: cell

                        required property int index
                        readonly property var day: Agenda.addDays(week.monday, index)
                        readonly property bool inMonth: day.getMonth() === root.viewMonth
                        readonly property bool isToday: root.sameDay(day, root.today)
                        readonly property bool isSelected: root.sameDay(day, root.selected)
                        readonly property bool isBusy: Agenda.busy[Agenda.key(day)] === true

                        width: monthTile.dayWidth
                        height: 34
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.selected = day;
                            if (!inMonth)
                                root.shift(day < new Date(root.viewYear, root.viewMonth, 1) ? -1 : 1);
                        }

                        // Today is a white disc; the selected day gets a ring.
                        Rectangle {
                            anchors.centerIn: parent
                            width: 34
                            height: 34
                            radius: 17
                            color: cell.isToday ? Theme.white : cell.containsMouse ? Theme.fillHover : "transparent"
                            border.width: cell.isSelected && !cell.isToday ? 1.5 : 0
                            border.color: Qt.rgba(1, 1, 1, 0.55)
                        }

                        UiText {
                            anchors.centerIn: parent
                            text: cell.day.getDate()
                            font.features: {
                                "tnum": 1
                            }
                            color: cell.isToday ? Theme.ink : cell.inMonth ? Theme.text : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.32)
                            // Days with events are bold, with a dot under the number.
                            font.weight: cell.isToday || cell.isBusy ? Font.DemiBold : Font.Normal
                        }

                        Rectangle {
                            visible: cell.isBusy
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: 4
                            width: 4
                            height: 4
                            radius: 2
                            color: cell.isToday ? Theme.ink : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.75)
                        }
                    }
                }
            }
        }

        // Wheel over the grid flips months; clicks pass through to the days.
        MouseArea {
            property real acc: 0

            parent: monthTile
            anchors.fill: parent
            acceptedButtons: Qt.NoButton
            onWheel: wheel => {
                acc += wheel.angleDelta.y;
                if (Math.abs(acc) < 120)
                    return;
                root.shift(acc > 0 ? -1 : 1);
                acc = 0;
            }
        }
    }

    SectionLabel {
        visible: Agenda.configured
        text: root.dayLabel(root.selected)
    }

    ColumnLayout {
        Layout.fillWidth: true
        Layout.topMargin: -6
        spacing: 0

        Repeater {
            model: root.dayEvents

            EventRow {
                when: root.span(modelData, root.selected)
            }
        }

        UiText {
            visible: root.dayEvents.length === 0 && Agenda.configured && (Agenda.loaded || Agenda.errors.length === 0)
            Layout.leftMargin: 10
            Layout.preferredHeight: 34
            text: Agenda.loaded ? "Nothing planned" : "Loading…"
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
        }
    }

    SectionLabel {
        visible: root.nextEvents.length > 0
        text: "Upcoming"
    }

    ColumnLayout {
        visible: root.nextEvents.length > 0
        Layout.fillWidth: true
        Layout.topMargin: -6
        spacing: 0

        Repeater {
            model: root.nextEvents

            EventRow {
                when: root.upcomingLabel(modelData)
            }
        }
    }

    footer: [
        UiText {
            visible: Agenda.loaded && Agenda.configured
            Layout.leftMargin: 6
            text: "Updated " + Qt.formatTime(Agenda.updated, "HH:mm")
            dim: true
            font.pixelSize: Theme.small
        },
        Item {
            Layout.fillWidth: true
        },
        MouseArea {
            implicitWidth: gcal.implicitWidth + 20
            implicitHeight: 30
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                Quickshell.execDetached(["xdg-open", "https://calendar.google.com/"]);
                root.close();
            }

            Rectangle {
                anchors.fill: parent
                radius: Theme.rowRadius
                color: parent.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
            }
            UiText {
                id: gcal
                anchors.centerIn: parent
                text: "Google Calendar…"
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.8)
            }
        }
    ]

    // "●  10:15–12:00   title            calendar"
    component EventRow: RowLayout {
        id: row

        required property var modelData
        property string when: ""

        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 10
        Layout.preferredHeight: 38
        spacing: 12

        Rectangle {
            implicitWidth: 8
            implicitHeight: 8
            radius: 4
            color: root.colorFor(row.modelData.calendar)
        }

        UiText {
            Layout.preferredWidth: 92
            text: row.when
            mono: true
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.75)
            font.pixelSize: Theme.small
        }

        UiText {
            Layout.fillWidth: true
            text: row.modelData.title
        }

        UiText {
            visible: text !== ""
            Layout.maximumWidth: 90
            text: row.modelData.calendar
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.55)
            font.pixelSize: Theme.tiny
        }
    }
}
