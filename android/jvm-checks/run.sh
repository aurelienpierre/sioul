#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# The phone's Java, checked on a computer's JVM against what Rust decides: no
# phone, no emulator, nothing built by Gradle.
#
#     android/jvm-checks/run.sh [--no-javadoc]
#
# Sioul's Java (android/package/src) is compiled against android-36 with a
# stand-in R, the resources' ids the Java names (made here from its sources:
# javac needs them, the checks never read them); then each check runs with
# org.json's own jar first on the class path, since android.jar's org.json is
# a stub that throws. The checks hold the pure parts, as Rust's tests do:
# - DecideCheck: Calls.decideFor, on the cases of calls.rs's
#   who_rings_when_and_what_always_rings;
# - TableCheck: the calls' table as Rust writes it (the matrix of what reaches
#   you), read by Calls.Table.of, then decided;
# - KeyCheck: Calls.key against phones::key, on the Rust test's own numbers;
# - LogCheck: the log of calls screened (Calls.line, Calls.append), as Rust
#   reads it; with SIOUL_CALLS_SAMPLE, its lines are written there for
#   calls::tests::java_s_lines_read_here (tools/final-pass.sh does both);
# - ModeCheck: Sioul's modes (PauseMode's policy from Rust's ask, which modes
#   let a channel through), an event's own alarm (EventAlarms.isAlarm), a
#   conversation's key against Rust's (AppNotes.talkKey), the conversations
#   forgotten, Android's pages for a conversation;
# - HeardCheck: the phone's own do-not-disturb heard both ways
#   (DndHeard.decide), a mode's kind, a mode the phone holds;
# - CardCheck: the full card on the home screen (HomeCardToday.Lines.of):
#   today's date and weather at a time, the Porch's lines, Now's, when it
#   draws again, a card of version 2, an older Java meeting version 3; with
#   SIOUL_CARD_SAMPLE, the card Rust's homecard::tests::the_file_java_reads
#   wrote there.
# - MessagesCheck: what the listener says for your computers
#   (PhoneMessages.notice, PhoneMessages.message), under the names
#   phonemsgs.rs's `Extra` reads: visibility, times, pictures by their type.
# - TextsCheck (texts): the texts read from the provider and a part's result
#   (Texts.sms, Texts.mms, TextSend.line) on stand-in rows, as texts.rs reads them.
# - AttachmentsCheck: a mail attachment handed to another app (Attachments:
#   its content:// address and the way back, its type, ACTION_VIEW read
#   only, the question where to save), and the provider's folders and the
#   manifest, read from the repository (its root as the property sioul.root).
# - ChannelsCheck: no badge on Sioul's icon: every notification channel is
#   made by Channels.quiet, its badge off, and none elsewhere (read from the
#   sources, as android.jar's NotificationChannel cannot be made here).
# Then javadoc as the website's strict build runs it (website/build.sh
# --strict --api): the comments' HTML and references, every warning an error.
#
# Needs a JDK 17 or newer, python3, and Android's android.jar (the SDK's
# platforms;android-36: ANDROID_JAR names it, else ANDROID_HOME,
# ANDROID_SDK_ROOT, then ~/Android/Sdk, as website/build.sh looks). The first
# run needs the network: org.json 20240303 is fetched from Maven Central into
# ~/.cache/sioul-jvm-checks/ and used only when its SHA-256 is the one below;
# it is never kept in the repository. Everything else is made in a temporary
# folder, removed at the end. Exits with 1 when a check fails, with 77 when
# the JDK or android.jar is missing (tools/final-pass.sh says "skipped").
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
src="$root/android/package/src"
checks=(DecideCheck TableCheck KeyCheck LogCheck ModeCheck HeardCheck CardCheck MessagesCheck TextsCheck AttachmentsCheck ChannelsCheck)
json_url=https://repo1.maven.org/maven2/org/json/json/20240303/json-20240303.jar
json_sha256=3cf6cd6892e32e2b4c1c39e0f52f5248a2f5b37646fdfbb79a66b46b618414ed

javadoc=yes
case "${1:-}" in
    "") ;;
    --no-javadoc) javadoc=no ;;
    *) echo "usage: android/jvm-checks/run.sh [--no-javadoc]" >&2; exit 2 ;;
esac

for tool in javac java python3; do
    if ! command -v "$tool" > /dev/null; then
        echo "jvm-checks: no $tool (a JDK 17 or newer, and python3)." >&2
        exit 77
    fi
done

android_jar=${ANDROID_JAR:-}
if [[ -z "$android_jar" ]]; then
    for sdk in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Android/Sdk"; do
        if [[ -n "$sdk" && -f "$sdk/platforms/android-36/android.jar" ]]; then
            android_jar="$sdk/platforms/android-36/android.jar"
            break
        fi
    done
fi
if [[ -z "$android_jar" || ! -f "$android_jar" ]]; then
    echo "jvm-checks: no android.jar (the Android SDK's platforms;android-36; ANDROID_JAR may name it)." >&2
    exit 77
fi

sha256() {
    python3 -c 'import hashlib, sys; print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())' "$1"
}

# org.json's own jar: kept between runs, checked at each.
cache="${XDG_CACHE_HOME:-$HOME/.cache}/sioul-jvm-checks"
json_jar="$cache/json-20240303.jar"
if [[ -f "$json_jar" && "$(sha256 "$json_jar")" != "$json_sha256" ]]; then
    echo "jvm-checks: $json_jar is not the jar expected: fetched again." >&2
    rm -f "$json_jar"
fi
if [[ ! -f "$json_jar" ]]; then
    mkdir -p "$cache"
    echo "jvm-checks: fetching org.json 20240303 from Maven Central"
    python3 - "$json_url" "$json_jar.part" <<'PY'
import sys
import urllib.request

with urllib.request.urlopen(sys.argv[1], timeout=60) as answer, open(sys.argv[2], "wb") as out:
    out.write(answer.read())
PY
    if [[ "$(sha256 "$json_jar.part")" != "$json_sha256" ]]; then
        rm -f "$json_jar.part"
        echo "jvm-checks: the jar fetched from $json_url is not the one expected (SHA-256): nothing run." >&2
        exit 1
    fi
    mv "$json_jar.part" "$json_jar"
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/sioul-jvm-checks.XXXXXX")
trap 'rm -rf "$work"' EXIT

# The stand-in R: every R.<kind>.<name> the Java names (not android.R's), numbered.
python3 - "$src/com/aurelienpierre/sioul" "$work/R.java" <<'PY'
import pathlib
import re
import sys

found = {}
for path in sorted(pathlib.Path(sys.argv[1]).glob("*.java")):
    for kind, name in re.findall(r"(?<![\w.])R\.([a-z]+)\.(\w+)", path.read_text(encoding="utf-8")):
        found.setdefault(kind, set()).add(name)
lines = ["package com.aurelienpierre.sioul;", "/** The resources' ids Sioul's Java names, for javac only (android/jvm-checks/run.sh). */", "public final class R {"]
number = 0
for kind in sorted(found):
    names = []
    for name in sorted(found[kind]):
        number += 1
        names.append(f"{name} = {number}")
    lines.append(f"  public static final class {kind} {{ public static final int {', '.join(names)}; }}")
lines.append("}")
pathlib.Path(sys.argv[2]).write_text("\n".join(lines) + "\n", encoding="utf-8")
PY

echo "jvm-checks: compiling Sioul's Java and the checks against $(basename "$(dirname "$android_jar")")"
sources=("$work/R.java" "$src"/com/aurelienpierre/sioul/*.java)
for check in "${checks[@]}"; do
    sources+=("$here/$check.java")
done
if ! javac --release 17 -Xlint:all,-options -encoding UTF-8 -cp "$android_jar" -d "$work/out" "${sources[@]}" > "$work/javac.log" 2>&1; then
    sed "s|^|  |" "$work/javac.log"
    echo "jvm-checks: javac failed (above)." >&2
    exit 1
fi
sed "s|^|  |" "$work/javac.log"

failed=()
for check in "${checks[@]}"; do
    if ! java -Dsioul.root="$root" -cp "$json_jar:$work/out:$android_jar" "com.aurelienpierre.sioul.$check" > "$work/$check.log" 2>&1; then
        failed+=("$check")
    fi
    sed -e "s|^$check: ||" -e "s|^|  $check: |" "$work/$check.log"
done

if [[ "$javadoc" == yes ]]; then
    if ! javadoc -private -encoding UTF-8 -docencoding UTF-8 -charset UTF-8 -notimestamp -quiet \
            -sourcepath "$src" -d "$work/javadoc" --class-path "$android_jar" -Xdoclint:all,-missing -Werror \
            com.aurelienpierre.sioul > "$work/javadoc.log" 2>&1; then
        failed+=(javadoc)
    fi
    echo "  javadoc, strict: $([[ " ${failed[*]} " == *" javadoc "* ]] && echo "failed" || echo "no warning")"
    sed "s|^|  javadoc: |" "$work/javadoc.log"
fi

if [[ ${#failed[@]} -gt 0 ]]; then
    echo "jvm-checks: failed: ${failed[*]}"
    exit 1
fi
echo "jvm-checks: all ${#checks[@]} checks passed$([[ "$javadoc" == yes ]] && echo ", and javadoc")"
