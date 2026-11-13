#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""Sioul's sound beside the recordings: each measure's range over the
recordings, their median, Sioul's value, and "<<" or ">>" when Sioul's lies
outside the range.

    tools/sounds/compare.py REFS.json SIOUL.json
"""

import json
import sys

refs = json.load(open(sys.argv[1]))["results"]
mine = json.load(open(sys.argv[2]))["results"][0]


def row(name, values, value):
    values = [v for v in values if v is not None]
    if not values or value is None:
        return
    lo, hi = min(values), max(values)
    med = sorted(values)[len(values) // 2]
    flag = "<<" if value < lo else ">>" if value > hi else "  "
    print(f"{name:26s} {lo:8.2f} {med:8.2f} {hi:8.2f}   {value:8.2f} {flag}")


print(f"{'measure':26s} {'min':>8s} {'median':>8s} {'max':>8s}   {'Sioul':>8s}")
BANDS = [63 * 2 ** (k / 3) for k in range(22)]
for i, c in enumerate(BANDS):
    row(f"spectrum {c:6.0f} Hz", [r["spectrum"][i] for r in refs], mine["spectrum"][i])
for key in ("centroid", "spread_low", "spread_mid", "spread_high", "lag_high_after_low", "lag_match", "clicks", "stereo_low", "stereo_mid", "stereo_high", "flatness", "chirps_per_second", "claps_per_minute"):
    row(key, [r[key] for r in refs], mine[key])
for i, q in enumerate((10, 50, 90)):
    row(f"chirp pitch p{q}", [r["chirp_pitch"][i] for r in refs], mine["chirp_pitch"][i])
MOD = [0.03 * 2 ** k for k in range(10)]
for part in ("low", "mid", "high"):
    for i, m in enumerate(MOD):
        row(f"mod {part} {m:5.2f} Hz", [r[f"mod_{part}"][i] for r in refs], mine[f"mod_{part}"][i])
