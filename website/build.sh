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
# The API reference is made from the code's own comments (docs/dev/api.md).
# The QML files' page, docs/dev/qml.md, is written at each build by
# ../tools/qml-docs.py (git-ignored, as the copied notes are). With --api,
# rustdoc documents the five crates, their private items included (into
# target/doc/, where `cargo doc` puts it), and javadoc the Android code; both
# are copied into site/api/rust/ and site/api/java/ once Zensical has built
# the site. --api-only makes them and copies them into the site built before,
# touching nothing else: the workflow builds the site first, then the
# references, so that a reference that fails never holds back the guide.
#
#   website/build.sh             build into website/site/
#   website/build.sh --strict    stop at the first warning (the workflow does)
#   website/build.sh --api       also the Rust and Java references (with --strict,
#                                their warnings stop the build too)
#   website/build.sh --api-only  the references alone, into the site built before
#                                (the workflow runs --strict, then --strict --api-only)
#   website/build.sh serve       build, then show it at http://localhost:8000/sioul/
#                                while you edit (run it again after changing ../docs)
#
# Needs Python 3.11 or newer, and Zensical: pip install zensical==0.0.67
# --api and --api-only also need Rust (cargo), Qt 6's development files (the
# window's crate builds its QML module even to be documented:
# docs/building.md), and a JDK (javadoc). Android's android.jar (the SDK's
# platforms;android-36, found by ANDROID_JAR, ANDROID_HOME, ANDROID_SDK_ROOT,
# else ~/Android/Sdk) links Android's classes to Android's own reference;
# without it the Java reference is made all the same, those classes unlinked
# (never with --strict).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --api and --api-only are this script's own; every other argument goes on to
# Zensical, as before (each is shifted off the front of the list and put back
# at its end).
api=no
api_only=no
strict=no
for argument in "$@"; do
    shift
    case "$argument" in
        --api) api=yes ;;
        --api-only) api=yes api_only=yes ;;
        *)
            [ "$argument" != --strict ] || strict=yes
            set -- "$@" "$argument"
            ;;
    esac
done

if [ "$api_only" = no ] && ! command -v zensical >/dev/null; then
    echo "build.sh: Zensical is not installed: python3 -m venv <folder>, then <folder>/bin/pip install zensical==0.0.67 and run this again with <folder>/bin on PATH." >&2
    exit 1
fi
if [ "$api_only" = yes ] && [ ! -f "$here/site/index.html" ]; then
    echo "build.sh: --api-only adds the references to the site built before: run website/build.sh first." >&2
    exit 1
fi

# The site's pages: the notes copied, the QML page written, the French
# guide's pictures. --api-only leaves the site as it was built.
if [ "$api_only" = no ]; then
    python3 - "$here" <<'PY'
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

website = Path(sys.argv[1]).resolve()
root = website.parent
notes = root / "docs"
dev = website / "docs" / "dev"
repository = "https://github.com/aurelienpierre/sioul"

# The site's own pages in docs/dev/, written for it and kept by git.
own = {"index.md", "api.md"}

# What was copied last time goes: a note removed from ../docs leaves the site.
dev.mkdir(parents=True, exist_ok=True)
for entry in dev.iterdir():
    if entry.name in own:
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
    a level. Code is left alone.

    Python-Markdown also reads a block (lines between blank lines) that
    starts indented as wholly inside the item above it: an item less
    indented than such a block's first line would be swallowed into it, as
    text or as a deeper item. Such an item starts a block of its own (a blank
    line before it). And a paragraph after a blank line belongs, as on GitHub,
    to the item its indentation is under, not always to the deepest one."""
    out, in_code, levels = [], False, []
    previous = ""
    # The indentation of the block's first line, as written out; the depth of
    # the paragraph going on inside a list (None: none, or a lazy one).
    block, paragraph = 0, None
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
                    previous = "\n"
            elif indent > levels[-1]:
                levels.append(indent)
            else:
                while len(levels) > 1 and levels[-1] > indent:
                    levels.pop()
            line = " " * (4 * (len(levels) - 1)) + found.group(2) + " " + found.group(4)
            # Less indented than its block's start: a block of its own, or it is swallowed.
            if previous.strip() and 4 * (len(levels) - 1) < block:
                out.append("\n")
                previous = "\n"
            paragraph = None
        elif not in_code and line.strip() and not line.startswith(" "):
            # A line of text at the margin ends the list (or continues the item lazily).
            if not (levels and previous.strip() and item.match(previous)):
                levels = []
            paragraph = None
        elif not in_code and line.strip() and levels:
            # A line inside an item: under its item's text. After a blank line,
            # under the item its indentation is under (GitHub's reading); a
            # paragraph's next lines go with it; without a blank line, the
            # deepest item's text goes on (a lazy continuation).
            if not previous.strip():
                indent = len(line) - len(line.lstrip(" "))
                paragraph = max(1, sum(1 for level in levels if level < indent))
            depth = paragraph if paragraph is not None else len(levels)
            line = " " * (4 * depth) + line.lstrip(" ")
        elif not line.strip():
            pass
        if line.strip() and not previous.strip():
            block = len(line) - len(line.lstrip(" "))
        out.append(line)
        previous = line
    return "".join(out)


# The shapes that once broke a page, checked at each build: an item after a
# paragraph or a nested list resumed past a blank line stays an item of its
# own list, never text of the paragraph above nor a deeper item; a lazy line
# under an item is left as it was.
for shape, wanted in [
    ("- A\n  - a1\n\n  Text of A.\n- B\n", "- A\n    - a1\n\n    Text of A.\n\n- B\n"),
    ("- A\n  - a1\n\n  - a2\n- B\n", "- A\n    - a1\n\n    - a2\n\n- B\n"),
    ("- A\n\n  | x |\n  |---|\n  | 1 |\n- B\n", "- A\n\n    | x |\n    |---|\n    | 1 |\n\n- B\n"),
    ("- A\n  goes on\n- B\n", "- A\n    goes on\n- B\n"),
]:
    if lists(shape) != wanted:
        sys.exit(f"build.sh: lists() reads {shape!r} as {lists(shape)!r}, not {wanted!r}")


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
    if target.parent == dev and target.name in own | {"qml.md"}:
        print(f"build.sh: ../docs/{target.name} is left out: docs/dev/{target.name} is the site's own page.", file=sys.stderr)
        continue
    target.parent.mkdir(parents=True, exist_ok=True)
    if source.suffix == ".md":
        target.write_text(rewrite(source.read_text(encoding="utf-8"), source), encoding="utf-8")
    else:
        shutil.copy2(source, target)

# The QML files' page, from their comments: needs Python alone, so it is
# written at every build, --api or not.
subprocess.run([sys.executable, str(root / "tools" / "qml-docs.py"), "--out", str(dev / "qml.md")], check=True)

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
fi

cd "$here"
if [ "$api_only" = no ] && [ "${1:-}" = "serve" ]; then
    [ "$api" = no ] || echo "build.sh: serve shows the site without the Rust and Java references; cargo doc --open shows the Rust one." >&2
    exec zensical serve
fi

# The Rust and Java references, made before the site, so that a missing tool
# or a failing build stops at once; copied in after it, since Zensical's
# --clean empties site/.
if [ "$api" = yes ]; then
    root="$(dirname "$here")"
    for tool in cargo javadoc; do
        if ! command -v "$tool" >/dev/null; then
            echo "build.sh: the API reference needs $tool: Rust (https://rustup.rs) for cargo, a JDK for javadoc." >&2
            exit 1
        fi
    done

    # Rust: one crate at a time, its private items included (the window's
    # modules are all private, and a contributor reads them too). The command
    # line's crate is the program `sioul`: its pages are in sioul/. With
    # --strict, rustdoc's warnings (a link to nothing, an unclosed tag) are errors.
    flags="${RUSTDOCFLAGS:-}"
    [ "$strict" = no ] || flags="$flags -D warnings"
    for crate in sioul-core sioul-sync sioul-learn sioul-cli sioul-app; do
        (cd "$root" && RUSTDOCFLAGS="$flags" cargo doc --locked --no-deps --document-private-items -p "$crate")
    done
    # Where cargo builds: target/, unless CARGO_TARGET_DIR or a cargo configuration says otherwise.
    rust_doc="$(cd "$root" && cargo metadata --format-version 1 --no-deps --locked | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')/doc"

    # Java: the Android side (../android/package/src), its private members
    # included, made in a folder of its own. Android's classes link to
    # Android's reference by android.jar's package names, nothing fetched;
    # the JDK's own link to Oracle's, as javadoc does by itself.
    java="$(mktemp -d)"
    trap 'rm -rf "$java"' EXIT
    jar="${ANDROID_JAR:-}"
    if [ -n "$jar" ] && [ ! -f "$jar" ]; then
        echo "build.sh: ANDROID_JAR names $jar, which is not there." >&2
        exit 1
    fi
    if [ -z "$jar" ]; then
        for sdk in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Android/Sdk"; do
            if [ -n "$sdk" ] && [ -f "$sdk/platforms/android-36/android.jar" ]; then
                jar="$sdk/platforms/android-36/android.jar"
                break
            fi
        done
    fi
    options=(-private -encoding UTF-8 -docencoding UTF-8 -charset UTF-8 -notimestamp -quiet -linksource
        -windowtitle "Sioul's Android code" -doctitle "Sioul's Android code"
        -sourcepath "$root/android/package/src" -d "$java/html")
    if [ -n "$jar" ]; then
        mkdir "$java/android"
        python3 - "$jar" "$java/android/element-list" <<'LIST'
import sys
import zipfile

# Android's own packages (android.*, dalvik.*, org.json), as javadoc's
# element-list: java.* and javax.* are the JDK's, linked to Oracle's. The
# name element-list, rather than package-list, makes javadoc link a method
# as Android's pages name it: #getResultCode(), not #getResultCode--.
names = zipfile.ZipFile(sys.argv[1]).namelist()
packages = sorted({n.rsplit("/", 1)[0].replace("/", ".") for n in names if n.endswith(".class") and "/" in n})
kept = [p for p in packages if p.startswith(("android.", "dalvik.")) or p == "org.json"]
with open(sys.argv[2], "w", encoding="utf-8") as listing:
    listing.write("\n".join(kept) + "\n")
LIST
        # The comments are checked (their HTML, their references), not asked
        # to name every parameter.
        options+=(--class-path "$jar" -linkoffline https://developer.android.com/reference/ "$java/android" -Xdoclint:all,-missing)
        [ "$strict" = no ] || options+=(-Werror)
    elif [ "$strict" = yes ]; then
        echo "build.sh: --strict --api needs Android's android.jar (the Android SDK's platforms;android-36; ANDROID_JAR may name it)." >&2
        exit 1
    else
        echo "build.sh: no android.jar (ANDROID_JAR, or ANDROID_HOME with platforms;android-36): Android's classes stay unlinked in the Java reference." >&2
        options+=(--ignore-source-errors -Xdoclint:none)
    fi
    if ! javadoc "${options[@]}" com.aurelienpierre.sioul; then
        if [ "$strict" = yes ] || [ ! -f "$java/html/index.html" ]; then
            echo "build.sh: javadoc failed (above)." >&2
            exit 1
        fi
        echo "build.sh: javadoc found faults in the comments (above); the Java reference is made all the same." >&2
    fi
fi

# English first: its build empties site/, French goes into site/fr/ after.
if [ "$api_only" = no ]; then
    zensical build --clean "$@"
    zensical build --clean -f zensical-fr.toml "$@"
fi

if [ "$api" = yes ]; then
    # Sioul's crates, their sources' pages, and rustdoc's own files beside them
    # (its scripts and styles, the search index, the list of crates); an
    # earlier copy goes first (--api-only, over a site that had one).
    crates=(sioul_core sioul_sync sioul_learn sioul sioul_app)
    rm -rf "$here/site/api"
    mkdir -p "$here/site/api/rust/src" "$here/site/api/java"
    for crate in "${crates[@]}"; do
        cp -r "$rust_doc/$crate" "$here/site/api/rust/"
        cp -r "$rust_doc/src/$crate" "$here/site/api/rust/src/"
    done
    # target/doc/ may also hold other crates' pages, from another `cargo doc`
    # (a folder with an all.html is a crate's): they are left behind.
    others=()
    for entry in "$rust_doc"/*; do
        name="${entry##*/}"
        case " ${crates[*]} src " in
            *" $name "*) continue ;;
        esac
        if [ -f "$entry/all.html" ]; then
            others+=("$name")
        else
            cp -r "$entry" "$here/site/api/rust/"
        fi
    done
    if [ "${#others[@]}" -gt 0 ]; then
        echo "build.sh: target/doc/ also holds ${others[*]}: the copied list of crates and the search name them too (remove target/doc/ to start afresh)." >&2
    fi
    cp -r "$java/html/." "$here/site/api/java/"
fi
