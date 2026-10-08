#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# One of Sioul's notifications, end to end on a computer: the real window,
# offscreen, against a stand-in for the desktop's notification server that
# logs what Sioul asks and presses its buttons as Plasma does
# (stand_in_server.py, beside this file). The scenarios follow the time
# running's notification (crates/sioul-app/src/timenote.rs), on a timer
# started five minutes ago on a task, "Write to the bank":
#
#     tools/e2e/notifications/run.sh [--debug] buttons|quit|elsewhere [FOLDER]
#
# - buttons: the server presses Pause, Go on, then Stop: each press is
#   answered by a notification in the place of the last; Stop closes it, the
#   timer ends and its session is kept;
# - quit: the window goes through the phone's step list (SIOUL_GRAB_STEPS=phone)
#   and closes: the notification is closed with it, and the timer still runs;
# - elsewhere: `sioul focus stop` on the command line 6 s in: at its next
#   minute the window finds the timer gone and closes the notification.
# Then it prints the server's log, the timer's files and the window's exit,
# and says PASS or FAIL, with each expectation. Exits with 1 on FAIL.
#
# FOLDER (default: a new folder under ~/.cache/sioul-e2e/) receives the
# profile (config, data, state, cache, notes: emptied first, nothing else in
# FOLDER touched) and out/: the server's log (server.log, a JSON line per
# call), the window's (app.log), its exit (app-exit), the command line's
# (cli.log). It runs target/release/sioul-app and target/release/sioul (cargo
# build --release -p sioul-app -p sioul-cli), or with --debug target/debug/'s.
#
# Isolated as tools/demo/run.sh is: bubblewrap (no bwrap, no run), the real
# home out of reach (a tmpfs on /home, the profile as the folders of a user
# "e2e"), no network, SIOUL_DEMO=1, no display, a D-Bus session of its own on
# which the stand-in takes org.freedesktop.Notifications, a runtime folder of
# its own (short: a socket's path holds 108 bytes), the session's hidden.
# Needs python3 with PyGObject (Fedora: python3-gobject).
set -uo pipefail

usage="usage: tools/e2e/notifications/run.sh [--debug] buttons|quit|elsewhere [FOLDER]"
build=release
if [[ "${1:-}" == "--debug" ]]; then
    build=debug
    shift
fi
scenario=${1:-}
case "$scenario" in
    buttons|quit|elsewhere) ;;
    *) echo "$usage" >&2; exit 2 ;;
esac
[[ $# -le 2 ]] || { echo "$usage" >&2; exit 2; }

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
app="$repo/target/$build/sioul-app"
cli="$repo/target/$build/sioul"
if ! command -v bwrap > /dev/null; then
    echo "run.sh: bubblewrap (bwrap) is needed: the window runs only in its sandbox." >&2
    exit 1
fi
for program in "$app" "$cli"; do
    if [[ ! -x "$program" ]]; then
        echo "run.sh: no $program: build it first (CARGO_INCREMENTAL=0 cargo build$([[ $build == release ]] && echo " --release") -p sioul-app -p sioul-cli)." >&2
        exit 1
    fi
done
if ! python3 -c 'import gi; gi.require_version("Gio", "2.0"); from gi.repository import Gio' 2> /dev/null; then
    echo "run.sh: the stand-in server needs python3's PyGObject (Fedora: python3-gobject; Debian: python3-gi)." >&2
    exit 1
fi

folder=${2:-${XDG_CACHE_HOME:-$HOME/.cache}/sioul-e2e/$scenario-$(date +%Y%m%d-%H%M%S)}
mkdir -p "$folder" || exit 1
folder=$(cd "$folder" && pwd)
rm -rf "$folder"/{config,data,state,cache,notes,out}
mkdir -p "$folder"/{config,data,state,cache,notes,out/grab}
home=/home/e2e

# The task and its timer, started five minutes ago, 25 minutes chosen.
list="$folder/data/sioul/calendars/local/plan"
mkdir -p "$list" "$folder/data/sioul/time"
echo Plan > "$list/displayname"
cat > "$list/e2e-task-1.ics" <<'ICS'
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//sioul e2e//EN
BEGIN:VTODO
UID:e2e-task-1
DTSTAMP:20261005T120000Z
SUMMARY:Write to the bank
STATUS:IN-PROCESS
END:VTODO
END:VCALENDAR
ICS
start=$(( $(date +%s) - 300 ))
printf 'task = "e2e-task-1"\nstart = %d\nplanned = 25\n' "$start" > "$folder/data/sioul/time/running.toml"
echo "$start" > "$folder/out/start"

case $scenario in
    buttons) presses=pause,resume,stop; extra=(); seconds=40; elsewhere=":" ;;
    quit) presses=,,; extra=(SIOUL_GRAB="$folder/out/grab" SIOUL_GRAB_STEPS=phone); seconds=75; elsewhere=":" ;;
    elsewhere) presses=,,; extra=(); seconds=100
        elsewhere="sleep 6; '$cli' focus stop > '$folder/out/cli.log' 2>&1; echo \"cli exit \$?\" >> '$folder/out/cli.log'" ;;
esac
# A debug build starts slower.
[[ $build == debug ]] && seconds=$((seconds + 30))

runtime=$(mktemp -d /tmp/sioul-run.XXXXXX)
trap 'rm -rf "$runtime"' EXIT
hide=()
if [[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]]; then
    hide=(--tmpfs "$XDG_RUNTIME_DIR")
fi
echo "run.sh: $scenario, $build build, at most $seconds s, into $folder"
env -u DISPLAY -u WAYLAND_DISPLAY -u SESSION_MANAGER -u DBUS_SESSION_BUS_ADDRESS \
    bwrap --die-with-parent --unshare-net --dev-bind / / --tmpfs /home "${hide[@]}" --ro-bind "$repo" "$repo" \
    --bind "$folder/config" "$home/.config" --bind "$folder/data" "$home/.local/share" \
    --bind "$folder/state" "$home/.local/state" --bind "$folder/cache" "$home/.cache" \
    --bind "$folder/notes" "$home/Notes" --bind "$folder" "$folder" --bind "$runtime" "$runtime" --chdir "$home" \
    env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        XDG_STATE_HOME="$home/.local/state" XDG_CACHE_HOME="$home/.cache" XDG_RUNTIME_DIR="$runtime" \
        SIOUL_DEMO=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_FORCE_STDERR_LOGGING=1 \
        LANG=en_GB.UTF-8 LC_ALL=en_GB.UTF-8 "${extra[@]}" \
        timeout "$seconds" dbus-run-session -- bash -c "python3 '$here/stand_in_server.py' '$folder/out/server.log' '$presses' & sleep 1; ($elsewhere) & '$app'; echo \"app exit \$?\" >> '$folder/out/app-exit'; sleep 4" \
    > "$folder/out/app.log" 2>&1

echo "== the server's log"; cat "$folder/out/server.log" 2> /dev/null
echo "== running.toml"; cat "$folder/data/sioul/time/running.toml" 2> /dev/null || echo "(gone)"
echo "== sessions"; cat "$folder"/data/sioul/time/20*.toml 2> /dev/null || echo "(none)"
echo "== the window"; cat "$folder/out/app-exit" 2> /dev/null || echo "(stopped at $seconds s)"
[[ -f "$folder/out/cli.log" ]] && { echo "== the command line"; cat "$folder/out/cli.log"; }

python3 - "$folder" "$scenario" <<'PY'
import json
import pathlib
import sys

folder, scenario = pathlib.Path(sys.argv[1]), sys.argv[2]
log = folder / "out" / "server.log"
entries = [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines() if line.strip()] if log.exists() else []
notes = [e for e in entries if "notify" in e]
pressed = [e["pressed"] for e in entries if "pressed" in e]
closed = [e["close"] for e in entries if "close" in e]
time = folder / "data" / "sioul" / "time"
running = (time / "running.toml").exists()
sessions = sorted(time.glob("20*.toml"))
exit_line = (folder / "out" / "app-exit").read_text(encoding="utf-8").strip() if (folder / "out" / "app-exit").exists() else ""
cli = (folder / "out" / "cli.log").read_text(encoding="utf-8") if (folder / "out" / "cli.log").exists() else ""

expected = [("a notification shown, on the task", bool(notes) and "Write to the bank" in notes[0]["summary"])]
if scenario == "buttons":
    expected += [
        ("Pause, Go on and Stop pressed", pressed == ["pause", "resume", "stop"]),
        ("each press answered in the first one's place", len(notes) >= 3 and all(n["replaces"] == notes[0]["id"] for n in notes[1:])),
        ("closed after Stop", bool(closed)),
        ("the timer ended", not running),
        ("its session kept", len(sessions) == 1),
    ]
elif scenario == "quit":
    expected += [
        ("the window ended by itself", exit_line == "app exit 0"),
        ("the notification closed with it", bool(closed)),
        ("the timer still runs", running),
    ]
else:
    expected += [
        ("stopped on the command line", "cli exit 0" in cli),
        ("closed at the window's minute", bool(closed)),
        ("the timer ended", not running),
        ("its session kept", len(sessions) == 1),
    ]
for what, ok in expected:
    print(("  ok    " if ok else "  NOT   ") + what)
good = all(ok for _, ok in expected)
print(f"{scenario}: {'PASS' if good else 'FAIL'}")
sys.exit(0 if good else 1)
PY
