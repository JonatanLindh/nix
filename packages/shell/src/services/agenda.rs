//! Read-only calendar events for the calendar panel (Agenda.qml): the iCal feeds in
//! ~/.config/quickshell-calendar/urls, cached, with recurring events expanded, in local time.
//! The shell used to get them from `qs-calendar-events` (calendar/ in ~/nix); [`feeds`] and
//! [`ical`] are its port.
//!
//! Refreshed every 15 minutes, and when the calendar opens on data older than a minute.

mod feeds;
mod ical;

use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};
use gpui::{AppContext, Context, Entity};

const EVERY: Duration = Duration::from_secs(15 * 60);
/// Opening the panel catches up, but not on every quick reopen.
const STALE: TimeDelta = TimeDelta::seconds(60);

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Local wall-clock times. All-day events start and end at midnight; the end is exclusive.
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
    pub title: String,
    pub location: String,
    pub calendar: String,
}

impl Event {
    /// Still on (or yet to come) at `now`. An event without length lasts until it starts.
    fn pending(&self, now: NaiveDateTime) -> bool {
        self.end > now || (self.end <= self.start && self.start >= now)
    }
}

/// What the feeds say.
#[derive(Debug, Default, PartialEq)]
pub struct Events {
    /// Sorted by start.
    pub events: Vec<Event>,
    pub errors: Vec<String>,
    /// There's a list of feeds to read.
    pub configured: bool,
    /// Every day something happens on.
    pub busy: HashSet<NaiveDate>,
}

impl Events {
    /// `events` sorted by start (then title), with the days they make busy.
    pub fn new(mut events: Vec<Event>, errors: Vec<String>, configured: bool) -> Self {
        events.sort_by(|a, b| (a.start, &a.title).cmp(&(b.start, &b.title)));
        let mut busy = HashSet::new();
        for e in &events {
            // The end is exclusive; an event without length still marks its day.
            let last = if e.end > e.start {
                e.end - TimeDelta::milliseconds(1)
            } else {
                e.start
            };
            busy.extend(e.start.date().iter_days().take_while(|d| *d <= last.date()));
        }
        Self {
            events,
            errors,
            configured,
            busy,
        }
    }

    /// Events overlapping `day`, in start order.
    pub fn on(&self, day: NaiveDate) -> Vec<&Event> {
        let from = day.and_time(NaiveTime::MIN);
        let to = from + TimeDelta::days(1);
        self.events
            .iter()
            .filter(|e| e.start < to && (e.end > from || (e.end <= e.start && e.start >= from)))
            .collect()
    }

    /// The next `n` events that haven't ended at `now`, leaving out `except`.
    pub fn upcoming(&self, now: NaiveDateTime, n: usize, except: &[&Event]) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.pending(now) && !except.iter().any(|x| std::ptr::eq(*x, *e)))
            .take(n)
            .collect()
    }

    /// The calendars' names, in a stable order (for their colours).
    pub fn calendars(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.events.iter().map(|e| e.calendar.as_str()).collect();
        names.sort();
        names.dedup();
        names
    }
}

/// Reads the feeds over the window the panel can show: from the start of last month, a bit over
/// a year ahead. Off the UI thread (network and files). `Err` is a message for the panel.
async fn fetch() -> Result<Events, String> {
    let today = Local::now().date_naive();
    let from = today
        .checked_sub_months(chrono::Months::new(1))
        .and_then(|d| d.with_day0(0))
        .unwrap_or(today)
        .and_time(NaiveTime::MIN);
    let to = from + TimeDelta::days(430);
    let read = tokio::task::spawn_blocking(move || feeds::read(from, to))
        .await
        .map_err(|_| "couldn't read the calendars".to_string())??;
    Ok(Events::new(read.events, read.errors, read.configured))
}

/// The events on the UI side, refreshed in the background.
pub struct Agenda {
    pub events: Events,
    /// Read at least once.
    pub loaded: bool,
    /// When it was last read.
    pub updated: Option<DateTime<Local>>,
    running: bool,
}

impl Agenda {
    pub fn new(cx: &mut gpui::App) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            cx.spawn(async move |this, cx| {
                loop {
                    if this.update(cx, |agenda, cx| agenda.refresh(cx)).is_err() {
                        break;
                    }
                    cx.background_executor().timer(EVERY).await;
                }
            })
            .detach();
            Self {
                events: Events {
                    // Until the feeds say otherwise, so the panel says "Loading…".
                    configured: true,
                    ..Events::default()
                },
                loaded: false,
                updated: None,
                running: false,
            }
        })
    }

    /// Reads the calendars again, unless that's already under way.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.running = true;
        crate::runtime::then(cx, fetch(), |agenda, result, cx| {
            agenda.running = false;
            match result {
                Ok(events) => {
                    agenda.events = events;
                    agenda.loaded = true;
                    agenda.updated = Some(Local::now());
                }
                // Keeps what it had, with the problem on top.
                Err(e) => agenda.events.errors = vec![e],
            }
            cx.notify();
        });
    }

    /// The calendar opened: catch up if what's here is more than a minute old.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        let stale = self.updated.is_none_or(|at| Local::now() - at > STALE);
        if !self.loaded || stale {
            self.refresh(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn sorted_with_calendars() {
        let mut standup = event("2026-11-05 10:15", "2026-11-05 12:00", false);
        standup.calendar = "Work".into();
        let mut trip = event("2026-11-05", "2026-11-07", true);
        trip.calendar = "Home".into();
        let events = Events::new(
            vec![standup, trip],
            vec!["calendar 2: timed out".into()],
            true,
        );
        assert!(events.configured);
        assert_eq!(events.errors, ["calendar 2: timed out"]);
        // All-day events sort from their midnight.
        assert!(events.events[0].all_day);
        assert_eq!(events.events[1].start, at("2026-11-05 10:15"));
        assert_eq!(events.calendars(), ["Home", "Work"]);
    }

    fn event(start: &str, end: &str, all_day: bool) -> Event {
        let parse = |s: &str| {
            if all_day {
                day(s).and_time(NaiveTime::MIN)
            } else {
                at(s)
            }
        };
        Event {
            start: parse(start),
            end: parse(end),
            all_day,
            title: format!("{start}"),
            location: String::new(),
            calendar: String::new(),
        }
    }

    fn with(events: Vec<Event>) -> Events {
        let made = Events::new(events.clone(), vec![], true);
        assert_eq!(made.events, events);
        made
    }

    #[test]
    fn busy_days_and_events_on_a_day() {
        let events = with(vec![
            event("2026-11-05", "2026-11-07", true),
            event("2026-11-06 23:00", "2026-11-07 01:00", false),
            event("2026-11-09 09:00", "2026-11-09 09:00", false),
            event("2026-11-10 08:00", "2026-11-10 09:00", false),
        ]);
        let busy: Vec<_> = {
            let mut b: Vec<_> = events.busy.iter().map(|d| d.to_string()).collect();
            b.sort();
            b
        };
        // The all-day end is exclusive; the overnight event marks both days; one without length
        // still marks its own.
        assert_eq!(
            busy,
            [
                "2026-11-05",
                "2026-11-06",
                "2026-11-07",
                "2026-11-09",
                "2026-11-10"
            ]
        );
        assert_eq!(events.on(day("2026-11-06")).len(), 2);
        assert_eq!(events.on(day("2026-11-07")).len(), 1);
        assert_eq!(events.on(day("2026-11-08")).len(), 0);
        assert_eq!(events.on(day("2026-11-09")).len(), 1);
    }

    #[test]
    fn upcoming_skips_ended_and_shown() {
        let events = with(vec![
            event("2026-11-05 08:00", "2026-11-05 09:00", false),
            event("2026-11-05 11:00", "2026-11-05 12:00", false),
            event("2026-11-05 13:00", "2026-11-05 13:00", false),
            event("2026-11-06 10:00", "2026-11-06 11:00", false),
            event("2026-11-07 10:00", "2026-11-07 11:00", false),
        ]);
        let now = at("2026-11-05 10:00");
        let shown = events.on(day("2026-11-06"));
        let next: Vec<_> = events
            .upcoming(now, 2, &shown)
            .into_iter()
            .map(|e| e.start)
            .collect();
        assert_eq!(next, [at("2026-11-05 11:00"), at("2026-11-05 13:00")]);
        let all = events.upcoming(now, 5, &shown);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].start, at("2026-11-07 10:00"));
    }
}
