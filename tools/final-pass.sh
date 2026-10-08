#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# The final pass before a commit: what a change could break, from the
# repository alone, each step's log in one folder.
#
#     tools/final-pass.sh [FOLDER]
#
# FOLDER receives the logs (default: a new folder under
# ~/.cache/sioul-final-pass/). In order:
# 1. the JVM checks (android/jvm-checks/run.sh): the phone's Java against what
#    Rust decides, then javadoc, strict; skipped without a JDK or android.jar.
#    Their lines of the calls' log are kept for step 2;
# 2. each crate's tests, in release, one crate at a time, each on a D-Bus
#    session of its own (dbus-run-session), so that no test reaches the
#    desktop's, then again with no locale set, as CI runs them; sioul-core's reads the JVM's lines of the calls' log
#    (SIOUL_CALLS_SAMPLE, calls::tests::java_s_lines_read_here);
# 3. the two programs, in release, and no test hook in them (the SIOUL_TEST_*
#    names belong to the test build alone);
# 4. every message in every language, and French typography
#    (tools/check-messages.py);
# 5. the QML: qmllint (tools/lint-qml.sh, which reads the types of the newest
#    debug build: `cargo build -p sioul-app` first) and the QML tests
#    (tools/qml-test.sh);
# 6. rustdoc for each crate, its private items included, warnings as errors
#    (the website's API reference).
# Builds without incremental files (CARGO_INCREMENTAL=0: they double target/),
# and stops before a build when the disk that holds target/ has less than
# 1.5 GB free (MIN_FREE_GB sets another floor). The window itself is never
# started here: tools/demo/run.sh does that (docs/building.md). One line per
# step, the logs' gist under it; exits with 1 when a step failed.
set -uo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo" || exit 1
export CARGO_INCREMENTAL=0 DEBUGINFOD_URLS=
out=${1:-${XDG_CACHE_HOME:-$HOME/.cache}/sioul-final-pass/$(date +%Y%m%d-%H%M%S)}
mkdir -p "$out" || exit 1
out=$(cd "$out" && pwd)
min_free_gb=${MIN_FREE_GB:-1.5}
crates=(sioul-core sioul-sync sioul-learn sioul-cli sioul-app)
failed=()
skipped=()

# Free space where target/ is, in GB with one decimal.
free_gb() {
    local where="$repo/target"
    [[ -d "$where" ]] || where="$repo"
    df -Pk "$where" | awk 'NR == 2 { printf "%.1f", $4 / 1048576 }'
}
# Stops the pass before a build that could fill the disk.
room() {
    local free
    free=$(free_gb)
    if awk -v free="$free" -v floor="$min_free_gb" 'BEGIN { exit !(free < floor) }'; then
        echo "$(date +%H:%M:%S) stopped: ${free} GB free where target/ is, under ${min_free_gb} GB." >&2
        failed+=(stopped-at-the-disk-floor)
        summary
        exit 1
    fi
}
summary() {
    echo "$(date +%H:%M:%S) done, $(free_gb) GB free; logs in $out"
    [[ ${#skipped[@]} -eq 0 ]] || echo "skipped: ${skipped[*]}"
    if [[ ${#failed[@]} -gt 0 ]]; then
        echo "failed: ${failed[*]}"
        return 1
    fi
    echo "every step passed"
}

echo "$(date +%H:%M:%S) final pass of $(git -C "$repo" log -1 --format=%h 2>/dev/null || echo "this tree")$([[ -n "$(git -C "$repo" status --porcelain --untracked-files=no 2>/dev/null)" ]] && echo " with changes"), logs in $out"

# 1. The JVM checks.
sample="$out/calls-sample.jsonl"
rm -f "$sample"
SIOUL_CALLS_SAMPLE="$sample" bash android/jvm-checks/run.sh > "$out/jvm-checks.log" 2>&1
status=$?
said=$(tail -1 "$out/jvm-checks.log" | sed 's/^jvm-checks: //')
case $status in
    0) echo "$(date +%H:%M:%S) jvm-checks: $said" ;;
    77) echo "$(date +%H:%M:%S) jvm-checks skipped: $said"; skipped+=(jvm-checks) ;;
    *) echo "$(date +%H:%M:%S) jvm-checks failed (exit $status)"; grep -E "FAIL|failed|error" "$out/jvm-checks.log" | head -8; failed+=(jvm-checks) ;;
esac

# 2. Each crate's tests.
for crate in "${crates[@]}"; do
    room
    echo "$(date +%H:%M:%S) test $crate, $(free_gb) GB free"
    env=()
    [[ "$crate" == sioul-core && -s "$sample" ]] && env=(SIOUL_CALLS_SAMPLE="$sample")
    env "${env[@]}" timeout 3600 dbus-run-session -- cargo test --release -p "$crate" > "$out/test-$crate.log" 2>&1
    status=$?
    grep -E "^test result: [A-Za-z]+\. [0-9]+ passed|FAILED|panicked|^error" "$out/test-$crate.log" | grep -v " 0 passed; 0 failed" | sort | uniq -c | head -12
    [[ $status -eq 0 ]] || { echo "  exit $status"; failed+=("test-$crate"); }
    # Again with no locale, as CI runs them: a test that names no language
    # reads English only there, and a French word it relies on is missed
    # (8 October 2026: CI failed on such a test that passed here).
    env -u LANG -u LC_ALL -u LC_MESSAGES -u LANGUAGE -u LC_TELEPHONE "${env[@]}" timeout 3600 dbus-run-session -- cargo test --release -p "$crate" > "$out/test-$crate-no-locale.log" 2>&1
    status=$?
    grep -E "FAILED|panicked|^error" "$out/test-$crate-no-locale.log" | sort | uniq -c | head -8 | sed 's/^/  no locale: /'
    [[ $status -eq 0 ]] || { echo "  exit $status with no locale"; failed+=("test-$crate-no-locale"); }
done

# 3. The programs, and no test hook in them.
for crate in sioul-app sioul-cli; do
    room
    echo "$(date +%H:%M:%S) build $crate"
    timeout 3600 cargo build --release -p "$crate" > "$out/build-$crate.log" 2>&1
    status=$?
    grep -E "^warning: unused|never used|^error" "$out/build-$crate.log" | sort | uniq -c | head
    [[ $status -eq 0 ]] || { echo "  exit $status"; failed+=("build-$crate"); }
done
for program in target/release/sioul-app target/release/sioul; do
    if [[ ! -f "$program" ]]; then
        echo "$program: missing"
        failed+=("$(basename "$program")-missing")
        continue
    fi
    hooks=$(grep -c -a -E 'SIOUL_TEST_(PASSWORD|INSECURE_TLS|GITHUB|GOOGLE|NO_ANTIVIRUS|VAULT)' "$program")
    echo "$program, built $(date -r "$program" '+%Y-%m-%d %H:%M'), test hooks: $hooks"
    [[ $hooks -eq 0 ]] || failed+=("hooks-in-$(basename "$program")")
done

# 4. The messages.
python3 tools/check-messages.py > "$out/check-messages.log" 2>&1
status=$?
echo "$(date +%H:%M:%S) check-messages exit $status"
tail -3 "$out/check-messages.log"
[[ $status -eq 0 ]] || failed+=(check-messages)

# 5. The QML.
bash tools/lint-qml.sh > "$out/lint-qml.log" 2>&1
status=$?
echo "$(date +%H:%M:%S) lint-qml exit $status, $(grep -c '^Warning' "$out/lint-qml.log") warnings"
grep '^Warning' "$out/lint-qml.log" | grep -v 'missing-property' | head -5
[[ $status -eq 0 ]] || failed+=(lint-qml)
bash tools/qml-test.sh > "$out/qml-test.log" 2>&1
status=$?
echo "$(date +%H:%M:%S) $(tail -1 "$out/qml-test.log")"
[[ $status -eq 0 ]] || { grep -E "^FAIL" "$out/qml-test.log" | head -8; failed+=(qml-test); }

# 6. rustdoc.
for crate in "${crates[@]}"; do
    room
    RUSTDOCFLAGS="-D warnings" timeout 1800 cargo doc --no-deps --document-private-items -p "$crate" > "$out/doc-$crate.log" 2>&1
    status=$?
    echo "$(date +%H:%M:%S) rustdoc $crate exit $status"
    grep -E "^(warning|error)" "$out/doc-$crate.log" | head -3
    [[ $status -eq 0 ]] || failed+=("doc-$crate")
done

summary
