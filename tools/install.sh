#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Builds the command line and the window, then installs them for you alone,
# with the desktop file, the AppStream file and the icons (the recipe of
# docs/building.md, "The window", in one step):
#
#     tools/install.sh
#
# PREFIX sets where (default: ~/.local). Each program is built right here,
# alone, then checked for test hooks before it is installed
# (tools/check-test-marks.sh): a `cargo test` run after the last
# `cargo build --release` leaves programs built for tests in
# target/release/, which keep no password (review of 19 October 2026, N1).
# Nothing is installed when a check fails.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
prefix=${PREFIX:-$HOME/.local}

# Each alone, as the final pass does: built together, Cargo would unify
# their features into a third variant of the command line.
cargo build --release -p sioul-cli
cargo build --release -p sioul-app
sh tools/check-test-marks.sh target/release/sioul target/release/sioul-app

install -Dm755 target/release/sioul-app "$prefix/bin/sioul-app"
install -Dm755 target/release/sioul "$prefix/bin/sioul"
install -Dm644 data/com.aurelienpierre.Sioul.desktop "$prefix/share/applications/com.aurelienpierre.Sioul.desktop"
install -Dm644 data/com.aurelienpierre.Sioul.metainfo.xml "$prefix/share/metainfo/com.aurelienpierre.Sioul.metainfo.xml"
mkdir -p "$prefix/share/icons" && cp -r data/icons/hicolor "$prefix/share/icons/"
echo "Installed in $prefix: sioul-app and sioul."
