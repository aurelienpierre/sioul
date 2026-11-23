#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Checks every official calendar of holidays Sioul offers
(crates/sioul-core/data/holiday-calendars.toml), before each release.

Each address is asked as Sioul asks it (`User-Agent: Sioul`, its `Accept`,
nothing compressed, three redirects at most, ten megabytes at most). It says,
for each:
  ok        it answers a whole calendar, every event a whole day, from this
            year to the next at least; served as `text/calendar`, or, said
            beside it, as another type (the Ministry's files come as
            `application/octet-stream`): Sioul reads the content, not the type;
  moved     the same, from another address: worth updating the file;
  short     the same, but nothing yet in the next year: its publisher may not
            have published it yet (a school year of the southern hemisphere
            begins in February): check by hand;
  blocked   it refuses (401, 403, 429, a server's error): check by hand;
  silent    no answer in time, a reset, a certificate this script cannot
            follow: check by hand;
  broken    not found (404, 410), no such host, not a whole calendar (a page,
            a line of text, cut short), an event with a time, or nothing in
            this year: fix it, or take the calendar out.

Exits with 1 when a calendar is broken, so it can run on a schedule. The
tests never run it: it needs the network.

    tools/check-holidays.py                     # the file of this source tree
    tools/check-holidays.py path/to/holiday-calendars.toml
    tools/check-holidays.py --only GB           # a country
    tools/check-holidays.py --json > report.json
"""

import argparse
import concurrent.futures
import datetime
import json
import pathlib
import re
import socket
import ssl
import sys
import tomllib
import urllib.error
import urllib.parse
import urllib.request

# As Sioul's own reader (sioul-sync's `feeds`): its name, what it accepts, nothing compressed, ten megabytes, three redirects.
HEADERS = {"User-Agent": "Sioul", "Accept": "text/calendar, text/plain;q=0.8, */*;q=0.5", "Accept-Encoding": "identity"}
MOST_BYTES = 10_000_000
MOST_REDIRECTS = 3
BLOCKED = {401, 403, 405, 406, 429, 451}
BROKEN = {404, 410}


class Count(urllib.request.HTTPRedirectHandler):
    """Follows three redirects at most, https alone, as Sioul does."""

    max_redirections = MOST_REDIRECTS

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).scheme != "https":
            raise urllib.error.HTTPError(newurl, code, "a redirect away from https", headers, fp)
        new = super().redirect_request(req, fp, code, msg, headers, newurl)
        if new is not None:
            for name, value in HEADERS.items():
                new.add_header(name, value)
        return new


def unfold(text):
    """The calendar's lines, unfolded (RFC 5545 §3.1)."""
    return re.sub(r"\r?\n[ \t]", "", text).splitlines()


def name_of(line):
    """A line's name, its parameters aside: "DTSTART;VALUE=DATE:20301225" gives "DTSTART"."""
    return re.split(r"[;:]", line, maxsplit=1)[0].strip().upper()


def value_of(line):
    """A line's value, after the first colon outside quotes."""
    quoted = False
    for at, c in enumerate(line):
        if c == '"':
            quoted = not quoted
        elif c == ":" and not quoted:
            return line[at + 1:].strip()
    return ""


def read(text, year):
    """What a calendar holds: (problem or "", events, whole days, first year, last year, the
    next year missing). A year is held when an event starts in it."""
    open_, ended, events, whole, years, timed = [], False, 0, 0, set(), 0
    for line in unfold(text.lstrip("﻿")):
        if not line.strip():
            continue
        name, value = name_of(line), value_of(line).upper()
        if name == "BEGIN":
            if not open_ and value != "VCALENDAR":
                return "a line outside the calendar", 0, 0, None, None, False
            open_.append(value)
            events += value == "VEVENT"
            ended = False
        elif name == "END":
            if not open_ or open_.pop() != value:
                return f"END:{value} out of order", 0, 0, None, None, False
            ended = not open_
        elif not open_:
            return "a line outside the calendar", 0, 0, None, None, False
        elif name == "DTSTART" and open_[-1] == "VEVENT":
            if re.fullmatch(r"\d{8}", value):
                whole += 1
                years.add(int(value[:4]))
            else:
                timed += 1
    if not ended:
        return "no end of the calendar: cut short", 0, 0, None, None, False
    if events == 0:
        return "no event", 0, 0, None, None, False
    first, last = min(years, default=None), max(years, default=None)
    if timed:
        return f"{timed} of its {events} events have a time, not a whole day", events, whole, first, last, False
    if year not in years:
        return f"nothing in {year}", events, whole, first, last, False
    return "", events, whole, first, last, year + 1 not in years


def ask(url, year, timeout):
    """(verdict, status, final address, detail, events, first year, last year)."""
    opener = urllib.request.build_opener(Count(), urllib.request.HTTPSHandler(context=ssl.create_default_context()))
    request = urllib.request.Request(url, headers=HEADERS)
    try:
        with opener.open(request, timeout=timeout) as answer:
            final, status = answer.geturl(), answer.status
            kind = answer.headers.get_content_type()
            body = answer.read(MOST_BYTES + 1)
    except urllib.error.HTTPError as error:
        final, status = error.geturl() or url, error.code
        if status in BROKEN:
            return "broken", status, final, "not found", 0, None, None
        if status in BLOCKED or status >= 500:
            return "blocked", status, final, str(error.reason), 0, None, None
        return "broken", status, final, str(error.reason), 0, None, None
    except (urllib.error.URLError, socket.timeout, ssl.SSLError, ConnectionError, TimeoutError) as error:
        reason = getattr(error, "reason", error)
        if isinstance(reason, socket.gaierror):
            return "broken", 0, url, "no such host", 0, None, None
        return "silent", 0, url, str(reason), 0, None, None
    if len(body) > MOST_BYTES:
        return "broken", status, final, "more than ten megabytes", 0, None, None
    problem, events, whole, first, last, short = read(body.decode("utf-8", errors="replace"), year)
    typed = "" if kind == "text/calendar" else f"served as {kind}, not text/calendar: read by its content, as Sioul reads it"
    if problem:
        return "broken", status, final, "; ".join(filter(None, [problem, typed])), events, first, last
    if final != url:
        return "moved", status, final, "; ".join(filter(None, [f"now {final}", typed])), events, first, last
    if short:
        return "short", status, final, "; ".join(filter(None, [f"nothing yet in {year + 1}", typed])), events, first, last
    return "ok", status, final, typed, events, first, last


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    here = pathlib.Path(__file__).resolve().parent.parent / "crates" / "sioul-core" / "data" / "holiday-calendars.toml"
    parser.add_argument("file", nargs="?", default=str(here))
    parser.add_argument("--only", help="one country's code (ISO 3166-1, as the file writes it)")
    parser.add_argument("--timeout", type=float, default=25)
    parser.add_argument("--json", action="store_true", help="the report as JSON")
    args = parser.parse_args()

    calendars = tomllib.loads(pathlib.Path(args.file).read_text(encoding="utf-8")).get("calendar", [])
    listed = [c for c in calendars if args.only in (None, c.get("country"))]
    year = datetime.date.today().year
    report = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        futures = {pool.submit(ask, c["url"], year, args.timeout): c for c in listed}
        for future in concurrent.futures.as_completed(futures):
            c = futures[future]
            verdict, status, final, detail, events, first, last = future.result()
            report.append({"country": c["country"], "kind": c["kind"], "id": c["id"], "url": c["url"], "verdict": verdict, "status": status, "final": final, "detail": detail, "events": events, "from": first, "until": last})
    order = ["broken", "moved", "short", "silent", "blocked", "ok"]
    report.sort(key=lambda r: (order.index(r["verdict"]), r["country"], r["id"]))

    if args.json:
        json.dump(report, sys.stdout, ensure_ascii=False, indent=1)
        print()
    else:
        for r in report:
            status = r["status"] or "—"
            years = f"{r['from']}–{r['until']}" if r["from"] else "—"
            held = f"{r['events']:>3} events, {years}" if r["events"] else "—"
            print(f"{r['verdict']:8} {status:>4}  {r['id']:34} {held:22} {r['url']}" + (f"  ({r['detail']})" if r["detail"] else ""))
        counts = {v: sum(1 for r in report if r["verdict"] == v) for v in order}
        typed = sum(1 for r in report if "served as" in r["detail"])
        print(f"\n{len(report)} calendars ({year} and {year + 1} asked): {counts['ok']} ok, {counts['moved']} moved, {counts['short']} short, {counts['blocked']} blocked and {counts['silent']} silent (check by hand), {counts['broken']} broken; {typed} not served as text/calendar.")
    return 1 if any(r["verdict"] == "broken" for r in report) else 0


if __name__ == "__main__":
    sys.exit(main())
