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
# alone, then copied beside its place under a hidden name and checked for
# test hooks there (tools/check-test-marks.sh): a `cargo test` run after the
# last `cargo build --release`, by you or by anything else on this machine
# meanwhile, leaves programs built for tests in target/release/, which keep
# no password (review of 19 October 2026, N1). The copy checked is the one
# that takes its place; nothing is installed when a check fails.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
prefix=${PREFIX:-$HOME/.local}

# Each alone, as the final pass does: built together, Cargo would unify
# their features into a third variant of the command line.
cargo build --release -p sioul-cli
cargo build --release -p sioul-app

mkdir -p "$prefix/bin"
staged=("$prefix/bin/.sioul-app.new" "$prefix/bin/.sioul.new")
trap 'rm -f "${staged[@]}"' EXIT
install -m755 target/release/sioul-app "${staged[0]}"
install -m755 target/release/sioul "${staged[1]}"
sh tools/check-test-marks.sh "${staged[@]}"
mv -f "${staged[0]}" "$prefix/bin/sioul-app"
mv -f "${staged[1]}" "$prefix/bin/sioul"
install -Dm644 data/com.aurelienpierre.Sioul.desktop "$prefix/share/applications/com.aurelienpierre.Sioul.desktop"
install -Dm644 data/com.aurelienpierre.Sioul.metainfo.xml "$prefix/share/metainfo/com.aurelienpierre.Sioul.metainfo.xml"
mkdir -p "$prefix/share/icons" && cp -r data/icons/hicolor "$prefix/share/icons/"
echo "Installed in $prefix: sioul-app and sioul."
