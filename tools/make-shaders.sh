#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# colour: bakes Sioul's shaders (crates/sioul-app/shaders/*.frag) into the
# .qsb files Qt Quick loads, each holding SPIR-V, GLSL (ES 1.00, 1.20, 1.50),
# HLSL 5.0 and MSL 1.2: one file for every system and graphics API. The .qsb
# files are kept in the repository, so that a build needs no shader tools; run
# this after changing a shader. Needs Qt's qsb (qt6-qtshadertools on Fedora,
# qt6-shader-baker on Debian and Ubuntu), found on PATH or beside qmake6.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
qsb="$(command -v qsb || true)"
if [ -z "$qsb" ]; then
    for candidate in "$(qmake6 -query QT_HOST_LIBEXECS 2>/dev/null)/qsb" "$(qmake6 -query QT_HOST_BINS 2>/dev/null)/qsb" /usr/lib64/qt6/bin/qsb /usr/lib/qt6/bin/qsb; do
        if [ -x "$candidate" ]; then
            qsb="$candidate"
            break
        fi
    done
fi
if [ -z "$qsb" ]; then
    echo "make-shaders.sh: Qt's qsb is not installed." >&2
    exit 1
fi
for shader in "$here"/crates/sioul-app/shaders/*.frag; do
    "$qsb" --qt6 -o "$shader.qsb" "$shader"
    echo "$(basename "$shader").qsb"
done
