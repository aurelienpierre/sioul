#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""The never-attributed blocks of the French numbering plan, for the call
screening's data (crates/sioul-core/data/screening-numbers.toml, the group
of kind "unattributed"; docs/android.md, "Calls").

A number shown to the person called must come from a block ARCEP attributed
to an operator (the plan, § 2.2.2 a). ARCEP publishes every attributed block
as open data, MAJNUM (https://extranet.arcep.fr/uploads/MAJNUM.csv, from the
dataset "Ressources en numérotation téléphonique" on data.gouv.fr). This
reads a copy of it and prints the group: the shortest prefixes, in national
form ("0937"), that cover the ten-digit numbers (01 to 09) no attributed
block holds, kept to whole blocks of 10,000 numbers or more (prefixes of six
digits at most). Download the file first, into a folder of its own, and run:

    python3 -I tools/screening-numbers.py MAJNUM.csv 2026-10-19

The date is the day the file was read; the group says it, with its source.
ARCEP attributes about a thousand blocks a year: the screening counts the
group only for `valid_days` after that date (crates/sioul-core/src/screening.rs).

The public-interest multi-use numbers (plan § 2.3.10 a) are left out, though
most of their blocks are attributed to nobody yet: a category of their own,
which the screening treats as any number nobody knows (research CS12).
"""

import csv
import sys

LONGEST = 6
PER_LINE = 12
# The roots of the public-interest multi-use numbers, in national form (plan
# § 2.3.10 a, version of 1 January 2026): +33; +590, +594 and +596; +262;
# +508. Never in the group, whatever MAJNUM says of their blocks.
INTEREST = (
    ["01510", "02810", "03410", "04410", "05410", "09410"]
    + [f"0598{n}" for n in range(50, 65)]
    + [f"09470{n}" for n in range(10)]
    + ["02687", "050860"]
)


def gaps(path):
    attributed = set()
    with open(path, encoding="latin-1", newline="") as f:
        for row in csv.DictReader(f, delimiter=";"):
            prefix = row.get("EZABPQM", "").strip()
            first = row.get("Tranche_Debut", "").strip()
            if len(first) == 10 and prefix.startswith("0") and prefix.isdigit():
                attributed.add(prefix)
    partial = {p[:i] for p in attributed for i in range(1, len(p))}
    out = []

    def walk(prefix):
        if prefix in attributed or any(prefix.startswith(root) for root in INTEREST):
            return
        if prefix not in partial and not any(root.startswith(prefix) for root in INTEREST):
            if len(prefix) <= LONGEST:
                out.append(prefix)
            return
        if len(prefix) >= LONGEST:
            return
        for digit in "0123456789":
            walk(prefix + digit)

    for z in "123456789":
        walk("0" + z)
    return out


MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: screening-numbers.py MAJNUM.csv YYYY-MM-DD")
    year, month, day = (int(part) for part in sys.argv[2].split("-"))
    read = f"{day} {MONTHS[month - 1]} {year}"
    prefixes = gaps(sys.argv[1])
    print("[[group]]")
    print('kind = "unattributed"')
    print('territory = "France, overseas departments included"')
    print("prefixes = [")
    for i in range(0, len(prefixes), PER_LINE):
        print("    " + ", ".join(f'"{p}"' for p in prefixes[i:i + PER_LINE]) + ",")
    print("]")
    print(f'source = "ARCEP, MAJNUM, the blocks attributed to operators (https://extranet.arcep.fr/uploads/MAJNUM.csv, of {read}); the rule: plan national de numérotation (annexe 1 à la décision n° 2018-0881 modifiée), version du 1er janvier 2026, § 2.2.2 a): https://www.arcep.fr/uploads/tx_gsavis/25-2215.pdf"')
    print(f'checked = "{sys.argv[2]}"')
    print("valid_days = 90")
    print(f"# {len(prefixes)} prefixes", file=sys.stderr)


if __name__ == "__main__":
    main()
