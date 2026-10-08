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

failed=()
for name in "${names[@]}"; do
    if [[ ! -f "$tests/$name" ]]; then
        echo "qml-test.sh: no $name in crates/sioul-app/tests/qml/." >&2
        failed+=("$name")
        continue
    fi
    echo "== $name"
    if ! env -u DISPLAY -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_FORCE_STDERR_LOGGING=1 \
            TZ=Europe/Paris "$runner" -input "$tests/$name" "$@"; then
        failed+=("$name")
    fi
done

files=$([[ ${#names[@]} -eq 1 ]] && echo "1 file" || echo "${#names[@]} files")
if [[ ${#failed[@]} -gt 0 ]]; then
    echo "QML tests: ${#failed[@]} of $files failed: ${failed[*]}"
    exit 1
fi
echo "QML tests: $files, all passed"
