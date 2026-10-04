#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Builds Sioul's website into website/site/ (the French guide into
# website/site/fr/), here and in the Pages workflow (.github/workflows/pages.yml).
#
# The developers' section is the design notes of ../docs, as they are: they are
# copied into docs/dev/ at each build (git-ignored there, but for the
# docs/dev/index.md written for the site), never by hand. On the copies only,
# links that leave ../docs (to ../examples, ../packaging…) are turned into
# their address on GitHub, since the site does not hold those files.
#
#   website/build.sh             build into website/site/
#   website/build.sh --strict    stop at the first warning (the workflow does)
#   website/build.sh serve       build, then show it at http://localhost:8000/sioul/
#                                while you edit (run it again after changing ../docs)
#
# Needs Python 3.11 or newer, and Zensical: pip install zensical==0.0.67
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v zensical >/dev/null; then
    echo "build.sh: Zensical is not installed: python3 -m venv <folder>, then <folder>/bin/pip install zensical==0.0.67 and run this again with <folder>/bin on PATH." >&2
    exit 1
fi

python3 - "$here" <<'PY'
import re
import shutil
import sys
import tomllib
from pathlib import Path

website = Path(sys.argv[1]).resolve()
root = website.parent
notes = root / "docs"
dev = website / "docs" / "dev"
repository = "https://github.com/aurelienpierre/sioul"

# What was copied last time goes: a note removed from ../docs leaves the site.
dev.mkdir(parents=True, exist_ok=True)
for entry in dev.iterdir():
    if entry.name == "index.md":
        continue
    if entry.is_dir():
        shutil.rmtree(entry)
    else:
        entry.unlink()

link = re.compile(r'(\]\()(<[^>]*>|[^)\s]+)((?:\s+"[^"]*")?\))')
reference = re.compile(r'^(\s{0,3}\[[^\]]+\]:\s*)(\S+)')
fence = re.compile(r'^\s*(```|~~~)')


def on_github(target: str, source: Path) -> str | None:
    """GitHub's address for a relative link that leaves ../docs, else None."""
    bare = target.strip("<>")
    if not bare or bare.startswith(("#", "/")) or re.match(r"^[a-z][a-z0-9+.-]*:", bare, re.I):
        return None
    path, hash_sign, anchor = bare.partition("#")
    resolved = (source.parent / path).resolve()
    if resolved.is_relative_to(notes) or not resolved.is_relative_to(root):
        return None
    kind = "tree" if resolved.is_dir() else "blob"
    return f"{repository}/{kind}/main/{resolved.relative_to(root).as_posix()}{hash_sign}{anchor}"


item = re.compile(r"^( *)([-*+]|\d+[.)])( +)(.*\n?)$")


def lists(text: str) -> str:
    """Lists as the site's Markdown reads them: the notes are written for
    GitHub, where a list may follow a line of text and nest by two or three
    spaces; Python-Markdown wants a blank line before a list and four spaces
    a level. Code is left alone."""
    out, in_code, levels = [], False, []
    previous = ""
    for line in text.splitlines(keepends=True):
        if fence.match(line):
            in_code = not in_code
        found = None if in_code else item.match(line)
        if found:
            indent = len(found.group(1))
            if not levels:
                levels = [indent]
                # A list right under a line of text starts its own block.
                if previous.strip() and not item.match(previous) and not previous.lstrip().startswith(("|", ">", "#")):
                    out.append("\n")
            elif indent > levels[-1]:
                levels.append(indent)
            else:
                while len(levels) > 1 and levels[-1] > indent:
                    levels.pop()
            line = " " * (4 * (len(levels) - 1)) + found.group(2) + " " + found.group(4)
        elif not in_code and line.strip() and not line.startswith(" "):
            # A line of text at the margin ends the list (or continues the item lazily).
            if not (levels and previous.strip() and item.match(previous)):
                levels = []
        elif not in_code and line.strip() and levels:
            # A line inside an item: under its item's text.
            line = " " * (4 * len(levels)) + line.lstrip(" ")
        elif not line.strip():
            pass
        out.append(line)
        previous = line
    return "".join(out)


def rewrite(text: str, source: Path) -> str:
    out, in_code = [], False
    for line in text.splitlines(keepends=True):
        if fence.match(line):
            in_code = not in_code
        if not in_code:
            line = link.sub(lambda m: m.group(1) + (on_github(m.group(2), source) or m.group(2)) + m.group(3), line)
            line = reference.sub(lambda m: m.group(1) + (on_github(m.group(2), source) or m.group(2)), line)
        out.append(line)
    return lists("".join(out))


for source in sorted(notes.rglob("*")):
    if source.is_dir() or any(part.startswith(".") for part in source.relative_to(notes).parts):
        continue
    target = dev / source.relative_to(notes)
    if target == dev / "index.md":
        print("build.sh: ../docs/index.md is left out: docs/dev/index.md is the site's own page.", file=sys.stderr)
        continue
    target.parent.mkdir(parents=True, exist_ok=True)
    if source.suffix == ".md":
        target.write_text(rewrite(source.read_text(encoding="utf-8"), source), encoding="utf-8")
    else:
        shutil.copy2(source, target)

# Every page in the navigation, and every note listed there.
config = tomllib.loads((website / "zensical.toml").read_text(encoding="utf-8"))


def listed(items):
    for item in items:
        if isinstance(item, str):
            yield item
        elif isinstance(item, dict):
            for value in item.values():
                yield from ([value] if isinstance(value, str) else listed(value))


pages = website / "docs"
in_nav = {entry for entry in listed(config["project"]["nav"]) if entry.endswith(".md")}
copied = {p.relative_to(pages).as_posix() for p in dev.rglob("*.md")}
for entry in sorted(in_nav):
    if not (pages / entry).is_file():
        print(f"build.sh: the navigation (website/zensical.toml) names {entry}, which is not there.", file=sys.stderr)
for entry in sorted(copied - in_nav):
    print(f"build.sh: {entry} is on the site but not in its navigation: add it to website/zensical.toml.", file=sys.stderr)
PY

# The French guide (fr/) shares the pictures and styles: copied in at each
# build (git-ignored there). A French page missing beside an English one is
# said, so that none is forgotten.
rm -rf "$here/fr/docs/assets"
cp -r "$here/docs/assets" "$here/fr/docs/assets"
for page in "$here"/docs/index.md "$here"/docs/privacy.md "$here"/docs/guide/*.md; do
    french="$here/fr/docs/${page#"$here"/docs/}"
    [ -f "$french" ] || echo "build.sh: no French page yet for docs/${page#"$here"/docs/}." >&2
done

cd "$here"
if [ "${1:-}" = "serve" ]; then
    exec zensical serve
fi
# English first: its build empties site/, French goes into site/fr/ after.
zensical build --clean "$@"
exec zensical build --clean -f zensical-fr.toml "$@"
