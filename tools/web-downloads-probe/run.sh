#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Sioul's downloads on real Qt WebEngine requests, run by hand (never in CI;
# docs/sites.md, "Downloads"): probe.qml, the project's own SiteDownloads.qml
# and DownloadCard.qml with a stand-in backend, offscreen, against a stand-in
# site on 127.0.0.1 (server.py), with watch.py logging every file made in the
# scratch home. Everything lives in a scratch folder: the home, its Downloads
# folder (where the engine makes its own hidden file), the XDG folders,
# TMPDIR, the held folders. No real site is reached; the real home is never
# touched. Sioul itself is not started.
#
#   tools/web-downloads-probe/run.sh held          # accepted at once: what shows in Downloads, and for how long
#   tools/web-downloads-probe/run.sh trickle       # the same, the file sent at a network's pace
#   tools/web-downloads-probe/run.sh view-cancel   # the page's view destroyed while the card waits, then Cancel
#   tools/web-downloads-probe/run.sh view-save     # … then Save
#   tools/web-downloads-probe/run.sh view-next     # two files, the view destroyed, Save on each
#   tools/web-downloads-probe/run.sh many          # thirty files offered by script, no click
#
# SCRATCH names the scratch folder (default ~/.cache/sioul-web-downloads-probe);
# QML_TOOL names Qt 6's `qml` (default: found where Fedora and Debian put it).
set -euo pipefail
scenario=${1:?a scenario: held, trickle, view-cancel, view-save, view-next, many}
here=$(cd "$(dirname "$0")" && pwd)
scratch=${SCRATCH:-$HOME/.cache/sioul-web-downloads-probe}
qml=${QML_TOOL:-}
if [[ -z "$qml" ]]; then
    for candidate in qml6 /usr/lib64/qt6/bin/qml /usr/lib/qt6/bin/qml /usr/lib/x86_64-linux-gnu/qt6/bin/qml; do
        if command -v "$candidate" > /dev/null; then
            qml=$(command -v "$candidate")
            break
        fi
    done
fi
[[ -n "$qml" ]] || { echo "run.sh: Qt 6's qml tool not found; QML_TOOL may name it." >&2; exit 1; }

run="$scratch/run-$scenario"
rm -rf "$run"
mkdir -p "$run/held" "$run/home/Downloads" "$run/xdg/data" "$run/xdg/cache" "$run/xdg/config" "$run/xdg/state" "$run/tmp"
chmod 700 "$run/held"
# The held folders the stand-in backend hands out, as Rust makes them: one per file, yours alone.
for n in $(seq 1 40); do
    mkdir -m 700 "$run/held/held-$n"
done
port=$((18000 + RANDOM % 1000))
python3 -I "$here/server.py" "$port" "$run/log.txt" &
server=$!
python3 -I "$here/watch.py" "$run/watch.txt" "$run/home" "$run/tmp" "$run/held" &
watcher=$!
trap 'kill "$server" "$watcher" 2> /dev/null || true' EXIT
sleep 0.7
status=0
env HOME="$run/home" TMPDIR="$run/tmp" XDG_DATA_HOME="$run/xdg/data" XDG_CACHE_HOME="$run/xdg/cache" XDG_CONFIG_HOME="$run/xdg/config" XDG_STATE_HOME="$run/xdg/state" \
    QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu" \
    timeout 40 "$qml" "$here/probe.qml" -- "$scenario" "http://127.0.0.1:$port" "$run/held" >> "$run/log.txt" 2>&1 || status=$?
sleep 0.3
echo "== $scenario: the program's exit: $status$([[ $status -eq 139 ]] && echo ' (a segmentation fault)')"
echo "== what the page and the stand-in backend said"
grep -E "^qml: probe|server: GET" "$run/log.txt" | grep -v favicon | sed "s|$run/||g; s|^qml: ||" | cut -c1-220
echo "== the engine's hidden files in the scratch Downloads folder, and how long each stayed there"
python3 -I - "$run/watch.txt" "$run/home/Downloads" <<'PY'
import re, sys
log, downloads = sys.argv[1], sys.argv[2]
made, lives, left = {}, [], []
for line in open(log):
    m = re.match(r"watch (\S+) (\S+) (.+?)( \d+ bytes)?$", line.rstrip("\n"))
    if not m or not m.group(3).startswith(downloads + "/"):
        continue
    at, kind, path = float(m.group(1)), m.group(2), m.group(3)
    size = int(m.group(4).split()[0]) if m.group(4) else None
    if kind == "create":
        made[path] = [at, size or 0]
    elif kind == "close-write" and path in made and size is not None:
        made[path][1] = max(made[path][1], size)
    elif kind in ("moved-from", "delete") and path in made:
        start, size = made.pop(path)
        lives.append((path[len(downloads) + 1:], (at - start) * 1000, size, kind))
for name, ms, size, how in lives:
    print("  %s: %.1f ms, %d bytes seen, then %s" % (name, ms, size, "moved out" if how == "moved-from" else "removed"))
for path in made:
    print("  %s: STILL THERE" % path[len(downloads) + 1:])
if not lives and not made:
    print("  none")
PY
echo "== files left in the scratch home and the held folders"
find "$run/home" "$run/held" -type f -printf '  %s bytes %m %P\n' | sort | head -40
