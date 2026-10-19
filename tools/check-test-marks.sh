#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# A program or library about to ship holds no test hook: no `SIOUL_TEST_`
# name at all. A release build names none. A test run builds with the
# features of the tests (sioul-sync's `no-keyring`, which keeps no password),
# and Cargo unifies them into every unit that run builds, the programs
# included: after `cargo test`, `target/release/sioul` and `sioul-app` are
# such programs until the next `cargo build --release` (review of
# 19 October 2026, N1). Every path that ships one runs this on it: the
# packages' workflows, the Flatpak's manifest, tools/install.sh and
# tools/final-pass.sh.
#
#     tools/check-test-marks.sh FILE...
#
# Says each file's marks; exits 1 when a file holds one, 2 when one is
# missing. POSIX sh: it runs in the Flatpak's sandbox and in Git Bash.
set -u
if [ "$#" -eq 0 ]; then
    echo "usage: $0 FILE..." >&2
    exit 2
fi
status=0
for file in "$@"; do
    if [ ! -f "$file" ]; then
        echo "$file: missing" >&2
        status=2
        continue
    fi
    marks=$(grep -a -o 'SIOUL_TEST_[A-Z_]*' "$file" | sort | uniq -c | awk '{ printf "%s%s (%s)", (NR > 1 ? ", " : ""), $2, $1 }')
    if [ -n "$marks" ]; then
        echo "$file: built for tests, never to ship: $marks" >&2
        [ "$status" -eq 0 ] && status=1
    else
        echo "$file: no test mark"
    fi
done
exit "$status"
