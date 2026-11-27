#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""No file call around the sealed store's door (docs/store.md, "Paths").

Every reader and writer of Sioul reaches its files through
`sioul_core::sealed::files` (`crate::files` in each crate): the standard
library's file calls with the same shape, which seal a path under Sioul's four
folders and leave any other path the system's. This check finds what goes
around it in the programs' sources: the standard library's own file calls
(any `std::fs`), and the path methods that ask the system alone
(`Path::exists`, `is_file`, `is_dir`, `try_exists`, `read_dir`,
`canonicalize`, `symlink_metadata`: the door's `Probe` and functions answer
for sealed files too). Tests (`tests.rs`, `*_tests.rs`, a file's inline
`#[cfg(test)] mod … {`) and benches (`bench.rs`, `*_bench.rs`) are left out,
and the door itself (`crates/sioul-core/src/sealed/`) is where they belong.

What must stay is listed with a count in `tools/store-door-allowed.txt`:
`filelock.rs`, the mechanics of a file of the system (written beside and
renamed, overwritten before it goes, read through its descriptor), which
takes the door's way at each entry for a sealed file; the standard input of
the agents' server. One line per source file and kind of call, its path, the
call and how many, separated by tabs; `#` lines are comments. The check
fails when a file makes a call the list does not allow, more of a kind than
its count, and fewer (lower its count: the list only shrinks).

    tools/check-store-door.py           the check
    tools/check-store-door.py --lower   write the lower counts a branch reached (never a higher one, never a new file)
    tools/check-store-door.py --init    the first list, from the sources as they are (refused once the list exists)
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIST = os.path.join(ROOT, "tools", "store-door-allowed.txt")
DOOR = "crates/sioul-core/src/sealed/"

CALL = re.compile(
    r"\bstd::fs\b"
    r"|\.(?:exists|is_file|is_dir|try_exists|read_dir|canonicalize|symlink_metadata)\(\)"
)


def kind(call):
    """A call's kind: `std::fs`, or the path method's name."""
    return call.lstrip(".").replace("()", "")


def sources():
    """The programs' Rust sources, relative to the repository, tests and benches left out."""
    found = []
    crates = os.path.join(ROOT, "crates")
    for dirpath, dirnames, filenames in os.walk(crates):
        dirnames[:] = sorted(d for d in dirnames if d not in ("target", "tests", "benches"))
        for name in sorted(filenames):
            if not name.endswith(".rs") or name in ("tests.rs", "bench.rs") or name.endswith(("_tests.rs", "_bench.rs")):
                continue
            path = os.path.relpath(os.path.join(dirpath, name), ROOT).replace(os.sep, "/")
            if "/src/" not in path or path.startswith(DOOR):
                continue
            found.append(path)
    return found


def calls(path):
    """The direct file calls a source holds, by kind, outside comments and its test module."""
    with open(os.path.join(ROOT, path), encoding="utf-8") as f:
        lines = f.read().split("\n")
    count = {}
    for i, line in enumerate(lines):
        stripped = line.strip()
        # A test module: `#[cfg(test)]`, attributes, then `mod … {`; nothing after it counts.
        if stripped.startswith("#[cfg(test)]"):
            following = next((l for l in lines[i + 1 : i + 4] if not l.strip().startswith("#[")), "")
            if re.match(r"\s*(pub(\([a-z]+\))?\s+)?mod\s+\w+\s*\{", following):
                break
        if stripped.startswith("//"):
            continue
        code = line.split("//", 1)[0] if "\"" not in line else line
        for match in CALL.finditer(code):
            count[kind(match.group(0))] = count.get(kind(match.group(0)), 0) + 1
    return count


def read_list():
    allowed = {}
    with open(LIST, encoding="utf-8") as f:
        for number, line in enumerate(f, 1):
            line = line.rstrip("\n")
            if not line.strip() or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) != 3 or not parts[2].isdigit():
                sys.exit(f"{LIST}:{number}: a file, a call and a count, by tabs: {line!r}")
            allowed[(parts[0], parts[1])] = int(parts[2])
    return allowed


HEADER = """# The file calls that go around the sealed store's door and must
# (tools/check-store-door.py, docs/store.md, "Paths"): filelock.rs keeps the
# mechanics of a file of the system and takes the door's way for a sealed
# file; the agents' server reads its standard input. This list only shrinks.
"""


def write_list(counts):
    with open(LIST, "w", encoding="utf-8") as f:
        f.write(HEADER)
        for (path, call) in sorted(counts):
            if counts[(path, call)] > 0:
                f.write(f"{path}\t{call}\t{counts[(path, call)]}\n")


def main(args):
    found = {}
    for path in sources():
        for call, count in calls(path).items():
            found[(path, call)] = count
    if args == ["--init"]:
        if os.path.exists(LIST):
            sys.exit(f"{LIST} exists: the list only shrinks (--lower).")
        write_list(found)
        print(f"{len({p for p, _ in found})} files, {sum(found.values())} direct calls listed.")
        return 0
    allowed = read_list()
    if args == ["--lower"]:
        lowered = dict(allowed)
        for key, count in allowed.items():
            if found.get(key, 0) < count:
                lowered[key] = found.get(key, 0)
        write_list(lowered)
        allowed = read_list()
        args = []
    if args:
        sys.exit(__doc__)
    problems = []
    for (path, call), count in sorted(found.items()):
        if (path, call) not in allowed:
            problems.append(f"{path}: {count} × {call}, and none allowed: go through crate::files (sioul_core::sealed::files)")
        elif count > allowed[(path, call)]:
            problems.append(f"{path}: {count} × {call}, {allowed[(path, call)]} allowed: go through crate::files (sioul_core::sealed::files)")
    for (path, call), count in sorted(allowed.items()):
        now = found.get((path, call), 0)
        if now < count:
            problems.append(f"{path}: {now} × {call}, {count} listed: lower its count (tools/check-store-door.py --lower)")
    if problems:
        print("\n".join(problems))
        return 1
    print(f"No file call around the door: {sum(found.values())} in {len({p for p, _ in found})} files, as listed.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
