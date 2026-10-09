#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Checks the window's and the core's words against the Fluent files.

- Every message the code asks for by a literal key (QML `sioul.text("…")`,
  `textWith`, `textArgs`, `textCounted`; Rust `text("…")`, `say("…")`, `attribute("…")`)
  exists in each language.
- French follows its typography: a narrow no-break space (U+202F) before
  `:` `;` `?` `!` and inside « », the apostrophe ’.

Keys built at run time ("area-" + kind) cannot be checked here: they are
listed as prefixes, for a reader to look at. Exit status 1 when something is
missing or misspelt, so that it can run in CI.

    tools/check-messages.py [--unused]
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCALES = ROOT / "crates" / "sioul-core" / "locales"
SOURCES = list((ROOT / "crates" / "sioul-app" / "qml").glob("*.qml")) + [p for p in ROOT.glob("crates/*/src/**/*.rs")]

KEY = r'"([a-z][a-z0-9]*(?:-[a-z0-9]+)+)"'
CALLS = [
    re.compile(r'\b(?:text|textWith|textArgs|textCounted|say|attribute|text_with)\(\s*' + KEY),
]
PREFIX = re.compile(r'\b(?:text|textWith|textArgs|say)\(\s*"([a-z][a-z0-9-]*-)"\s*\+')
FORMAT_PREFIX = re.compile(r'format!\("([a-z][a-z0-9-]*-)\{')


def messages(language):
    """The ids of a language's messages, and its values line by line (for typography)."""
    ids, values = set(), []
    for path in sorted((LOCALES / language).glob("*.ftl")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            found = re.match(r"^([a-z][a-zA-Z0-9-]*)\s*=\s*(.*)$", line)
            if found:
                ids.add(found.group(1))
                values.append((path.name, number, found.group(2)))
            elif line.startswith((" ", "\t")) and not line.strip().startswith((".", "[", "*[", "}")):
                values.append((path.name, number, line.strip()))
            elif re.match(r"^\s+\.[a-z]+\s*=", line):
                values.append((path.name, number, line.split("=", 1)[1].strip()))
            elif re.match(r"^\s+\*?\[[^\]]+\]", line):
                values.append((path.name, number, re.sub(r"^\s+\*?\[[^\]]+\]\s*", "", line)))
    return ids, values


def used():
    """Literal keys asked for in the sources, with where; prefixes of keys built at run time."""
    keys, prefixes = {}, {}
    for path in SOURCES:
        text = path.read_text(encoding="utf-8")
        for number, line in enumerate(text.splitlines(), 1):
            if line.lstrip().startswith(("//", "///", "//!")):
                continue
            for call in CALLS:
                for key in call.findall(line):
                    keys.setdefault(key, []).append(f"{path.relative_to(ROOT)}:{number}")
            for prefix in PREFIX.findall(line) + FORMAT_PREFIX.findall(line):
                prefixes.setdefault(prefix, []).append(f"{path.relative_to(ROOT)}:{number}")
    return keys, prefixes


def french_typography(values):
    """Spaces where French wants a narrow no-break one, and straight apostrophes."""
    problems = []
    for name, number, value in values:
        # Placeables ({ $x }) and markup are not prose.
        prose = re.sub(r"\{[^}]*\}", "", value)
        prose = re.sub(r"<[^>]+>", "", prose)
        prose = re.sub(r"https?://\S*", "", prose)
        # Mail headers keep their own colon: "Precedence: bulk".
        prose = re.sub(r"\b(?:Precedence|List-Id|List-Unsubscribe|Authentication-Results|Received|Message-ID|Reply-To|From|To|Subject|Date|X-[A-Za-z-]+):", "", prose)
        for mark in ":;?!":
            for found in re.finditer(r"(\S)?([  ]?)" + re.escape(mark), prose):
                before, space = found.group(1), found.group(2)
                # "Ctrl+N:"-like and times ("9:00") are not prose punctuation.
                if mark == ":" and before is not None and before.isdigit():
                    continue
                if space != " " and before is not None:
                    problems.append(f"fr/{name}:{number}: '{mark}' wants a narrow no-break space before it: {value.strip()[:90]}")
                    break
        if "'" in prose:
            problems.append(f"fr/{name}:{number}: straight apostrophe, French takes ’: {value.strip()[:90]}")
        if re.search(r"«[  ]|[  ]»", prose) or re.search(r"«[^ ]|[^ ]»", prose):
            problems.append(f"fr/{name}:{number}: « » want narrow no-break spaces inside: {value.strip()[:90]}")
    return problems


def main():
    show_unused = "--unused" in sys.argv
    languages = sorted(p.name for p in LOCALES.iterdir() if p.is_dir())
    known = {language: messages(language) for language in languages}
    keys, prefixes = used()
    failed = False
    for language in languages:
        ids = known[language][0]
        missing = sorted(k for k in keys if k not in ids)
        for key in missing:
            failed = True
            print(f"missing in {language}: {key}  ({', '.join(keys[key][:3])})")
    if "fr" in known:
        for problem in french_typography(known["fr"][1]):
            failed = True
            print(problem)
    if show_unused:
        ids = known.get("en", (set(), []))[0]
        for key in sorted(ids):
            if key not in keys and not any(key.startswith(p) for p in prefixes):
                print(f"never asked for by a literal key (maybe built at run time): {key}")
        for prefix, where in sorted(prefixes.items()):
            print(f"keys built at run time: {prefix}…  ({', '.join(where[:2])})")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
