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
# Makes the profile in SCRATCH (a new temporary folder by default), runs Sioul
# on it off the network (SIOUL_DEMO=1) taking its pictures (SIOUL_GRAB_STEPS=demo,
# main.qml), in the light theme, at 1280×860; then once more on the same
# profile without hours, for the Porch asking for them and the budgets of every
# area at once; and copies the pictures into website/docs/assets/screens/.
# Build first: CARGO_INCREMENTAL=0 cargo build -p sioul-app.
#
# The pictures show a weekday afternoon, whatever the day they are taken: Sioul
# runs in a time zone of its own (TZ=DEMO-hh, a fixed offset) where it is
# 14:xx on the nearest weekday, and the profile is made for that moment. Sioul
# reads the time zone as the system does; nothing else changes. From Saturday
# 16:00 to Sunday 10:00 (UTC) no weekday is close enough: it says so.
#
# With bubblewrap (bwrap), Sioul runs with the profile mounted as the home of a
# user "demo", so the pictures show /home/demo/… rather than SCRATCH, and the
# real home is out of its reach. Without it, it runs on SCRATCH directly.

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
app="$repo/target/debug/sioul-app"
screens="$repo/website/docs/assets/screens${suffix:+/$language}"
scratch=${1:-$(mktemp -d -t sioul-demo.XXXXXX)}
mkdir -p "$scratch"
scratch=$(cd "$scratch" && pwd)

if [[ ! -x "$app" ]]; then
    echo "No $app: build it first (CARGO_INCREMENTAL=0 cargo build -p sioul-app)." >&2
    exit 1
fi

# The moment the pictures show: a weekday at 14:xx (else another hour of
# work), in a zone a whole number of hours from UTC (the day view draws its
# hours on UTC's) and at most a day away; the minutes are the clock's.
read -r now zone < <(python3 - <<'EOF'
from datetime import datetime, timedelta, timezone
real = datetime.now(timezone.utc).replace(second=0, microsecond=0)
for hour in (14, 15, 10, 9, 11, 16):
    found = []
    for days in (-1, 0, 1):
        offset = hour - real.hour + 24 * days
        # A fixed offset stays within a day (Python's limit, and POSIX's).
        if abs(offset) <= 23 and (real + timedelta(hours=offset)).weekday() < 5:
            found.append(offset)
    if found:
        offset = min(found, key=abs)
        local = real.astimezone(timezone(timedelta(hours=offset)))
        # POSIX writes the offset the other way round: DEMO-02 is two hours east.
        print(local.isoformat(), f"DEMO{'-' if offset >= 0 else '+'}{abs(offset):02d}")
        break
else:
    print("none none")
EOF
)
if [[ "$now" == none ]]; then
    echo "No weekday is within a day of now (Saturday 16:00 to Sunday 10:00, UTC): try again later." >&2
    exit 1
fi
echo "Pictures of $now (TZ=$zone), in $language, in $scratch"

if command -v bwrap > /dev/null; then
    home=/home/demo
else
    home=""
fi

# The fonts Plasma users see, Noto Sans and Hack, whatever this system's
# default: Bitstream Vera Sans, for one, draws the no-break space of "620 €"
# twice as wide as a space. The system's configuration, then these first.
fonts="$scratch/fonts.conf"
cat > "$fonts" <<'EOF'
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <include ignore_missing="yes">/etc/fonts/fonts.conf</include>
  <match target="pattern">
    <test name="family" qual="any"><string>sans-serif</string></test>
    <edit name="family" mode="prepend" binding="strong"><string>Noto Sans</string></edit>
  </match>
  <match target="pattern">
    <test name="family" qual="any"><string>Sans Serif</string></test>
    <edit name="family" mode="prepend" binding="strong"><string>Noto Sans</string></edit>
  </match>
  <match target="pattern">
    <test name="family" qual="any"><string>monospace</string></test>
    <edit name="family" mode="prepend" binding="strong"><string>Hack</string></edit>
  </match>
</fontconfig>
EOF
if command -v fc-match > /dev/null && ! FONTCONFIG_FILE="$fonts" fc-match "Sans Serif" | grep -q "Noto Sans"; then
    echo "Noto Sans is not installed: the pictures take this system's sans-serif font." >&2
fi

# Runs Sioul on profile $1, its pictures into $2.
run() {
    local profile=$1 out=$2
    rm -rf "$out"
    mkdir -p "$out"
    local env=(SIOUL_DEMO=1 SIOUL_GRAB="$out" SIOUL_GRAB_STEPS=demo SIOUL_TEST_PASSWORD=x SIOUL_THEME=light FONTCONFIG_FILE="$fonts"
               QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software LANG="$locale" LC_ALL="$locale" TZ="$zone")
    # Nothing reaches the desktop: no display, and a bus of its own (dbus-run-session).
    if [[ -n "$home" ]]; then
        env -u DISPLAY -u WAYLAND_DISPLAY bwrap --dev-bind / / --tmpfs /home --ro-bind "$repo" "$repo" \
            --bind "$profile/config" "$home/.config" --bind "$profile/data" "$home/.local/share" \
            --bind "$profile/state" "$home/.local/state" --bind "$profile/cache" "$home/.cache" \
            --bind "$profile/notes" "$home/Notes" --bind "$out" "$out" --chdir "$home" \
            env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
                XDG_STATE_HOME="$home/.local/state" XDG_CACHE_HOME="$home/.cache" "${env[@]}" \
                timeout 400 dbus-run-session -- "$app" > "$out/log" 2>&1 || true
    else
        env -u DISPLAY -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$profile/config" XDG_DATA_HOME="$profile/data" XDG_STATE_HOME="$profile/state" \
            XDG_CACHE_HOME="$profile/cache" "${env[@]}" \
            timeout 400 dbus-run-session -- "$app" > "$out/log" 2>&1 || true
    fi
    grep -E "qml:|TypeError|ReferenceError" "$out/log" | sed "s|^|  $out/log: |" | head -20 || true
}

notes_at=()
[[ -n "$home" ]] && notes_at=(--notes-at "$home/Notes")

python3 "$repo/tools/demo/make-demo.py" --into "$scratch/profile$suffix" --now "$now" --language "$language" "${notes_at[@]}"
run "$scratch/profile$suffix" "$scratch/shots$suffix"
python3 "$repo/tools/demo/make-demo.py" --into "$scratch/profile-no-hours$suffix" --now "$now" --language "$language" --no-hours "${notes_at[@]}"
run "$scratch/profile-no-hours$suffix" "$scratch/shots-no-hours$suffix"

# With hours, a weekday afternoon; without them, what asks for them, and the
# budgets of every area at once (with hours, only those of the hours now).
names=(porch mail mail-reader tasks-now tasks-day tasks-list agenda-week contacts notes projects time
       papers health sites sites-menu accounts accounts-senders settings-hours work-now)
no_hours=(porch-hours budgets bank-accounts)
mkdir -p "$screens"
missing=0
copy() {
    if [[ -f "$1/$2.png" ]]; then
        cp "$1/$2.png" "$screens/$2.png"
    else
        echo "Missing: $1/$2.png" >&2
        missing=1
    fi
}
for name in "${names[@]}"; do
    copy "$scratch/shots$suffix" "$name"
done
for name in "${no_hours[@]}"; do
    copy "$scratch/shots-no-hours$suffix" "$name"
done
echo "Copied into $screens"
exit $missing
