#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Lints the window's QML as one module: the qmldir and types the build
# generated, with the pages beside them, so qmllint knows Sioul's own types.
# Run `cargo build -p sioul-app` first.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
# The newest build's types: each feature set builds in its own folder.
types=$(ls -t "$repo"/target/debug/build/sioul-app-*/out/qt-build-utils/qml_modules/com/aurelienpierre/sioul/plugin.qmltypes 2>/dev/null | head -1)
generated=${types%/com/aurelienpierre/sioul/plugin.qmltypes}
if [ -z "$types" ]; then
    echo "Build the window first: cargo build -p sioul-app" >&2
    exit 1
fi
lint=$(mktemp -d "$repo/target/lint-qml.XXXX")
trap 'rm -rf "$lint"' EXIT
module=$lint/com/aurelienpierre/sioul
mkdir -p "$module/qml" "$module/qml-desktop"
cp "$generated/com/aurelienpierre/sioul/qmldir" "$generated/com/aurelienpierre/sioul/plugin.qmltypes" "$module/"
cp "$repo"/crates/sioul-app/qml/*.qml "$module/qml/"
# The desktop's own pages (the system tray), apart from what the phone's build reads.
cp "$repo"/crates/sioul-app/qml-desktop/*.qml "$module/qml-desktop/"
cd "$module"
# Fedora names the Qt 6 tool qmllint-qt6; a bare qmllint may be Qt 5's.
qmllint=$(command -v qmllint-qt6 || command -v qmllint6 || echo /usr/lib64/qt6/bin/qmllint)
"$qmllint" -I "$lint" qml/*.qml qml-desktop/*.qml
