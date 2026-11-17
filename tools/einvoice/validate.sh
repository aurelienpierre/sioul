#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Checks the invoices Sioul issues with Mustang (mustangproject.org), offline
# (docs/projects.md, "The issued file"):
#   - every PDF of valid/ must pass whole (its PDF/A-3b, its Factur-X XML,
#     the report's own summary) and draw no word from the French rules
#     (XP Z12-012's schematron, which Mustang runs beside EN 16931's);
#   - every PDF of invalid/ must fail for its own rule, the one its .rule
#     file beside it names (BR-CO-15), not for any other reason;
#   - every PDF of without-xml/, which carries no XML yet, must pass as a
#     PDF/A-3b.
# Mustang's exit status is not enough (it may end at 0 while its PDF/A part
# failed): each part of its report is read, and every warning and error is
# listed. The reports are kept in reports/.
#
#   SIOUL_INVOICE_FIXTURES=<folder> cargo test --release -p sioul-core --lib -- --ignored --exact invoice::samples::write_fixtures
#   tools/einvoice/validate.sh <Mustang-CLI-2.26.0.jar> <folder>
#
# Never an online validator: an invoice names people and their money.
set -euo pipefail
jar=$(realpath "$1")
root=$(realpath "$2")
reports="$root/reports"
mkdir -p "$reports"
# Mustang writes its own log where it runs: there, not in the sources.
cd "$reports"

# Every summary of a report, in order: its PDF's, its XML's (when it read one), its own.
statuses() { grep -o 'summary status="[a-z]*"' "$1" | sed 's/.*="\(.*\)"/\1/' | tr '\n' ' ' || true; }
# What veraPDF found of the PDF alone: "3b true" for a compliant PDF/A-3b.
pdfa() { grep -o 'flavour=[0-9a-z]*\|isCompliant=[a-z]*' "$1" | sed 's/.*=//' | tr '\n' ' ' || true; }
# Every warning and error of a report, one a line.
messages() { grep -o '<\(warning\|error\)[^>]*>[^<]*' "$1" | sed 's/^<\(warning\|error\)[^>]*>/\1: /' | cut -c1-240 || true; }
# How many messages the French rules gave.
french() { grep -c 'XP_Z12_012' "$1" || true; }

failures=0
check() {
    local file=$1 want=$2 name report all part good rule
    name=$(basename "$(dirname "$file")")-$(basename "$file" .pdf)
    report="$reports/$name.xml"
    java -Xmx1G -Dfile.encoding=UTF-8 -jar "$jar" --action validate --source "$file" > "$report" 2> "$reports/$name.log" || true
    all=$(statuses "$report")
    part=$(pdfa "$report")
    good=no
    case "$want" in
        valid) [[ -n "$all" && "$all" != *invalid* && "$part" == "3b true " && $(french "$report") -eq 0 ]] && good=yes ;;
        invalid)
            rule=$(cat "${file%.pdf}.rule" 2>/dev/null || true)
            [[ "$all" == *invalid* && -n "$rule" ]] && grep -q "\[$rule\]" "$report" && good=yes
            all="$all/ for ${rule:-no rule named} "
            ;;
        pdfa) [[ "$part" == "3b true " ]] && good=yes ;;
    esac
    if [[ $good == yes ]]; then
        echo "as expected ($want): $name: $all/ PDF/A $part"
    else
        echo "NOT AS EXPECTED ($want): $name: $all/ PDF/A $part"
        grep -o '<exception[^>]*>[^<]*' "$report" | head -5 || true
        failures=$((failures + 1))
    fi
    # Its warnings and errors, for a reader of the log.
    messages "$report" | sed 's/^/    /'
}

count=0
for set in valid:valid invalid:invalid without-xml:pdfa; do
    folder=${set%%:*}
    want=${set##*:}
    for file in "$root/$folder"/*.pdf; do
        [[ -e "$file" ]] || continue
        check "$file" "$want"
        count=$((count + 1))
    done
done
if [[ $count -eq 0 ]]; then
    echo "No invoice found in $root"
    exit 1
fi
echo "$count files, $failures not as expected."
[[ $failures -eq 0 ]]
