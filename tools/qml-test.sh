#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# The window's QML tests, crates/sioul-app/tests/qml/tst_*.qml, offscreen, on
# stand-ins for Sioul: no Sioul started, no profile read, nothing built.
#
#     tools/qml-test.sh [tst_name.qml…] [-- qmltestrunner's own options]
#
# Without a name, every test of the folder, one file after the other. After
# --, Qt's own options: `-- DndSetup::test_the_list_names_each_person` runs
# one function, `-- -functions` lists them.
#
# Each test imports the window's QML folder by its relative path
# (`import "../../qml"`), as the window's own files see each other: nothing is
# copied, and no build is needed. The module the build generates would not do:
# its qmldir prefers the copies compiled into the program (`prefer :/qt/qml/…`).
#
# The window's module (com.aurelienpierre.sioul) is stood in for by
# tests/qml/stand-ins/: its C++ types that a tested file uses (TextSpacing,
# for TaskPanel.qml), as QML that does nothing.
#
# A test that names tools/qml-stand-in.py gets that stand-in web server on
# 127.0.0.1:18731 while it runs (tst_plaintext.qml asks it whether Qt
# fetched a picture); it fails at once when the port is taken.
#
# A test that names "qml-test: accessibility" runs with an assistive tool
# listening, as a screen reader would: offscreen has no accessibility at all,
# so it runs on a headless Weston (Wayland), where Qt's AT-SPI bridge is told
# to stay on (QT_LINUX_ACCESSIBILITY_ALWAYS_ON) and the address of a bus
# (AT_SPI_BUS_ADDRESS): a D-Bus session of its own, which starts nothing,
# in a runtime folder of its own; Weston reads no configuration. Needs weston.
#
# A test that names "qml-test: android" runs on the window's QML as the
# phone's build has it (crates/sioul-app/build.rs): the files for computers
# only left out (its `desktop_only`), each file of qml/android/ in place of
# its namesake, in a copy made for the run, the test file beside it so that
# its `import "../../qml"` finds that copy. A page that gives a property, or
# names a type, that the phone's files lack is refused there as on the phone,
# where the computer's files hide it (the Notes page, 3 to 26 November 2026).
#
# Qt 6's runner: Fedora names it qmltestrunner-qt6, and its bare qmltestrunner
# is Qt 5's, which cannot read these files ("Library import requires a
# version"). QMLTESTRUNNER names another. Its messages on the terminal (Fedora's Qt sends them to the
# journal), in a fixed time zone, Europe/Paris, which tst_now.qml reads its
# days in. Exits with 1 when a test failed or a file is missing.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
tests="$repo/crates/sioul-app/tests/qml"

runner=${QMLTESTRUNNER:-}
if [[ -z "$runner" ]]; then
    for candidate in qmltestrunner-qt6 qmltestrunner6 /usr/lib64/qt6/bin/qmltestrunner /usr/lib/qt6/bin/qmltestrunner \
                     /usr/lib/x86_64-linux-gnu/qt6/bin/qmltestrunner /usr/lib/aarch64-linux-gnu/qt6/bin/qmltestrunner; do
        if command -v "$candidate" > /dev/null; then
            runner=$(command -v "$candidate")
            break
        fi
    done
fi
if [[ -z "$runner" ]]; then
    echo "qml-test.sh: Qt 6's qmltestrunner not found (Fedora: qt6-qtdeclarative-devel; Debian: qml6-module-qttest); QMLTESTRUNNER may name it." >&2
    exit 1
fi

names=()
while [[ $# -gt 0 && "$1" != "--" ]]; do
    names+=("$(basename "$1")")
    shift
done
[[ "${1:-}" == "--" ]] && shift
if [[ ${#names[@]} -eq 0 ]]; then
    for file in "$tests"/tst_*.qml; do
        names+=("$(basename "$file")")
    done
fi

# The stand-in web server, for a test that names it: started, and stopped after.
stand_in_port=18731
stand_in=""
stop_stand_in() {
    if [[ -n "$stand_in" ]]; then
        kill "$stand_in" 2> /dev/null || true
        wait "$stand_in" 2> /dev/null || true
        stand_in=""
    fi
}
trap stop_stand_in EXIT
start_stand_in() {
    if (exec 3<> "/dev/tcp/127.0.0.1/$stand_in_port") 2> /dev/null; then
        echo "qml-test.sh: 127.0.0.1:$stand_in_port is taken; the stand-in web server needs it." >&2
        return 1
    fi
    python3 -I "$repo/tools/qml-stand-in.py" "$stand_in_port" &
    stand_in=$!
    for _ in $(seq 50); do
        (exec 3<> "/dev/tcp/127.0.0.1/$stand_in_port") 2> /dev/null && return 0
        sleep 0.1
    done
    echo "qml-test.sh: the stand-in web server did not start." >&2
    return 1
}

# One test file with an assistive tool listening (above); its runner's status.
with_accessibility() {
    local file=$1
    if ! command -v weston > /dev/null; then
        echo "qml-test.sh: $(basename "$file") needs weston (Fedora, Debian: weston): it runs with an assistive tool listening." >&2
        return 1
    fi
    local run status
    run=$(mktemp -d /tmp/sioul-qml-a11y.XXXXXX) || return 1
    chmod 700 "$run"
    cat > "$run/bus.conf" <<CONF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=$run</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
    env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR="$run" XDG_CONFIG_HOME="$run" \
        QT_FORCE_STDERR_LOGGING=1 TZ=Europe/Paris RUNNER="$runner" STAND_INS="$tests/stand-ins" FILE="$file" \
        dbus-run-session --config-file="$run/bus.conf" -- sh -c '
            weston --no-config --backend=headless --socket=qml-test --idle-time=0 > "$XDG_RUNTIME_DIR/weston.log" 2>&1 &
            weston=$!
            for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/qml-test" ] && break; sleep 0.1; done
            AT_SPI_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_LOGGING_RULES=qt.accessibility.atspi=false QT_QPA_PLATFORM=wayland \
                WAYLAND_DISPLAY=qml-test QT_QUICK_BACKEND=software "$RUNNER" -import "$STAND_INS" -input "$FILE" "$@"
            status=$?
            kill "$weston" 2> /dev/null
            exit $status' sh "${@:2}"
    status=$?
    rm -rf "$run"
    return $status
}

# One test file on the phone's set of files (above), offscreen; its runner's status.
on_phone_set() {
    local file=$1 run gone status
    gone=$(sed -n 's/^ *let desktop_only = \[\(.*\)\];$/\1/p' "$repo/crates/sioul-app/build.rs" | grep -o '"qml/[^"]*\.qml"' | tr -d '"' || true)
    if [[ -z "$gone" ]]; then
        echo "qml-test.sh: no one-line \`let desktop_only = [...];\` in crates/sioul-app/build.rs: the phone's set of files is not known." >&2
        return 1
    fi
    run=$(mktemp -d /tmp/sioul-qml-android.XXXXXX) || return 1
    mkdir -p "$run/tests/qml"
    cp -r "$repo/crates/sioul-app/qml" "$run/qml"
    rm -rf "$run/qml/android"
    for path in $gone; do
        rm -f "${run:?}/$path"
    done
    cp "$repo/crates/sioul-app/qml/android/"*.qml "$run/qml/"
    cp "$file" "$run/tests/qml/"
    status=0
    env -u DISPLAY -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_FORCE_STDERR_LOGGING=1 \
        TZ=Europe/Paris "$runner" -import "$tests/stand-ins" -input "$run/tests/qml/$(basename "$file")" "${@:2}" || status=$?
    rm -rf "$run"
    return $status
}

failed=()
for name in "${names[@]}"; do
    if [[ ! -f "$tests/$name" ]]; then
        echo "qml-test.sh: no $name in crates/sioul-app/tests/qml/." >&2
        failed+=("$name")
        continue
    fi
    echo "== $name"
    if grep -q "qml-stand-in.py" "$tests/$name" && ! start_stand_in; then
        failed+=("$name")
        continue
    fi
    if grep -q "qml-test: accessibility" "$tests/$name"; then
        with_accessibility "$tests/$name" "$@" || failed+=("$name")
    elif grep -q "qml-test: android" "$tests/$name"; then
        on_phone_set "$tests/$name" "$@" || failed+=("$name")
    elif ! env -u DISPLAY -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_FORCE_STDERR_LOGGING=1 \
            TZ=Europe/Paris "$runner" -import "$tests/stand-ins" -input "$tests/$name" "$@"; then
        failed+=("$name")
    fi
    stop_stand_in
done

files=$([[ ${#names[@]} -eq 1 ]] && echo "1 file" || echo "${#names[@]} files")
if [[ ${#failed[@]} -gt 0 ]]; then
    echo "QML tests: ${#failed[@]} of $files failed: ${failed[*]}"
    exit 1
fi
echo "QML tests: $files, all passed"
