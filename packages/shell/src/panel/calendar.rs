//! The calendar (CalendarPanel.qml): a month grid with ISO week numbers, opened from the clock,
//! and under it the selected day's events and the next few after them, from the agenda
//! (`services::agenda`). Six weeks always, so the grid's height never changes between months.

use chrono::{Datelike, Duration, Local, Months, NaiveDate, NaiveDateTime, NaiveTime};
use gpui::{Context, Div, Entity, Rgba, SharedString, Subscription, Window, div, prelude::*, px};

use super::frame;
use crate::services::Services;
use crate::services::agenda::{Agenda, Event};
use crate::theme;
use crate::ui::controls::{circle_button, footer_link, pill, section};
use crate::ui::{Width, bar_button, hstack, label, small, update, vstack};

pub const WIDTH: f32 = 380.;
const WEEK_COLUMN: f32 = 26.;
const CELL: f32 = 34.;

/// How many events "Upcoming" lists.
const UPCOMING: usize = 5;
/// Each calendar's dot, in the order of their names.
const PALETTE: [u32; 6] = [0x7ec3e6, 0xb4cd8a, 0xe8a87c, 0xb79cff, 0xf2c46d, 0xf28fb1];

/// The month on show and the selected day.
pub struct CalendarPanel {
    /// The first of the month on show.
    month: NaiveDate,
    selected: NaiveDate,
    agenda: Entity<Agenda>,
    _observer: Subscription,
}

impl CalendarPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let today = Local::now().date_naive();
        let agenda = Services::get(cx).agenda;
        agenda.update(cx, |agenda, cx| agenda.opened(cx));
        Self {
            month: first_of(today),
            selected: today,
            _observer: cx.observe(&agenda, crate::ui::redraw),
            agenda,
        }
    }

    fn go_today(&mut self, cx: &mut Context<Self>) {
        let today = Local::now().date_naive();
        self.month = first_of(today);
        self.selected = today;
        cx.notify();
    }
}

fn first_of(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

fn shift(month: NaiveDate, by: i32) -> NaiveDate {
    let months = Months::new(by.unsigned_abs());
    if by >= 0 {
        month.checked_add_months(months)
    } else {
        month.checked_sub_months(months)
    }
    .unwrap_or(month)
}

impl CalendarPanel {
    fn select(&mut self, day: NaiveDate, cx: &mut Context<Self>) {
        self.selected = day;
        // A day from the next or previous month brings that month up.
        self.month = first_of(day);
        cx.notify();
    }
}

impl Render for CalendarPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        // The Monday on or before the 1st.
        let start = self.month - Duration::days(self.month.weekday().num_days_from_monday().into());
        let faint = |a| theme::alpha(theme::TEXT, a);

        let names = hstack(0.)
            .pb(px(4.))
            .child(div().w(px(WEEK_COLUMN)))
            .children(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map(|d| {
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_size(px(11.))
                    .text_color(faint(0.55))
                    .child(d)
            }));
        let mut grid = vstack(2.).child(names);
        for week in 0..6 {
            let monday = start + Duration::weeks(week);
            let number = div()
                .w(px(WEEK_COLUMN))
                .flex()
                .justify_center()
                .font_family(theme::MONO)
                .text_size(px(10.))
                .text_color(faint(0.42))
                .child(monday.iso_week().week().to_string());
            let days = (0..7).map(|d| {
                let day = monday + Duration::days(d);
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .child(self.cell(day, today, cx))
            });
            grid = grid.child(hstack(0.).child(number).children(days));
        }

        frame(self.month.format("%B %Y").to_string())
            .action(pill("today", "Today").on_click(update(cx, Self::go_today)))
            .action(
                hstack(4.)
                    .child(
                        circle_button("prev-month", "chevron-left", theme::TEXT).on_click(update(
                            cx,
                            |this, cx| {
                                this.month = shift(this.month, -1);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        circle_button("next-month", "chevron-right", theme::TEXT).on_click(update(
                            cx,
                            |this, cx| {
                                this.month = shift(this.month, 1);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .footer(div().children(self.updated(cx)))
            .footer(div().flex_1())
            .footer(
                footer_link("google-calendar", "Google Calendar…").on_click(|window, _| {
                    crate::runtime::detach(crate::services::spawn(
                        "xdg-open",
                        &["https://calendar.google.com/"],
                    ));
                    window.remove_window();
                }),
            )
            .body(self.body(grid, today, cx))
    }
}

impl CalendarPanel {
    /// A day: today a white disc, the selected day a ring, other months' days faint.
    fn cell(&self, day: NaiveDate, today: NaiveDate, cx: &mut Context<Self>) -> impl IntoElement {
        let is_today = day == today;
        let color = if is_today {
            theme::INK
        } else if day.month() == self.month.month() {
            theme::TEXT
        } else {
            theme::alpha(theme::TEXT, 0.32)
        };
        // Days with events are bold, with a dot under the number.
        let busy = self.agenda.read(cx).events.busy.contains(&day);
        let number = label(day.day().to_string(), color)
            .when(is_today || busy, |l| l.font_weight(theme::SEMIBOLD));
        let dot = busy.then(|| {
            div()
                .absolute()
                .bottom(px(4.))
                .left(px((CELL - 4.) / 2.))
                .size(px(4.))
                .rounded(px(2.))
                .bg(if is_today {
                    theme::INK
                } else {
                    theme::alpha(theme::TEXT, 0.75)
                })
        });
        let selected = day == self.selected && !is_today;
        let button = bar_button(SharedString::from(day.to_string()), number)
            .width(Width::Px(CELL))
            .height(CELL)
            .radius(CELL / 2.)
            .padding(0.)
            .centered()
            .rest(if is_today {
                theme::WHITE
            } else {
                theme::TRANSPARENT
            })
            .outline(selected.then(|| theme::alpha(theme::WHITE, 0.55)))
            .on_click(update(cx, move |this, cx| this.select(day, cx)));
        div().relative().size(px(CELL)).child(button).children(dot)
    }

    /// Problems and hints, the month grid, the selected day's events and what's next.
    fn body(&self, grid: Div, today: NaiveDate, cx: &Context<Self>) -> Div {
        let agenda = self.agenda.read(cx);
        let events = &agenda.events;
        let note = |text: String, color| small(text, color).w_full().px(px(8.));
        let calendars = events.calendars();
        let row = |e: &Event, when: String| event_row(e, when, &calendars);

        let mut body = vstack(10.).w_full();
        if !events.errors.is_empty() {
            body = body.child(note(events.errors.join("\n"), theme::WARN_TEXT));
        }
        if agenda.loaded && !events.configured {
            body = body.child(note(
                "No calendars yet. Put Google Calendar \"secret address in iCal format\" URLs, one \
                 per line, in ~/.config/quickshell-calendar/urls"
                    .into(),
                theme::TEXT_DIM,
            ));
        }
        body = body.child(
            div()
                .w_full()
                .p(px(8.))
                .rounded(px(16.))
                .bg(theme::FILL)
                .child(grid),
        );

        let on_day = events.on(self.selected);
        if events.configured {
            let mut list = vstack(0.)
                .w_full()
                .mt(px(-6.))
                .children(on_day.iter().map(|e| row(e, span(e, self.selected))));
            if on_day.is_empty() && (agenda.loaded || events.errors.is_empty()) {
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(34.))
                        .pl(px(10.))
                        .text_color(theme::alpha(theme::TEXT, 0.55))
                        .child(if agenda.loaded {
                            "Nothing planned"
                        } else {
                            "Loading…"
                        }),
                );
            }
            body = body
                .child(section(day_label(self.selected, today)))
                .child(list);
        }

        let now = Local::now().naive_local();
        let next = events.upcoming(now, UPCOMING, &on_day);
        if !next.is_empty() {
            body = body.child(section("Upcoming")).child(
                vstack(0.)
                    .w_full()
                    .mt(px(-6.))
                    .children(next.iter().map(|e| row(e, upcoming_label(e, today)))),
            );
        }
        body
    }

    /// "Updated 14:05" in the footer, once there's something to be up to date with.
    fn updated(&self, cx: &Context<Self>) -> Option<Div> {
        let agenda = self.agenda.read(cx);
        let at = agenda
            .updated
            .filter(|_| agenda.loaded && agenda.events.configured)?;
        Some(small(format!("Updated {}", at.format("%H:%M")), theme::TEXT_DIM).pl(px(6.)))
    }
}

/// "●  10:15–12:00   title            calendar"
fn event_row(e: &Event, when: String, calendars: &[&str]) -> Div {
    let index = calendars.iter().position(|c| *c == e.calendar).unwrap_or(0);
    let color: Rgba = gpui::rgb(PALETTE[index % PALETTE.len()]);
    hstack(12.)
        .w_full()
        .h(px(38.))
        .px(px(10.))
        .child(div().flex_none().size(px(8.)).rounded(px(4.)).bg(color))
        .child(
            div()
                .flex_none()
                .w(px(92.))
                .font_family(theme::MONO)
                .text_size(px(theme::SMALL))
                .text_color(theme::alpha(theme::TEXT, 0.75))
                .child(when),
        )
        .child(
            label(e.title.clone(), theme::TEXT)
                .flex_1()
                .min_w_0()
                .truncate(),
        )
        .when(!e.calendar.is_empty(), |row| {
            row.child(
                div()
                    .flex_none()
                    .max_w(px(90.))
                    .truncate()
                    .text_size(px(11.))
                    .text_color(theme::alpha(theme::TEXT, 0.55))
                    .child(e.calendar.clone()),
            )
        })
}

/// "Today · Monday 19 October", "Tuesday 20 October".
fn day_label(day: NaiveDate, today: NaiveDate) -> String {
    let prefix = match (day - today).num_days() {
        0 => "Today · ",
        1 => "Tomorrow · ",
        -1 => "Yesterday · ",
        _ => "",
    };
    format!("{prefix}{}", day.format("%A %-d %B"))
}

/// "10:15–12:00" within `day`; "…" where the event runs past the day's edge.
fn span(e: &Event, day: NaiveDate) -> String {
    if e.all_day {
        return "All day".into();
    }
    let from = day.and_time(NaiveTime::MIN);
    let to = from + Duration::days(1);
    let time = |t: NaiveDateTime| t.format("%H:%M").to_string();
    let start = if e.start >= from {
        time(e.start)
    } else {
        "…".into()
    };
    if e.end <= e.start {
        return start;
    }
    let end = if e.end <= to {
        time(e.end)
    } else {
        "…".into()
    };
    format!("{start}–{end}")
}

/// "Today 10:15", "Wed 21 13:00", "Fri 23" (all day).
fn upcoming_label(e: &Event, today: NaiveDate) -> String {
    let day = if e.start.date() == today {
        "Today".to_string()
    } else {
        e.start.format("%a %-d").to_string()
    };
    if e.all_day {
        day
    } else {
        format!("{day} {}", e.start.format("%H:%M"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn event(start: &str, end: &str, all_day: bool) -> Event {
        Event {
            start: at(start),
            end: at(end),
            all_day,
            title: String::new(),
            location: String::new(),
            calendar: String::new(),
        }
    }

    #[test]
    fn spans_within_a_day() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        assert_eq!(
            span(&event("2026-11-05 10:15", "2026-11-05 12:00", false), day),
            "10:15–12:00"
        );
        assert_eq!(
            span(&event("2026-11-04 22:00", "2026-11-05 01:00", false), day),
            "…–01:00"
        );
        assert_eq!(
            span(&event("2026-11-05 23:00", "2026-11-06 01:00", false), day),
            "23:00–…"
        );
        assert_eq!(
            span(&event("2026-11-05 09:00", "2026-11-05 09:00", false), day),
            "09:00"
        );
        assert_eq!(
            span(&event("2026-11-05 00:00", "2026-11-06 00:00", true), day),
            "All day"
        );
    }

    #[test]
    fn labels() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        assert_eq!(day_label(today, today), "Today · Thursday 5 November");
        assert_eq!(
            day_label(today.succ_opt().unwrap(), today),
            "Tomorrow · Friday 6 November"
        );
        assert_eq!(
            day_label(today + Duration::days(3), today),
            "Sunday 8 November"
        );
        assert_eq!(
            upcoming_label(&event("2026-11-05 10:15", "2026-11-05 11:00", false), today),
            "Today 10:15"
        );
        assert_eq!(
            upcoming_label(&event("2026-11-11 13:00", "2026-11-11 14:00", false), today),
            "Wed 11 13:00"
        );
        assert_eq!(
            upcoming_label(&event("2026-11-13 00:00", "2026-11-14 00:00", true), today),
            "Fri 13"
        );
    }
}
