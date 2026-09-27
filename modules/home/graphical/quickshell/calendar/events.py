"""Read-only calendar events for the Quickshell bar.

Fetches the iCal feeds listed in $XDG_CONFIG_HOME/quickshell-calendar/urls (one per
line, e.g. Google Calendar's "secret address in iCal format"), keeps the last good copy
of each in $XDG_CACHE_HOME/quickshell-calendar so it also works offline, expands
recurring events over a window and prints them as JSON in local time.

The URLs are secrets: they are never printed, errors name calendars by position.
"""

import argparse
import datetime as dt
import hashlib
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

import icalendar
import recurring_ical_events
import x_wr_timezone


def xdg(var, fallback):
    return Path(os.environ.get(var) or Path.home() / fallback)


def read_urls(path):
    try:
        lines = path.read_text().splitlines()
    except FileNotFoundError:
        return []
    return [
        line.strip()
        for line in lines
        if line.strip() and not line.strip().startswith("#")
    ]


def describe(exc, url):
    if isinstance(exc, urllib.error.HTTPError):
        msg = f"HTTP {exc.code}"
    elif isinstance(exc, TimeoutError):
        msg = "timed out"
    elif isinstance(exc, urllib.error.URLError):
        return (
            describe(exc.reason, url)
            if isinstance(exc.reason, OSError)
            else str(exc.reason).replace(url, "<url>")
        )
    elif isinstance(exc, OSError) and exc.strerror:
        # strerror only: str(exc) can include the file name.
        msg = exc.strerror.lower()
    else:
        msg = str(exc) or type(exc).__name__
    # Belt and braces: never leak the secret address through an exception message.
    return msg.replace(url, "<url>")


def fetch(url, cache_file):
    """Returns (ics bytes or None, problem or None); falls back to the cached copy."""
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "qs-calendar-events"})
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = resp.read()
        if not data.lstrip().startswith(b"BEGIN:VCALENDAR"):
            raise ValueError("not an iCal feed")
        tmp = cache_file.with_suffix(".tmp")
        tmp.write_bytes(data)
        tmp.replace(cache_file)
        return data, None
    except Exception as exc:  # noqa: BLE001 - any failure means "use the cache"
        problem = describe(exc, url)
        if cache_file.exists():
            stamp = (
                dt.datetime.fromtimestamp(cache_file.stat().st_mtime)
                .strftime("%d %b %H:%M")
                .lower()
            )
            return cache_file.read_bytes(), f"{problem}, showing copy from {stamp}"
        return None, problem


def local(value, all_day):
    """date/datetime -> local naive-free ISO string (dates stay dates)."""
    if all_day:
        return value.isoformat()
    if value.tzinfo is None:  # floating time: already local
        value = value.astimezone()
    return value.astimezone().isoformat(timespec="minutes")


def events_of(data, index, window):
    cal = x_wr_timezone.to_standard(icalendar.Calendar.from_ical(data))
    name = str(cal.get("X-WR-CALNAME") or f"calendar {index}")
    out = []
    for ev in recurring_ical_events.of(cal).between(*window):
        if str(ev.get("STATUS", "")).upper() == "CANCELLED":
            continue
        start = ev.decoded("DTSTART")
        all_day = not isinstance(start, dt.datetime)
        if "DTEND" in ev:
            end = ev.decoded("DTEND")
        elif "DURATION" in ev:
            end = start + ev.decoded("DURATION")
        else:
            end = start + dt.timedelta(days=1) if all_day else start
        if all_day and isinstance(end, dt.datetime):
            end = end.date()
        sort_key = (
            dt.datetime.combine(start, dt.time()).astimezone()
            if all_day
            else (start if start.tzinfo else start.astimezone())
        )
        out.append(
            (
                sort_key,
                {
                    "start": local(start, all_day),
                    "end": local(end, all_day),
                    "allDay": all_day,
                    "title": str(ev.get("SUMMARY", "")).strip() or "(no title)",
                    "location": str(ev.get("LOCATION", "")).strip(),
                    "calendar": name,
                },
            )
        )
    return out


def main():
    today = dt.date.today()
    prev_month = (today.replace(day=1) - dt.timedelta(days=1)).replace(day=1)
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument(
        "--urls-file",
        type=Path,
        default=xdg("XDG_CONFIG_HOME", ".config") / "quickshell-calendar" / "urls",
    )
    p.add_argument(
        "--cache-dir",
        type=Path,
        default=xdg("XDG_CACHE_HOME", ".cache") / "quickshell-calendar",
    )
    p.add_argument(
        "--from", dest="start", type=dt.date.fromisoformat, default=prev_month
    )
    p.add_argument("--days", type=int, default=100)
    args = p.parse_args()

    urls = read_urls(args.urls_file)
    args.cache_dir.mkdir(parents=True, exist_ok=True)
    window = (args.start, args.start + dt.timedelta(days=args.days))

    events, errors = [], []
    for i, url in enumerate(urls, 1):
        cache_file = args.cache_dir / (
            hashlib.sha256(url.encode()).hexdigest()[:16] + ".ics"
        )
        data, problem = fetch(url, cache_file)
        if problem:
            errors.append(f"calendar {i}: {problem}")
        if data is None:
            continue
        try:
            events += events_of(data, i, window)
        except Exception as exc:  # noqa: BLE001
            errors.append(f"calendar {i}: can't read feed ({describe(exc, url)})")

    events.sort(key=lambda e: (e[0], e[1]["title"]))
    json.dump(
        {"events": [e for _, e in events], "errors": errors, "configured": bool(urls)},
        sys.stdout,
        ensure_ascii=False,
    )
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
