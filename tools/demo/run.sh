#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Sioul's window on a fresh demo profile, in a sandbox, going through one of
# its step lists: the one way to start the window for a check.
#
#     tools/demo/run.sh [--release] OUT en|fr STEPS [phone]
#
# OUT receives the demo profile, made anew by make-demo.py (OUT/profile), and
# the pictures and the window's log (OUT/shots/, OUT/shots/log): those two are
# emptied first, nothing else in OUT is touched. STEPS names a step list of the window (SIOUL_GRAB_STEPS;
# main.qml's `grabber.steps`): "pages" and "demo" take a picture of each
# page, others act in the window, take their pictures and say what they did
# in the log. Which lists this build allows is `grab_steps` in
# crates/sioul-app/src/backend.rs; any other runs "pages". "phone" gives the
# window a phone's size and layout (SIOUL_GRAB_PHONE). --release runs
# target/release/sioul-app rather than target/debug/sioul-app; build it first.
# RUN_HOUR=19 shows 19:xx on a weekday rather than the afternoon (moment.py).
#
# The sandbox, bubblewrap's (no bwrap, no run):
# - the real home out of reach (a tmpfs on /home), the profile mounted as the
#   home of a user "demo", the repository read-only;
# - no network (a network namespace of its own), and SIOUL_DEMO=1, which keeps
#   Sioul from fetching or sending anything by itself;
# - no display (DISPLAY and WAYLAND_DISPLAY unset; offscreen, software
#   rendering), a D-Bus session of its own (dbus-run-session), a runtime
#   folder of its own (/tmp/sioul-run.*, removed after: a short path, since a
#   socket's path holds 108 bytes at most, and past them Qt's audio waited
#   half a minute at each step), and the session's own runtime folder hidden
#   (its bus, its Wayland and audio sockets);
# - the profile's fonts, Noto Sans and Hack (fonts.conf), the light theme, a
#   test password standing in for the keyring;
# - 400 seconds at most.
# Afterwards it prints the log's lines from QML and its errors, and the
# pictures taken. Exits with 1 when the window ended badly (a crash, or
# stopped at 400 seconds) or its log holds a script error (TypeError,
# ReferenceError).
set -euo pipefail

usage="usage: tools/demo/run.sh [--release] OUT en|fr STEPS [phone]"
build=debug
if [[ "${1:-}" == "--release" ]]; then
    build=release
    shift
fi
if [[ $# -lt 3 || $# -gt 4 || ( $# -eq 4 && "$4" != "phone" ) ]]; then
    echo "$usage" >&2
    exit 2
fi
out=$1 language=$2 steps=$3 phone=${4:-}
case "$language" in
    en) locale=en_US.UTF-8 ;;
    fr) locale=fr_FR.UTF-8 ;;
    *) echo "$usage" >&2; exit 2 ;;
esac

repo=$(cd "$(dirname "$0")/../.." && pwd)
app="$repo/target/$build/sioul-app"
if ! command -v bwrap > /dev/null; then
    echo "run.sh: bubblewrap (bwrap) is needed: the window runs only in its sandbox (Fedora, Debian, Arch: bubblewrap)." >&2
    exit 1
fi
if [[ ! -x "$app" ]]; then
    echo "run.sh: no $app: build it first (CARGO_INCREMENTAL=0 cargo build$([[ $build == release ]] && echo " --release") -p sioul-app)." >&2
    exit 1
fi

read -r now zone < <(python3 "$repo/tools/demo/moment.py" ${RUN_HOUR:+--hour "$RUN_HOUR"})
if [[ "$now" == none ]]; then
    echo "run.sh: no weekday is within a day of now at that hour (Saturday 16:00 to Sunday 10:00, UTC, for 14:00): try again later, or another RUN_HOUR." >&2
    exit 1
fi

mkdir -p "$out"
out=$(cd "$out" && pwd)
profile="$out/profile"
shots="$out/shots"
rm -rf "$profile" "$shots"
mkdir -p "$shots"
# A runtime folder of its own: Sioul keeps one window per profile through a
# socket there, and every demo profile is the same /home/demo, so runs side by
# side would hand over to each other and the second would take no pictures.
runtime=$(mktemp -d /tmp/sioul-run.XXXXXX)
trap 'rm -rf "$runtime"' EXIT
home=/home/demo
python3 "$repo/tools/demo/make-demo.py" --into "$profile" --now "$now" --language "$language" --notes-at "$home/Notes" > /dev/null

env=(XDG_RUNTIME_DIR="$runtime" SIOUL_DEMO=1 SIOUL_GRAB="$shots" SIOUL_GRAB_STEPS="$steps" SIOUL_TEST_PASSWORD=x SIOUL_THEME=light
     FONTCONFIG_FILE="$repo/tools/demo/fonts.conf" QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
     LANG="$locale" LC_ALL="$locale" TZ="$zone")
if [[ -n "$phone" ]]; then
    env+=(SIOUL_GRAB_PHONE=1)
fi
# The session's runtime folder (/run/user/<uid>) hidden: its D-Bus, Wayland and audio sockets.
hide=()
if [[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]]; then
    hide=(--tmpfs "$XDG_RUNTIME_DIR")
fi
echo "run.sh: $steps, in $language, at $now (TZ=$zone), $build build, into $out"
status=0
env -u DISPLAY -u WAYLAND_DISPLAY -u SESSION_MANAGER -u DBUS_SESSION_BUS_ADDRESS \
    bwrap --die-with-parent --unshare-net --dev-bind / / --tmpfs /home "${hide[@]}" --ro-bind "$repo" "$repo" \
    --bind "$profile/config" "$home/.config" --bind "$profile/data" "$home/.local/share" \
    --bind "$profile/state" "$home/.local/state" --bind "$profile/cache" "$home/.cache" \
    --bind "$profile/notes" "$home/Notes" --bind "$out" "$out" --bind "$runtime" "$runtime" --chdir "$home" \
    env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        XDG_STATE_HOME="$home/.local/state" XDG_CACHE_HOME="$home/.cache" "${env[@]}" \
        timeout 400 dbus-run-session -- "$app" > "$shots/log" 2>&1 || status=$?

grep -E "qml:|TypeError|ReferenceError|OVERFLOW|WIDE|rror" "$shots/log" | grep -v "sioul-perf" | head -40 || true
pictures=$(find "$shots" -name '*.png' | wc -l)
echo "run.sh: $pictures pictures in $shots, the log in $shots/log"
if [[ $status -eq 124 ]]; then
    echo "run.sh: the window was still open after 400 seconds: stopped." >&2
    exit 1
elif [[ $status -ne 0 ]]; then
    echo "run.sh: the window ended with status $status." >&2
    exit 1
fi
if grep -qE "TypeError|ReferenceError" "$shots/log"; then
    echo "run.sh: script errors in the log (above)." >&2
    exit 1
fi
