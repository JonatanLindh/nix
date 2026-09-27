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
    readonly property int cellWidth: Math.floor((panelWidth - 2 * Theme.panelPadding) / 8)

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

    function dayLabel(d) {
        const diff = Math.round((Agenda.dayStart(d) - today) / 86400000);
        return diff === 0 ? "today" : diff === 1 ? "tomorrow" : diff === -1 ? "yesterday" : Qt.formatDate(d, "ddd dd MMM").toLowerCase();
    }

    // "10:15-12:00" within `day`; ".." where the event runs past the day's edge.
    function span(e, day) {
        if (e.allDay)
            return "all day";
        const from = Agenda.dayStart(day), to = Agenda.addDays(from, 1);
        const s = e.start >= from ? Qt.formatTime(e.start, "HH:mm") : "..";
        if (e.end <= e.start)
            return s;
        return s + "-" + (e.end <= to ? Qt.formatTime(e.end, "HH:mm") : "..");
    }

    name: "cal"
    title: "cal"
    centered: true

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

    TextMetrics {
        id: digit
        font.family: Theme.font
        font.pixelSize: Theme.fontSize
        text: "0"
    }

    actions: [
        Action {
            text: "<"
            onClicked: root.shift(-1)
        },
        MouseArea {
            // Fixed width so the arrows stay put from "may" to "september".
            implicitWidth: Math.ceil(digit.advanceWidth * 14)
            implicitHeight: monthLabel.implicitHeight
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.goToday()

            BarText {
                id: monthLabel
                anchors.centerIn: parent
                text: Qt.formatDate(new Date(root.viewYear, root.viewMonth, 1), "MMMM yyyy").toLowerCase()
                color: parent.containsMouse ? Theme.bright : Theme.text
            }
        },
        Action {
            text: ">"
            onClicked: root.shift(1)
        }
    ]

    Note {
        visible: Agenda.errors.length > 0
        Layout.topMargin: 12
        text: Agenda.errors.join("\n")
        color: Theme.warn
    }

    Note {
        visible: Agenda.loaded && !Agenda.configured
        Layout.topMargin: 12
        text: "no calendars yet. put google calendar \"secret address in ical format\" urls, one per line, in ~/.config/quickshell-calendar/urls"
    }

    Item {
        Layout.leftMargin: Theme.panelPadding
        Layout.topMargin: 12
        implicitWidth: grid.implicitWidth
        implicitHeight: grid.implicitHeight

        Grid {
            id: grid
            columns: 8

            Repeater {
                model: ["wk", "mo", "tu", "we", "th", "fr", "sa", "su"]

                BarText {
                    required property string modelData
                    width: root.cellWidth
                    height: 26
                    horizontalAlignment: Text.AlignHCenter
                    text: modelData
                    color: modelData === "wk" ? Theme.off : Theme.label
                }
            }

            Repeater {
                model: 6 * 8

                MouseArea {
                    id: cell

                    required property int index
                    readonly property bool isWeek: index % 8 === 0
                    readonly property var day: Agenda.addDays(root.gridStart, Math.floor(index / 8) * 7 + Math.max(index % 8 - 1, 0))
                    readonly property bool inMonth: day.getMonth() === root.viewMonth
                    readonly property bool isToday: root.sameDay(day, root.today)
                    readonly property bool isSelected: !isWeek && root.sameDay(day, root.selected)
                    readonly property bool isBusy: !isWeek && Agenda.busy[Agenda.key(day)] === true

                    width: root.cellWidth
                    height: 26
                    enabled: !isWeek
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.selected = day;
                        if (!inMonth)
                            root.shift(day < new Date(root.viewYear, root.viewMonth, 1) ? -1 : 1);
                    }

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 1
                        color: cell.isSelected ? Theme.line : cell.containsMouse ? Theme.hover : "transparent"
                    }

                    BarText {
                        id: number
                        anchors.centerIn: parent
                        text: cell.isWeek ? root.isoWeek(cell.day) : cell.day.getDate()
                        // Days with events are bright and bold; plain days stay regular.
                        color: cell.isWeek ? Theme.off : cell.isToday ? Theme.accent : !cell.inMonth ? Theme.off : cell.isBusy ? Theme.bright : Theme.text
                        font.bold: cell.isBusy
                    }

                    // Today: underlined like the active workspace.
                    Rectangle {
                        visible: cell.isToday && !cell.isWeek
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 4
                        width: number.implicitWidth
                        height: 1
                        color: Theme.accent
                    }
                }
            }
        }

        // Wheel over the grid flips months; clicks pass through to the days.
        MouseArea {
            property real acc: 0

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

    Section {
        text: root.dayLabel(root.selected)
    }

    Repeater {
        model: root.dayEvents

        EventRow {
            day: root.selected
        }
    }

    Note {
        visible: root.dayEvents.length === 0 && Agenda.configured && (Agenda.loaded || Agenda.errors.length === 0)
        text: Agenda.loaded ? "nothing" : "loading"
    }

    Section {
        visible: root.nextEvents.length > 0
        text: "upcoming"
    }

    Repeater {
        model: root.nextEvents

        EventRow {
            showDate: true
        }
    }

    footer: [
        BarText {
            visible: Agenda.loaded && Agenda.configured
            text: "updated " + Qt.formatTime(Agenda.updated, "HH:mm")
            color: Theme.off
        },
        Item {
            Layout.fillWidth: true
        },
        Action {
            visible: !root.onToday
            text: "today"
            onClicked: root.goToday()
        },
        Action {
            text: "google calendar →"
            onClicked: {
                Quickshell.execDetached(["xdg-open", "https://calendar.google.com/"]);
                root.close();
            }
        }
    ]

    // "10:15-12:00  title            calendar", optionally led by "sun 12".
    component EventRow: RowLayout {
        id: row

        required property var modelData
        property var day: modelData.start
        property bool showDate: false

        Layout.fillWidth: true
        Layout.leftMargin: Theme.panelPadding
        Layout.rightMargin: Theme.panelPadding
        Layout.preferredHeight: 24
        spacing: 12

        BarText {
            visible: row.showDate
            Layout.preferredWidth: Math.ceil(digit.advanceWidth * 6)
            text: root.sameDay(row.modelData.start, root.today) ? "today" : Qt.formatDate(row.modelData.start, "ddd dd").toLowerCase()
            color: Theme.label
        }

        BarText {
            Layout.preferredWidth: Math.ceil(digit.advanceWidth * 11)
            text: root.span(row.modelData, row.day)
            color: row.modelData.allDay ? Theme.label : Theme.text
        }

        BarText {
            Layout.fillWidth: true
            text: row.modelData.title
            elide: Text.ElideRight
            color: Theme.bright
        }

        BarText {
            visible: Agenda.multiple
            Layout.maximumWidth: Math.ceil(digit.advanceWidth * 10)
            text: row.modelData.calendar
            elide: Text.ElideRight
            color: Theme.off
        }
    }
}
