#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""The moment the demo shows, for screenshots.sh and run.sh:

    tools/demo/moment.py [--hour H]

prints the time and the time zone to run Sioul in, "2026-10-07T14:05:00+02:00
DEMO-02", or "none none" when no weekday is close enough.

A weekday at 14:xx, else another hour of work (15, 10, 9, 11, 16), or the
hour asked (`--hour 19`: after the demo's work hours), whatever the day it
is: in a zone a whole number of hours from UTC (the day view draws its hours
on UTC's) and at most a day away; the minutes are the clock's. Sioul runs in
that zone (TZ=DEMO-hh, a fixed offset), which it reads as the system does,
and the demo profile is made for that moment (make-demo.py --now). From
Saturday 16:00 to Sunday 10:00 (UTC) no weekday is close enough at 14:00.
"""

import argparse
from datetime import datetime, timedelta, timezone


def moment(real: datetime, hours: tuple[int, ...]) -> str:
    """`real`, a UTC time to the minute, seen at the first of `hours` that a weekday within a day has."""
    for hour in hours:
        found = []
        for days in (-1, 0, 1):
            offset = hour - real.hour + 24 * days
            # A fixed offset stays within a day (Python's limit, and POSIX's).
            if abs(offset) <= 23 and (real + timedelta(hours=offset)).weekday() < 5:
                found.append(offset)
        if found:
            offset = min(found, key=abs)
            local = real.astimezone(timezone(timedelta(hours=offset)))
            # POSIX writes the offset the other way round: DEMO-02 is two hours east.
            return f"{local.isoformat()} DEMO{'-' if offset >= 0 else '+'}{abs(offset):02d}"
    return "none none"


def hour(text: str) -> int:
    """An hour of the day, as --hour takes it."""
    if not text.isdigit() or int(text) > 23:
        raise argparse.ArgumentTypeError(f"an hour, 0 to 23, not {text!r}")
    return int(text)


def main() -> None:
    parser = argparse.ArgumentParser(description="The moment the demo shows: its time and its time zone.")
    parser.add_argument("--hour", type=hour, metavar="H", help="this hour of a weekday, rather than one of work")
    args = parser.parse_args()
    hours = (args.hour,) if args.hour is not None else (14, 15, 10, 9, 11, 16)
    print(moment(datetime.now(timezone.utc).replace(second=0, microsecond=0), hours))


if __name__ == "__main__":
    main()
