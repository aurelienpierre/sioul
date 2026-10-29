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
#   calls::tests::java_s_lines_read_here (tools/final-pass.sh does both); the
#   numbers you called, kept 14 days at most and only while the call
#   screening counts them (Calls.rememberCalled, Calls.calledAt);
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
# - TextsCheck (texts): the texts read from the provider, a part's result and
#   a long text's parts still on their way (Texts.sms, Texts.mms, TextSend.line,
#   Texts.segment) on stand-in rows, as texts.rs reads them.
# - AttachmentsCheck: a mail attachment handed to another app (Attachments:
#   its content:// address and the way back, its type, ACTION_VIEW read
#   only, the question where to save), and the provider's folders and the
#   manifest, read from the repository (its root as the property sioul.root).
# - EventNotesCheck: every reminder in the Events channels is private, its
#   public version saying only that Sioul has a reminder (composed tasks §7.3);
# - ChannelsCheck: no badge on Sioul's icon: every notification channel is
#   made by Channels.quiet, its badge off, and none elsewhere (read from the
#   sources, as android.jar's NotificationChannel cannot be made here).
# - StepsCheck: the background service's steps asked for later
#   (StepService.Later.of: a folder's never in place of a message's), the
#   files that bring a step (StepService.othersFile), whether the texts'
#   import asks Android (Texts.changedSince), whether a message received
#   brings a step soon (Texts.grew), and how an inbox's newest id is asked
#   for (Texts.newestBy: the limit in the sort, else the plain sort).
# - DayNotesCheck: the end of work's notice and the night's (DayNotes.buttons:
#   each notice's buttons and what each opens, from the words Rust gives,
#   which eventalarms.rs's own test reads in this check), the presses kept
#   apart (DayNotes.requestCode), what a press may open (ReminderOpener.opens),
#   and no words kept for them (EventAlarms.isDay).
# - RepliesCheck: a reply from your computers (Replies.choose, refusal, live):
#   each app's Reply and Mark as read found by what they mean, on stand-ins
#   shaped as Signal, WhatsApp, Telegram (a bot's buttons), Element X, /e/OS's
#   Message (Delete, Block filed as Mute, Call) and K-9 Mail make theirs; a
#   Reply that cannot be filled, one that opens the app, one that asks for an
#   unlocked phone; swapped titles, another order, the car extender's and the
#   watch's; the notification by its name (Rust's `replies::notice`), gone, or
#   another profile's.
# - SecurityKeyCheck: your security key held against the phone, read from the
#   sources (android.jar's NFC classes are stubs): no tag filter in the
#   manifest, NFC not required; reader mode only in SecurityKeyLink, on while
#   a band asks, the key let go (ignored until it leaves the field) before it
#   ends; the screen kept from captures meanwhile; nothing of an APDU logged
#   in Java or in main.cpp's bridge, each command and answer wiped; plugged
#   in: no USB filter, the smart card interface claimed alone, Android asked
#   once for each plugging, the key powered off and given back.
# - DoseAlarmsCheck: what Android holds of the doses' alarms (DoseAlarms.Plan):
#   two processes handing in either order from one record, a dose asked again
#   at waking kept, a past one never armed left alone, a first hand arming again.
# - CcidCheck: Ccid.java (the key plugged in) against a stand-in reader: the
#   messages' bytes and sequence numbers, packets and the zero-length one,
#   answers across transfers, a key waiting for a touch (time extensions),
#   an answer to another message, a reader's failure, a key unplugged, the
#   messages wiped once sent.
# - MmsCheck, compiled and run apart: the multimedia messages Rust writes
#   (mms-samples.jsonl, which sioul-core's `mms` tests keep equal to what its
#   encoder writes now) read by Android's own parser, as Android's MMS service
#   reads the message an app hands it: AOSP's PduParser and the classes it
#   needs (aosp/, copied unchanged from platform/frameworks/base at commit
#   99b01a65cc4c104933788b3143285ab6bae65827, android16-release,
#   telephony/common/com/google/android/mms/; Apache-2.0, its NOTICE beside
#   them), with stand-ins for the four Android classes they name (stand-ins/:
#   android.jar's would throw). Never part of Sioul's program.
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
checks=(DecideCheck TableCheck KeyCheck LogCheck ModeCheck HeardCheck CardCheck MessagesCheck TextsCheck AttachmentsCheck ChannelsCheck StepsCheck DayNotesCheck RepliesCheck SecurityKeyCheck CcidCheck DoseAlarmsCheck EventNotesCheck)
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

# AOSP's MMS parser apart from Sioul's Java and android.jar, with its stand-ins; MmsCheck against it.
echo "jvm-checks: compiling AOSP's MMS parser apart, with its stand-ins"
aosp_sources=()
while IFS= read -r -d '' file; do
    aosp_sources+=("$file")
done < <(find "$here/aosp" "$here/stand-ins" -name '*.java' -print0 | sort -z)
if ! javac --release 17 -nowarn -Xlint:none -encoding UTF-8 -d "$work/aosp" "${aosp_sources[@]}" > "$work/javac-aosp.log" 2>&1; then
    sed "s|^|  |" "$work/javac-aosp.log"
    echo "jvm-checks: javac failed on AOSP's parser (above)." >&2
    exit 1
fi
if ! javac --release 17 -Xlint:all,-options -encoding UTF-8 -cp "$json_jar:$work/aosp" -d "$work/mms" "$here/MmsCheck.java" > "$work/javac-mms.log" 2>&1; then
    sed "s|^|  |" "$work/javac-mms.log"
    echo "jvm-checks: javac failed on MmsCheck (above)." >&2
    exit 1
fi
sed "s|^|  |" "$work/javac-mms.log"

failed=()
for check in "${checks[@]}"; do
    if ! java -Dsioul.root="$root" -cp "$json_jar:$work/out:$android_jar" "com.aurelienpierre.sioul.$check" > "$work/$check.log" 2>&1; then
        failed+=("$check")
    fi
    sed -e "s|^$check: ||" -e "s|^|  $check: |" "$work/$check.log"
done
if ! java -Dsioul.root="$root" -cp "$json_jar:$work/aosp:$work/mms" com.aurelienpierre.sioul.MmsCheck > "$work/MmsCheck.log" 2>&1; then
    failed+=(MmsCheck)
fi
sed -e "s|^MmsCheck: ||" -e "s|^|  MmsCheck: |" "$work/MmsCheck.log"

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
echo "jvm-checks: all $((${#checks[@]} + 1)) checks passed$([[ "$javadoc" == yes ]] && echo ", and javadoc")"
