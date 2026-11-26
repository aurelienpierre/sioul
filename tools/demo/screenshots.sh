#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# The documentation's screenshots, on the demo profile (make-demo.py):
#
#     tools/demo/screenshots.sh [--language en|fr] [SCRATCH]
#
# In English, into website/docs/assets/screens/; with --language fr, the same
# life in French (make-demo.py --language fr) and Sioul in French, into
# website/docs/assets/screens/fr/, under the same names.
#
# Every picture the guide shows, in one go (some fifteen minutes): one run of
# Sioul for each of the step lists they come from (`runs`, below; main.qml's
# `grabber.steps`), each on a demo profile of its own made anew in SCRATCH (a
# new temporary folder by default), off the network (SIOUL_DEMO=1), in the
# light theme, at the size its steps give the window (1280×860 for the
# places, 1400 or 1100 by 1500 for long settings, a phone's 412×891); then the
# pictures the guide uses are copied into website/docs/assets/screens/, under
# the guide's names. SHOTS_RUNS="attention spam" takes those runs alone.
# Build first (CARGO_INCREMENTAL=0 cargo build --release -p sioul-app); the
# program is CARGO_TARGET_DIR/release/sioul-app (default target/).
#
# The pictures show a weekday afternoon, whatever the day they are taken: Sioul
# runs in a time zone of its own (TZ=DEMO-hh, a fixed offset) where it is
# 14:xx on the nearest weekday, and the profile is made for that moment. Sioul
# reads the time zone as the system does; nothing else changes. From Saturday
# 16:00 to Sunday 10:00 (UTC) no weekday is close enough: it says so.
#
# It runs in run.sh's sandbox, bubblewrap's (no bwrap, no pictures): the
# profile mounted as the home of a user "demo", so the pictures show
# /home/demo/… rather than SCRATCH, and the real home out of its reach; no
# network (a namespace of its own, and SIOUL_DEMO=1); no display; a D-Bus
# session of its own standing in for the system's too, and a runtime folder
# of its own (the session's own runtime folder and /run/dbus hidden), so that
# the English and the French pictures can be taken side by side.

set -euo pipefail

language=en
if [[ "${1:-}" == "--language" ]]; then
    language=${2:-}
    shift 2
fi
case "$language" in
    en) locale=en_US.UTF-8 suffix="" ;;
    fr) locale=fr_FR.UTF-8 suffix="-fr" ;;
    *) echo "--language: en or fr" >&2; exit 2 ;;
esac

repo=$(cd "$(dirname "$0")/../.." && pwd)
target="${CARGO_TARGET_DIR:-$repo/target}"
app="$target/release/sioul-app"
[[ -x "$app" ]] || app="$target/debug/sioul-app"
screens="$repo/website/docs/assets/screens${suffix:+/$language}"
scratch=${1:-$(mktemp -d -t sioul-demo.XXXXXX)}
mkdir -p "$scratch"
scratch=$(cd "$scratch" && pwd)

if [[ ! -x "$app" ]]; then
    echo "No $app: build it first (CARGO_INCREMENTAL=0 cargo build --release -p sioul-app)." >&2
    exit 1
fi

# The moment the pictures show: a weekday at 14:xx (else another hour of
# work), in a zone a whole number of hours from UTC (moment.py, which
# run.sh shares).
read -r now zone < <(python3 "$repo/tools/demo/moment.py")
if [[ "$now" == none ]]; then
    echo "No weekday is within a day of now (Saturday 16:00 to Sunday 10:00, UTC): try again later." >&2
    exit 1
fi
echo "Pictures of $now (TZ=$zone), in $language, in $scratch"

if ! command -v bwrap > /dev/null; then
    echo "bubblewrap (bwrap) is needed: Sioul runs only in its sandbox (Fedora, Debian, Arch: bubblewrap)." >&2
    exit 1
fi
home=/home/demo

# The fonts Plasma users see, Noto Sans and Hack, whatever this system's
# default (fonts.conf, which run.sh shares).
fonts="$repo/tools/demo/fonts.conf"
if command -v fc-match > /dev/null && ! FONTCONFIG_FILE="$fonts" fc-match "Sans Serif" | grep -q "Noto Sans"; then
    echo "Noto Sans is not installed: the pictures take this system's sans-serif font." >&2
fi

# The session's runtime folder (/run/user/<uid>) hidden: its D-Bus, Wayland and
# audio sockets; and the system's bus (/run/dbus): the sandbox's own session
# bus stands in for it.
hide=()
if [[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" ]]; then
    hide=(--tmpfs "$XDG_RUNTIME_DIR")
fi
if [[ -d /run/dbus ]]; then
    hide+=(--tmpfs /run/dbus)
fi

# Runs Sioul on profile $1 through step list $3 (SIOUL_GRAB_STEPS), its
# pictures into $2; "phone" as $4 gives the window a phone's size and layout.
run() {
    local profile=$1 out=$2 steps=$3 phone=${4:-}
    rm -rf "$out"
    mkdir -p "$out" "$profile/documents"
    # A runtime folder of its own: Sioul keeps one window per profile through a
    # socket there, and every demo profile is the same /home/demo. In SCRATCH,
    # unless its path is too long for a socket's (108 bytes at most).
    local runtime
    if (( ${#scratch} <= 60 )); then
        runtime=$(mktemp -d "$scratch/run.XXXXXX")
    else
        runtime=$(mktemp -d /tmp/sioul-run.XXXXXX)
    fi
    # COMPUTERNAME: the system's name for this computer, which Sioul reads before /etc/hostname
    # (sioul-sync's `lease::host_name`): an invented one, not the machine's.
    local env=(XDG_RUNTIME_DIR="$runtime" SIOUL_DEMO=1 SIOUL_GRAB="$out" SIOUL_GRAB_STEPS="$steps" SIOUL_TEST_PASSWORD=x SIOUL_THEME=light
               FONTCONFIG_FILE="$fonts" QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
               LANG="$locale" LC_ALL="$locale" TZ="$zone" COMPUTERNAME=noa-laptop)
    if [[ -n "$phone" ]]; then
        env+=(SIOUL_GRAB_PHONE=1)
    fi
    env -u DISPLAY -u WAYLAND_DISPLAY -u SESSION_MANAGER -u DBUS_SESSION_BUS_ADDRESS \
        bwrap --die-with-parent --unshare-net --dev-bind / / --tmpfs /home "${hide[@]}" --ro-bind "$repo" "$repo" --ro-bind "$app" "$app" \
        --bind "$profile/config" "$home/.config" --bind "$profile/data" "$home/.local/share" \
        --bind "$profile/state" "$home/.local/state" --bind "$profile/cache" "$home/.cache" \
        --bind "$profile/notes" "$home/Notes" --bind "$profile/documents" "$home/Documents" --bind "$out" "$out" \
        --bind "$runtime" "$runtime" --chdir "$home" \
        env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
            XDG_STATE_HOME="$home/.local/state" XDG_CACHE_HOME="$home/.cache" "${env[@]}" \
            timeout 400 dbus-run-session -- sh -c 'DBUS_SYSTEM_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" exec "$1"' sh "$app" > "$out/log" 2>&1 || true
    rm -rf "$runtime"
    grep -E "qml:|TypeError|ReferenceError" "$out/log" | grep -vE "sioul-(perf|objects):" | sed "s|^|  $out/log: |" | head -20 || true
}

# The runs, one line each: its name, make-demo.py's options (- for none), the
# step list (main.qml's `grabber.steps`), "phone" or "-", then the pictures
# it gives the guide, as PICTURE (its name in the guide too) or
# TAKEN:PICTURE. Each run has a profile of its own, made anew: some steps
# change it (an invoice issued, a safe sender added).
runs=(
    # A weekday afternoon, with hours.
    "demo - demo - porch mail mail-reader tasks-now tasks-day tasks-list agenda-week contacts notes projects time papers health sites sites-menu accounts settings-tabs settings-devices settings-hours reachable"
    # The Porch of the first days, its two cards not answered yet: joining your other devices,
    # the topics Sioul started.
    "first-day --first-day demo - porch:porch-first-day"
    # Without hours: the Porch asking for them, and the budgets of every area
    # at once (with hours, only those of the hours now).
    "no-hours --no-hours demo - porch-hours budgets bank-accounts"
    # Settings ▸ What reaches you, at 1400 by 1500, and a contact's sheet.
    "attention - attention - attention-rules:settings-attention attention-exception attention-own-times attention-dnd attention-circles attention-table attention-decided attention-sheet"
    # The status line's card of what reaches you, on a phone.
    "reach - reach phone reach-card reach-card-lengths"
    # Mail: a search by conditions, a selection held over a folder; the filters in Mail's ⚙.
    "mail-search - mail-search - mail-search mail-search-drag"
    "mail-filters - mail-filters - mail-filters mail-filter-editor mail-filters-run"
    # Sioul's own spam filter: its settings in Mail's ⚙, the Porch's review queue.
    "spam --spam spam - spam-settings-panel:settings-spam spam-reader:porch-spam"
    # The window that says what moves when Sioul's lists leave the notes folder (a profile of the old layout).
    "storage --old-layout storage - storage-move"
    # Settings on a phone: its list of sections.
    "settings-phone - shell phone settings-list:settings-phone"
    # Invoices, on a computer and on a phone.
    "invoices - invoices - project-client invoice-editor invoice-preview invoice-issued invoice-paid invoices settings-invoices"
    "invoices-phone - invoices phone invoices:invoices-phone invoice-editor:invoice-editor-phone invoice-issued:invoice-issued-phone"
)

mkdir -p "$screens"
missing=0
for line in "${runs[@]}"; do
    read -r name options steps phone pictures <<< "$line"
    if [[ -n "${SHOTS_RUNS:-}" && " $SHOTS_RUNS " != *" $name "* ]]; then
        continue
    fi
    [[ "$options" == - ]] && options=""
    [[ "$phone" == - ]] && phone=""
    profile="$scratch/profile-$name$suffix"
    shots="$scratch/shots-$name$suffix"
    # shellcheck disable=SC2086
    python3 "$repo/tools/demo/make-demo.py" --into "$profile" --now "$now" --language "$language" --notes-at "$home/Notes" $options > /dev/null
    echo "$name: $steps${phone:+ (phone)}"
    run "$profile" "$shots" "$steps" "$phone"
    for picture in $pictures; do
        taken=${picture%%:*}
        to=${picture##*:}
        if [[ -f "$shots/$taken.png" ]]; then
            cp "$shots/$taken.png" "$screens/$to.png"
        else
            echo "Missing: $shots/$taken.png (for $to.png)" >&2
            missing=1
        fi
    done
done
echo "Copied into $screens"
exit $missing
