//! Just enough of iCalendar (RFC 5545) to list a feed's events over a window, with recurring
//! ones expanded: what `qs-calendar-events` got from Python's icalendar, x-wr-timezone and
//! recurring-ical-events packages.
//!
//! - Times with a TZID are in that (IANA) zone; UTC and floating times are in the calendar's
//!   X-WR-TIMEZONE if it has one (as Google's feeds do), and floating ones are local otherwise.
//! - A recurring event repeats by its RRULEs (through the `rrule` crate) and RDATEs, less its
//!   EXDATEs. Each repeat lasts as long as the first.
//! - An event with a RECURRENCE-ID replaces that repeat of the event with its UID (a moved or
//!   edited meeting); of duplicates, the highest SEQUENCE wins.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone};
use rrule::{RRule, RRuleSet, Tz, Unvalidated};

use super::Event;

/// A parsed feed.
#[derive(Debug)]
pub struct Calendar {
    /// X-WR-CALNAME.
    pub name: Option<String>,
    events: Vec<VEvent>,
}

/// One content line: `NAME;PARAM=VALUE:VALUE`.
#[derive(Debug)]
struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, name: &str) -> Option<&str> {
        (self.params.iter())
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Lines with their continuations (lines starting with a space or tab) joined back on.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        match (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(line.to_string()),
        }
    }
    lines
}

fn parse_prop(line: &str) -> Option<Prop> {
    let name_end = line.find([';', ':'])?;
    let name = line[..name_end].to_ascii_uppercase();
    let mut rest = &line[name_end..];
    let mut params = Vec::new();
    while let Some(after) = rest.strip_prefix(';') {
        let (pname, after) = after.split_once('=')?;
        let (value, after) = if let Some(quoted) = after.strip_prefix('"') {
            let (value, after) = quoted.split_once('"')?;
            (value, after)
        } else {
            let end = after.find([';', ':'])?;
            (&after[..end], &after[end..])
        };
        params.push((pname.to_ascii_uppercase(), value.to_string()));
        rest = after;
    }
    let value = rest.strip_prefix(':')?;
    Some(Prop {
        name,
        params,
        value: value.to_string(),
    })
}

/// A TEXT value without its escapes.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Parses a feed. Only an iCalendar object at all is required: events it can't make sense of are
/// left out.
pub fn parse(text: &str) -> Result<Calendar, String> {
    let mut stack: Vec<String> = Vec::new();
    let mut seen_calendar = false;
    let mut calendar_props = Vec::new();
    let mut raw_events: Vec<Vec<Prop>> = Vec::new();
    let mut current: Option<Vec<Prop>> = None;
    for line in unfold(text) {
        if line.trim().is_empty() {
            continue;
        }
        let Some(prop) = parse_prop(&line) else {
            continue;
        };
        match prop.name.as_str() {
            "BEGIN" => {
                let what = prop.value.trim().to_ascii_uppercase();
                if what == "VCALENDAR" && stack.is_empty() {
                    seen_calendar = true;
                }
                if what == "VEVENT" && stack == ["VCALENDAR"] {
                    current = Some(Vec::new());
                }
                stack.push(what);
            }
            "END" => {
                let what = prop.value.trim().to_ascii_uppercase();
                if what == "VEVENT"
                    && stack == ["VCALENDAR", "VEVENT"]
                    && let Some(event) = current.take()
                {
                    raw_events.push(event);
                }
                if let Some(at) = stack.iter().rposition(|s| *s == what) {
                    stack.truncate(at);
                }
            }
            _ => match stack.len() {
                1 if stack[0] == "VCALENDAR" => calendar_props.push(prop),
                2 if stack[1] == "VEVENT" => {
                    if let Some(event) = current.as_mut() {
                        event.push(prop);
                    }
                }
                _ => {}
            },
        }
    }
    if !seen_calendar {
        return Err("no VCALENDAR".into());
    }
    let calendar_prop = |name: &str| {
        (calendar_props.iter())
            .find(|p| p.name == name)
            .map(|p| unescape(&p.value).trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let zones = Zones::new(calendar_prop("X-WR-TIMEZONE").as_deref());
    Ok(Calendar {
        name: calendar_prop("X-WR-CALNAME"),
        events: (raw_events.iter())
            .filter_map(|props| VEvent::new(props, &zones))
            .collect(),
    })
}

// ---- Times

/// A DATE or a DATE-TIME in a zone (floating ones are given one when read).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Time {
    Date(NaiveDate),
    At(DateTime<Tz>),
}

impl Time {
    /// Local wall-clock time; a date is its midnight.
    fn local(self) -> NaiveDateTime {
        match self {
            Time::Date(d) => d.and_time(NaiveTime::MIN),
            Time::At(t) => t.with_timezone(&Local).naive_local(),
        }
    }

    /// The date, in the time's own zone.
    fn date(self) -> NaiveDate {
        match self {
            Time::Date(d) => d,
            Time::At(t) => t.date_naive(),
        }
    }

    /// What identifies a repeat (RECURRENCE-ID, EXDATE): the UTC and the wall-clock time, as
    /// recurring-ical-events compares them.
    fn keys(self) -> [NaiveDateTime; 2] {
        match self {
            Time::Date(d) => [d.and_time(NaiveTime::MIN); 2],
            Time::At(t) => [t.naive_utc(), t.naive_local()],
        }
    }

    fn same_repeat(self, other: Time) -> bool {
        let (a, b) = (self.keys(), other.keys());
        a.iter().any(|k| b.contains(k))
    }
}

/// `naive` in `tz`; a time that a clock change skips is taken an hour later.
fn localise(tz: Tz, naive: NaiveDateTime) -> Option<DateTime<Tz>> {
    (tz.from_local_datetime(&naive).earliest()).or_else(|| {
        tz.from_local_datetime(&(naive + TimeDelta::hours(1)))
            .earliest()
    })
}

/// How the feed's times are read.
struct Zones {
    /// X-WR-TIMEZONE: where UTC and floating times go.
    calendar: Option<Tz>,
}

impl Zones {
    fn new(x_wr_timezone: Option<&str>) -> Self {
        Self {
            calendar: x_wr_timezone.and_then(zone),
        }
    }

    fn floating(&self) -> Tz {
        self.calendar.unwrap_or(Tz::LOCAL)
    }

    /// A DATE or DATE-TIME value, with the TZID and VALUE parameters it came with.
    fn time(&self, value: &str, tzid: Option<&str>, kind: Option<&str>) -> Option<Time> {
        let value = value.trim();
        if kind.is_some_and(|k| k.eq_ignore_ascii_case("DATE")) || value.len() == 8 {
            return NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d")
                .ok()
                .map(Time::Date);
        }
        let (naive, utc) = match value.strip_suffix(['Z', 'z']) {
            Some(v) => (v, true),
            None => (value, false),
        };
        let naive = NaiveDateTime::parse_from_str(naive, "%Y%m%dT%H%M%S").ok()?;
        let tz = if utc {
            Tz::UTC
        } else {
            tzid.and_then(zone).unwrap_or_else(|| self.floating())
        };
        let at = localise(tz, naive)?;
        // x-wr-timezone moves UTC times into the calendar's zone, which matters for repeats
        // across a clock change.
        Some(Time::At(match self.calendar {
            Some(calendar) if tz == Tz::UTC => at.with_timezone(&calendar),
            _ => at,
        }))
    }

    /// A comma-separated list of times (EXDATE, RDATE; periods by their start).
    fn times(&self, prop: &Prop) -> Vec<Time> {
        (prop.value.split(','))
            .filter_map(|v| {
                let start = v.split('/').next().unwrap_or(v);
                let kind = prop
                    .param("VALUE")
                    .filter(|k| !k.eq_ignore_ascii_case("PERIOD"));
                self.time(start, prop.param("TZID"), kind)
            })
            .collect()
    }
}

/// An IANA zone by name, also from the longer TZIDs some programs write
/// ("/mozilla.org/20050126_1/Europe/Stockholm").
fn zone(name: &str) -> Option<Tz> {
    let name = name.trim().trim_matches('"');
    if name.eq_ignore_ascii_case("UTC") || name.eq_ignore_ascii_case("Z") {
        return Some(Tz::UTC);
    }
    std::iter::once(name)
        .chain(name.match_indices('/').map(|(i, _)| &name[i + 1..]))
        .find_map(|n| chrono_tz::Tz::from_str(n).ok())
        .map(Tz::Tz)
}

/// An iCalendar DURATION ("PT1H30M", "P1D", "-P1W").
fn parse_duration(text: &str) -> Option<TimeDelta> {
    let text = text.trim();
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let mut rest = text.strip_prefix(['P', 'p'])?;
    let mut total = TimeDelta::zero();
    let mut in_time = false;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix(['T', 't']) {
            in_time = true;
            rest = after;
            continue;
        }
        let digits = rest.find(|c: char| !c.is_ascii_digit())?;
        let n: i64 = rest[..digits].parse().ok()?;
        let unit = rest[digits..].chars().next()?.to_ascii_uppercase();
        total += match (unit, in_time) {
            ('W', false) => TimeDelta::weeks(n),
            ('D', false) => TimeDelta::days(n),
            ('H', true) => TimeDelta::hours(n),
            ('M', true) => TimeDelta::minutes(n),
            ('S', true) => TimeDelta::seconds(n),
            _ => return None,
        };
        rest = &rest[digits + 1..];
    }
    Some(if negative { -total } else { total })
}

// ---- Events

#[derive(Debug, Clone)]
struct VEvent {
    uid: Option<String>,
    sequence: i64,
    recurrence_id: Option<Time>,
    start: Time,
    end: Time,
    rrules: Vec<String>,
    rdates: Vec<Time>,
    exdates: Vec<Time>,
    summary: String,
    location: String,
    cancelled: bool,
}

impl VEvent {
    fn new(props: &[Prop], zones: &Zones) -> Option<Self> {
        let get = |name: &str| props.iter().find(|p| p.name == name);
        let time = |p: &Prop| zones.time(&p.value, p.param("TZID"), p.param("VALUE"));
        let text = |name: &str| get(name).map(|p| unescape(&p.value)).unwrap_or_default();
        let start = time(get("DTSTART")?)?;
        // As recurring-ical-events reads it: DTEND, else DTSTART + DURATION, else a day for a
        // date and no time at all for a time.
        let end = match (get("DTEND").and_then(time), get("DURATION")) {
            (Some(end), _) => end,
            (None, Some(duration)) => {
                let duration = parse_duration(&duration.value)?;
                match start {
                    Time::Date(d) => Time::Date((d.and_time(NaiveTime::MIN) + duration).date()),
                    Time::At(t) => Time::At(t + duration),
                }
            }
            (None, None) => match start {
                Time::Date(d) => Time::Date(d + TimeDelta::days(1)),
                at => at,
            },
        };
        let all = |name: &str| {
            (props.iter())
                .filter(|p| p.name == name)
                .flat_map(|p| zones.times(p))
                .collect()
        };
        Some(Self {
            uid: get("UID").map(|p| unescape(&p.value)),
            sequence: get("SEQUENCE")
                .and_then(|p| p.value.trim().parse().ok())
                .unwrap_or(0),
            recurrence_id: get("RECURRENCE-ID").and_then(time),
            start,
            end,
            rrules: (props.iter())
                .filter(|p| p.name == "RRULE")
                .map(|p| p.value.trim().to_string())
                .collect(),
            rdates: all("RDATE"),
            exdates: all("EXDATE"),
            summary: text("SUMMARY"),
            location: text("LOCATION"),
            cancelled: text("STATUS").trim().eq_ignore_ascii_case("CANCELLED"),
        })
    }

    /// Lasts from `start` to `end`, for the panel.
    fn event(&self, start: Time, end: Time, calendar: &str) -> Event {
        let all_day = matches!(start, Time::Date(_));
        let end = match (all_day, end) {
            (true, end) => Time::Date(end.date()),
            (false, end) => end,
        };
        let title = self.summary.trim();
        Event {
            start: start.local(),
            end: end.local(),
            all_day,
            title: if title.is_empty() {
                "(no title)".into()
            } else {
                title.into()
            },
            location: self.location.trim().into(),
            calendar: calendar.into(),
        }
    }
}

/// Whether `start`..`end` (local) touches `from`..`to`. Something without length counts where
/// it starts.
fn overlaps(
    start: NaiveDateTime,
    end: NaiveDateTime,
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> bool {
    start < to && (end > from || (end <= start && start >= from))
}

/// The starts of an event's repeats from `after` to `before` (local), RDATEs and its own start
/// included, EXDATEs not yet taken out.
fn repeats(master: &VEvent, after: NaiveDateTime, before: NaiveDateTime) -> Vec<Time> {
    let all_day = matches!(master.start, Time::Date(_));
    let back = |t: DateTime<Tz>| {
        if all_day {
            Time::Date(t.date_naive())
        } else {
            Time::At(t)
        }
    };
    // Dates repeat as UTC midnights, so no clock change moves them.
    let start = match master.start {
        Time::Date(d) => Tz::UTC.from_utc_datetime(&d.and_time(NaiveTime::MIN)),
        Time::At(at) => at,
    };
    let tz = start.timezone();
    let bound = |local: NaiveDateTime| -> Option<DateTime<Tz>> {
        if all_day {
            Some(Tz::UTC.from_utc_datetime(&local))
        } else {
            Some(
                Local
                    .from_local_datetime(&local)
                    .earliest()?
                    .with_timezone(&tz),
            )
        }
    };
    let (Some(after), Some(before)) = (bound(after), bound(before)) else {
        return vec![master.start];
    };

    let mut starts = Vec::new();
    // DTSTART counts, unless every rule ended before it.
    let mut last_until: Option<Option<DateTime<Tz>>> = None;
    for rule in &master.rrules {
        let Some((rule, until)) = rrule(rule, start, all_day) else {
            continue;
        };
        last_until = Some(match (last_until, until) {
            (Some(None), _) | (_, None) => None,
            (Some(Some(a)), Some(b)) => Some(a.max(b)),
            (None, Some(b)) => Some(b),
        });
        if until.is_some_and(|u| u < start) {
            continue;
        }
        let Ok(rule) = rule.validate(start) else {
            continue;
        };
        let set = RRuleSet::new(start).rrule(rule).after(after).before(before);
        starts.extend(set.all(u16::MAX).dates.into_iter().map(back));
    }
    if last_until.flatten().is_none_or(|until| start <= until) {
        starts.push(master.start);
    }
    starts.extend(master.rdates.iter().copied());
    starts
}

/// RRULE `text` for a series starting at `start` (a UTC midnight if `all_day`), and its UNTIL in a form the rrule crate takes
/// (in UTC unless the start is local), as recurring-ical-events reads it: a date UNTIL is that
/// day's midnight, and a floating one is UTC.
fn rrule(
    text: &str,
    start: DateTime<Tz>,
    all_day: bool,
) -> Option<(RRule<Unvalidated>, Option<DateTime<Tz>>)> {
    let text = text.strip_prefix("RRULE:").unwrap_or(text);
    let mut until = None;
    let mut parts = Vec::new();
    for part in text.split(';').filter(|p| !p.is_empty()) {
        match part.split_once('=') {
            Some((key, value)) if key.eq_ignore_ascii_case("UNTIL") => until = Some(value.trim()),
            // A negative COUNT makes no sense; recurring-ical-events drops it (issue 128).
            Some((key, value)) if key.eq_ignore_ascii_case("COUNT") && value.starts_with('-') => {}
            _ => parts.push(part),
        }
    }
    let mut rule = RRule::<Unvalidated>::from_str(&parts.join(";")).ok()?;
    let until = match until {
        None => None,
        Some(value) => {
            let floating = start.timezone().is_local();
            let digits = value.trim_end_matches(['Z', 'z']);
            let naive = match NaiveDateTime::parse_from_str(digits, "%Y%m%dT%H%M%S") {
                Ok(naive) => naive,
                Err(_) => NaiveDate::parse_from_str(digits.get(..8)?, "%Y%m%d")
                    .ok()?
                    .and_time(NaiveTime::MIN),
            };
            Some(if all_day {
                // Repeats are UTC midnights: the until day is in.
                Tz::UTC.from_utc_datetime(&naive.date().and_time(NaiveTime::MIN))
            } else if floating && !value.ends_with(['Z', 'z']) {
                localise(Tz::LOCAL, naive)?
            } else {
                Tz::UTC.from_utc_datetime(&naive)
            })
        }
    };
    if let Some(until) = until {
        rule = rule.until(until);
    }
    Some((rule, until))
}

impl Calendar {
    /// Every event and repeat that touches `from`..`to` (local), not cancelled, named for the
    /// panel as `calendar`. Unsorted.
    pub fn events(&self, from: NaiveDateTime, to: NaiveDateTime, calendar: &str) -> Vec<Event> {
        // Events by UID, in feed order; ones without a UID are on their own.
        let mut series: Vec<Vec<&VEvent>> = Vec::new();
        let mut by_uid: HashMap<&str, usize> = HashMap::new();
        for event in &self.events {
            match event.uid.as_deref() {
                Some(uid) => match by_uid.get(uid) {
                    Some(&i) => series[i].push(event),
                    None => {
                        by_uid.insert(uid, series.len());
                        series.push(vec![event]);
                    }
                },
                None => series.push(vec![event]),
            }
        }
        let mut out = Vec::new();
        for events in series {
            expand(&events, from, to, calendar, &mut out);
        }
        out
    }
}

/// Picks `b` over `a` only with a higher SEQUENCE.
fn newer<'a>(a: Option<&'a VEvent>, b: &'a VEvent) -> &'a VEvent {
    match a {
        Some(a) if a.sequence >= b.sequence => a,
        _ => b,
    }
}

/// The occurrences of one UID's events touching `from`..`to`.
fn expand(
    events: &[&VEvent],
    from: NaiveDateTime,
    to: NaiveDateTime,
    calendar: &str,
    out: &mut Vec<Event>,
) {
    let mut master: Option<&VEvent> = None;
    let mut changed: Vec<&VEvent> = Vec::new();
    for &event in events {
        let Some(id) = event.recurrence_id else {
            master = Some(newer(master, event));
            continue;
        };
        let same = |c: &&VEvent| c.recurrence_id.is_some_and(|c| c.same_repeat(id));
        match changed.iter().position(same) {
            Some(i) => changed[i] = newer(Some(changed[i]), event),
            None => changed.push(event),
        }
    }

    let mut emit = |event: &VEvent, start: Time, end: Time| {
        if event.cancelled {
            return;
        }
        let e = event.event(start, end, calendar);
        if overlaps(e.start, e.end, from, to) {
            out.push(e);
        }
    };

    let Some(master) = master else {
        for c in &changed {
            emit(c, c.start, c.end);
        }
        return;
    };
    let span = match (master.start, master.end) {
        (Time::At(s), Time::At(e)) => e - s,
        (s, e) => e.local() - s.local(),
    };
    let excluded = |t: Time| {
        master.exdates.iter().any(|x| match x {
            // A date takes out every repeat on that day.
            Time::Date(d) => t.date() == *d,
            x => x.same_repeat(t),
        })
    };
    let mut used: HashSet<usize> = HashSet::new();
    let mut seen: Vec<Time> = Vec::new();
    let margin = TimeDelta::days(2);
    let after = from - span.max(TimeDelta::zero()) - margin;
    for start in repeats(master, after, to + margin) {
        if seen.iter().any(|s| s.same_repeat(start)) || excluded(start) {
            continue;
        }
        seen.push(start);
        let replaced =
            (changed.iter()).position(|c| c.recurrence_id.is_some_and(|id| id.same_repeat(start)));
        match replaced {
            Some(i) => {
                if used.insert(i) {
                    emit(changed[i], changed[i].start, changed[i].end);
                }
            }
            None => {
                let end = match start {
                    Time::Date(d) => Time::Date((d.and_time(NaiveTime::MIN) + span).date()),
                    Time::At(at) => Time::At(at + span),
                };
                emit(master, start, end);
            }
        }
    }
    // Moved repeats whose original slot is outside the window may still land inside it.
    for (i, c) in changed.iter().enumerate() {
        let gone = c.recurrence_id.is_some_and(excluded);
        if !used.contains(&i) && !gone {
            emit(c, c.start, c.end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `naive` ("2026-11-05 10:00") in `zone`, as local wall-clock time.
    fn local(zone: chrono_tz::Tz, naive: &str) -> NaiveDateTime {
        let naive = NaiveDateTime::parse_from_str(naive, "%Y-%m-%d %H:%M").unwrap();
        (zone.from_local_datetime(&naive).unwrap())
            .with_timezone(&Local)
            .naive_local()
    }

    fn midnight(date: &str) -> NaiveDateTime {
        NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_time(NaiveTime::MIN)
    }

    fn events_in(text: &str, from: &str, to: &str) -> Vec<Event> {
        let calendar = parse(text).unwrap();
        let name = calendar.name.clone().unwrap_or_default();
        let mut events = calendar.events(midnight(from), midnight(to), &name);
        events.sort_by(|a, b| (a.start, &a.title).cmp(&(b.start, &b.title)));
        events
    }

    const STOCKHOLM: chrono_tz::Tz = chrono_tz::Europe::Stockholm;
    const UTC: chrono_tz::Tz = chrono_tz::UTC;

    /// Shaped like a Google Calendar feed.
    const GOOGLE: &str = "BEGIN:VCALENDAR\r
PRODID:-//Google Inc//Google Calendar 70.9054//EN\r
VERSION:2.0\r
X-WR-CALNAME:Work\r
X-WR-TIMEZONE:Europe/Stockholm\r
BEGIN:VTIMEZONE\r
TZID:Europe/Stockholm\r
BEGIN:STANDARD\r
DTSTART:19701025T030000\r
TZOFFSETFROM:+0200\r
TZOFFSETTO:+0100\r
END:STANDARD\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Stockholm:20261105T113000\r
DTEND;TZID=Europe/Stockholm:20261105T123000\r
UID:lunch@google.com\r
SUMMARY:Lunch\\, with Bob\r
LOCATION:Kafé\r
  Kontoret\\nfloor 2\r
BEGIN:VALARM\r
ACTION:DISPLAY\r
DESCRIPTION:This is an event reminder\r
TRIGGER:-P0DT0H10M0S\r
END:VALARM\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;VALUE=DATE:20261105\r
DTEND;VALUE=DATE:20261107\r
UID:trip@google.com\r
SUMMARY:  Trip  \r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261001T080000Z\r
DTEND:20261001T083000Z\r
RRULE:FREQ=WEEKLY;WKST=MO;UNTIL=20261126T225959Z;BYDAY=TH\r
EXDATE;TZID=Europe/Stockholm:20261015T100000\r
UID:standup@google.com\r
SEQUENCE:0\r
SUMMARY:Standup\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Stockholm:20261022T140000\r
DTEND;TZID=Europe/Stockholm:20261022T143000\r
RECURRENCE-ID;TZID=Europe/Stockholm:20261022T100000\r
UID:standup@google.com\r
SEQUENCE:1\r
SUMMARY:Standup (moved)\r
END:VEVENT\r
BEGIN:VEVENT\r
RECURRENCE-ID:20261105T090000Z\r
DTSTART:20261105T090000Z\r
DTEND:20261105T093000Z\r
UID:standup@google.com\r
STATUS:CANCELLED\r
SUMMARY:Standup\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261110T130000Z\r
DURATION:PT1H30M\r
UID:review@google.com\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Stockholm:20261112T090000\r
DTEND;TZID=Europe/Stockholm:20261112T100000\r
UID:off@google.com\r
STATUS:CANCELLED\r
SUMMARY:Called off\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=Europe/Stockholm:20270301T090000\r
DTEND;TZID=Europe/Stockholm:20270301T100000\r
UID:later@google.com\r
SUMMARY:Outside the window\r
END:VEVENT\r
END:VCALENDAR\r
";

    #[test]
    fn google_feed() {
        let events = events_in(GOOGLE, "2026-10-01", "2026-12-31");
        let shown: Vec<_> = (events.iter())
            .map(|e| (e.start, e.end, e.all_day, e.title.as_str()))
            .collect();
        let standup = |day: &str| {
            (
                local(STOCKHOLM, &format!("{day} 10:00")),
                local(STOCKHOLM, &format!("{day} 10:30")),
                false,
                "Standup",
            )
        };
        assert_eq!(
            shown,
            [
                // UTC times are in the calendar's zone, so the standup stays at 10:00 across
                // the clock change on 25 October.
                standup("2026-10-01"),
                standup("2026-10-08"),
                // 15 October is an EXDATE.
                (
                    local(STOCKHOLM, "2026-10-22 14:00"),
                    local(STOCKHOLM, "2026-10-22 14:30"),
                    false,
                    "Standup (moved)",
                ),
                standup("2026-10-29"),
                (midnight("2026-11-05"), midnight("2026-11-07"), true, "Trip"),
                // 5 November's standup is cancelled.
                (
                    local(STOCKHOLM, "2026-11-05 11:30"),
                    local(STOCKHOLM, "2026-11-05 12:30"),
                    false,
                    "Lunch, with Bob",
                ),
                (
                    local(UTC, "2026-11-10 13:00"),
                    local(UTC, "2026-11-10 14:30"),
                    false,
                    "(no title)",
                ),
                standup("2026-11-12"),
                standup("2026-11-19"),
                // UNTIL 26 Nov 22:59:59 UTC takes in the 26th.
                standup("2026-11-26"),
            ]
        );
        let lunch = &events[5];
        assert_eq!(lunch.location, "Kafé Kontoret\nfloor 2");
        assert_eq!(lunch.calendar, "Work");
        assert_eq!(parse(GOOGLE).unwrap().name.as_deref(), Some("Work"));
    }

    #[test]
    fn window() {
        // Only what touches the window; the moved standup comes along on its own.
        let events = events_in(GOOGLE, "2026-10-20", "2026-10-23");
        let titles: Vec<_> = events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Standup (moved)"]);
        // A repeat moved into the window from outside it still shows.
        let moved_in = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:a
DTSTART:20260105T100000Z
DTEND:20260105T110000Z
RRULE:FREQ=WEEKLY;COUNT=2
SUMMARY:Weekly
END:VEVENT
BEGIN:VEVENT
UID:a
RECURRENCE-ID:20260112T100000Z
DTSTART:20260301T100000Z
DTEND:20260301T110000Z
SUMMARY:Moved far
END:VEVENT
END:VCALENDAR
";
        let events = events_in(moved_in, "2026-02-01", "2026-04-01");
        let titles: Vec<_> = events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Moved far"]);
        assert!(events_in(moved_in, "2026-01-10", "2026-01-14").is_empty());
    }

    #[test]
    fn all_day_repeats() {
        let text = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:bins
DTSTART;VALUE=DATE:20261102
RRULE:FREQ=DAILY;COUNT=4
EXDATE;VALUE=DATE:20261103
SUMMARY:Bins
END:VEVENT
BEGIN:VEVENT
UID:birthday
DTSTART;VALUE=DATE:20200310
DTEND;VALUE=DATE:20200311
RRULE:FREQ=YEARLY;UNTIL=20280310
SUMMARY:Birthday
END:VEVENT
END:VCALENDAR
";
        let events = events_in(text, "2026-01-01", "2027-12-31");
        let shown: Vec<_> = (events.iter())
            .map(|e| (e.start, e.end, e.all_day, e.title.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                (
                    midnight("2026-03-10"),
                    midnight("2026-03-11"),
                    true,
                    "Birthday"
                ),
                // No DTEND: a day long.
                (midnight("2026-11-02"), midnight("2026-11-03"), true, "Bins"),
                (midnight("2026-11-04"), midnight("2026-11-05"), true, "Bins"),
                (midnight("2026-11-05"), midnight("2026-11-06"), true, "Bins"),
                (
                    midnight("2027-03-10"),
                    midnight("2027-03-11"),
                    true,
                    "Birthday"
                ),
            ]
        );
    }

    #[test]
    fn floating_and_odd_until() {
        // No X-WR-TIMEZONE: floating times are local. A date UNTIL on a zoned start is that
        // day's midnight UTC (as recurring-ical-events reads it).
        let text = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:float
DTSTART:20261105T090000
DTEND:20261105T100000
SUMMARY:Floating
END:VEVENT
BEGIN:VEVENT
UID:mozilla
DTSTART;TZID=/mozilla.org/20050126_1/Europe/Stockholm:20261102T090000
DTEND;TZID=/mozilla.org/20050126_1/Europe/Stockholm:20261102T100000
RRULE:FREQ=DAILY;UNTIL=20261104
SUMMARY:Daily
END:VEVENT
END:VCALENDAR
";
        let events = events_in(text, "2026-11-01", "2026-11-10");
        let shown: Vec<_> = (events.iter())
            .map(|e| (e.start, e.title.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                (local(STOCKHOLM, "2026-11-02 09:00"), "Daily"),
                (local(STOCKHOLM, "2026-11-03 09:00"), "Daily"),
                (
                    NaiveDateTime::parse_from_str("2026-11-05 09:00", "%Y-%m-%d %H:%M").unwrap(),
                    "Floating"
                ),
            ]
        );
        assert_eq!(events[0].end - events[0].start, TimeDelta::hours(1));
    }

    #[test]
    fn duplicates_and_rdates() {
        let text = "BEGIN:VCALENDAR
BEGIN:VEVENT
UID:x
SEQUENCE:1
DTSTART:20261105T100000Z
DTEND:20261105T110000Z
RDATE:20261107T100000Z,20261109T100000Z
SUMMARY:Newer
END:VEVENT
BEGIN:VEVENT
UID:x
SEQUENCE:0
DTSTART:20261105T100000Z
DTEND:20261105T110000Z
SUMMARY:Older
END:VEVENT
END:VCALENDAR
";
        let events = events_in(text, "2026-11-01", "2026-11-30");
        let shown: Vec<_> = (events.iter())
            .map(|e| (e.start, e.title.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                (local(UTC, "2026-11-05 10:00"), "Newer"),
                (local(UTC, "2026-11-07 10:00"), "Newer"),
                (local(UTC, "2026-11-09 10:00"), "Newer"),
            ]
        );
    }

    #[test]
    fn content_lines() {
        let lines = unfold("A:one\r\n two\r\n\tthree\r\nB:x\n");
        assert_eq!(lines, ["A:onetwothree", "B:x", ""]);
        let prop = parse_prop(r#"DTSTART;TZID="Europe/Stockholm";VALUE=DATE-TIME:20261105T100000"#)
            .unwrap();
        assert_eq!(prop.name, "DTSTART");
        assert_eq!(prop.param("tzid"), Some("Europe/Stockholm"));
        assert_eq!(prop.param("VALUE"), Some("DATE-TIME"));
        assert_eq!(prop.value, "20261105T100000");
        let colon = parse_prop("DESCRIPTION;ALTREP=\"cid:a;b\":see: here").unwrap();
        assert_eq!(colon.param("ALTREP"), Some("cid:a;b"));
        assert_eq!(colon.value, "see: here");
        assert_eq!(unescape(r"a\, b\; c\\d\ne"), "a, b; c\\d\ne");
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("PT1H30M"), Some(TimeDelta::minutes(90)));
        assert_eq!(parse_duration("P1D"), Some(TimeDelta::days(1)));
        assert_eq!(parse_duration("P2W"), Some(TimeDelta::weeks(2)));
        assert_eq!(
            parse_duration("-P0DT0H10M0S"),
            Some(TimeDelta::minutes(-10))
        );
        assert_eq!(parse_duration("1H"), None);
    }

    #[test]
    fn not_a_calendar() {
        assert!(parse("<html>sign in</html>").is_err());
        assert!(
            parse("BEGIN:VCALENDAR\nEND:VCALENDAR\n")
                .unwrap()
                .events
                .is_empty()
        );
        assert_eq!(zone("Europe/Stockholm"), Some(Tz::Tz(STOCKHOLM)));
        assert_eq!(zone("Not/AZone"), None);
    }
}
