#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Sioul's files on a phone, read from this computer for a diagnosis
# (docs/android.md, "Sioul's files over USB"): what Sioul keeps in its
# private storage, which nothing else on the phone reads, listed and copied
# here through adb, with no copy on the phone and no root.
#
#     tools/android-files.sh list [PATH]        a folder's entries (the roots without PATH)
#     tools/android-files.sh read PATH          a file, written out as it is
#     tools/android-files.sh pull PATH [DEST]   a file or a whole folder, into DEST/PATH
#                                               (DEST: $XDG_CACHE_HOME/sioul-android-files)
#
# PATH starts with one of the four roots: config, data, state (Sioul's own
# folders: its settings, its lists and records, its state and logs) and
# cache (Sioul's own cache). For instance: state/sioul/extensions/planity.log,
# config/sioul. Pull the logs and the state a diagnosis needs, never data
# whole: it holds the mail. DEST is never inside a git work tree, so that
# the person's files cannot land in a repository (this one is public).
#
# It needs, on the phone: Sioul's switch on, Settings ▸ Devices ▸ this
# phone's card ▸ Diagnosis ▸ "Let a computer read Sioul's files over USB";
# USB debugging (or wireless debugging) allowed for this computer. Here:
# Android's adb (ADB names another; ANDROID_SERIAL chooses among several
# phones, as adb reads it). Each command is adb's shell asking Sioul's
# provider, as one can by hand:
#
#     adb shell content query --uri content://com.aurelienpierre.sioul.debug/state/sioul
#     adb shell content read --uri content://com.aurelienpierre.sioul.debug/state/sioul/extensions/planity.log
#
# A listing's first line is the one asked ("."); a link is said as one and
# never followed by pull. Sioul answers the shell's uid alone (a computer
# allowed to debug the phone, or on the phone an adb client or an app given
# adb's access, such as Shizuku), read only, inside those four roots; never
# the secret parts of OpenPGP keys, the home card's codes, the sharing's
# memory nor a health record. Passwords and tokens are in the keyring,
# sealed by Android's KeyStore, outside the roots.
set -euo pipefail

authority=com.aurelienpierre.sioul.debug
adb=${ADB:-adb}

usage() {
    echo "usage: tools/android-files.sh list [PATH] | read PATH | pull PATH [DEST]" >&2
    exit 2
}

# PATH as an address's path: each byte outside letters, digits and - . _ ~ /
# written %XX, so that a name with spaces or accents reaches the phone's
# shell and the provider as it is.
encode() {
    local byte out=""
    for byte in $(printf '%s' "$1" | od -An -v -tx1); do
        case $byte in
            2[def] | 3[0-9] | 4[1-9a-f] | 5[0-9a] | 5f | 6[1-9a-f] | 7[0-9a] | 7e) out+=$(printf "\\x$byte") ;;
            *) out+="%${byte^^}" ;;
        esac
    done
    printf '%s' "$out"
}

# PATH without the slashes around it, nor doubled.
clean() {
    local path=$1
    while [[ $path == /* ]]; do path=${path#/}; done
    while [[ $path == */ ]]; do path=${path%/}; done
    while [[ $path == *//* ]]; do path=${path//\/\//\/}; done
    printf '%s' "$path"
}

uri() {
    printf 'content://%s/%s' "$authority" "$(encode "$1")"
}

# What the phone said when it refused, made plain; then out.
refused() {
    local said=$1
    if grep -q -e "Could not find provider" -e "Unknown authority" -e "No content provider" <<< "$said"; then
        echo "android-files: Sioul does not answer on this phone. Turn on Settings ▸ Devices ▸ this phone's card ▸ Diagnosis ▸ \"Let a computer read Sioul's files over USB\" (and check that Sioul is installed)." >&2
    elif grep -q "Only adb's shell" <<< "$said"; then
        echo "android-files: refused: only adb's shell reads Sioul's files." >&2
    elif grep -q "is off on this phone" <<< "$said"; then
        echo "android-files: the switch is off on the phone (Settings ▸ Devices ▸ this phone's card ▸ Diagnosis)." >&2
    elif grep -q "Not one of Sioul's files" <<< "$said"; then
        echo "android-files: not one of Sioul's files that can be read: $2" >&2
    else
        printf '%s\n' "$said" | sed 's/^/android-files: /' >&2
    fi
    exit 1
}

# A listing's rows, one per line, tab-separated: kind, size, modified, name.
rows() {
    local said status=0
    said=$("$adb" shell -T content query --uri "$(uri "$1")" 2>&1) || status=$?
    if [[ $said == "No result found." ]]; then
        return 0
    fi
    if [[ $status -ne 0 ]] || ! grep -q '^Row: ' <<< "$said"; then
        refused "$said" "$1"
    fi
    sed -n -E 's/^Row: [0-9]+ kind=([^,]*), size=([^,]*), modified=([^,]*), name=(.*)$/\1\t\2\t\3\t\4/p' <<< "$said"
}

# The file at PATH, written to standard output as it is (adb's shell without
# a terminal: no byte changed); refused before anything is written.
read_file() {
    local first
    first=$(rows "$1" | head -1)
    [[ $(cut -f1 <<< "$first") == file ]] || { echo "android-files: not a file: $1" >&2; exit 1; }
    "$adb" shell -T content read --uri "$(uri "$1")"
}

# PATH into DEST/PATH: a file as it is, a folder with everything under it but links.
pull() {
    local path=$1 dest=$2 kind size modified name
    local listing
    listing=$(rows "$path")
    IFS=$'\t' read -r kind size modified name <<< "$(head -1 <<< "$listing")"
    case $kind in
        file)
            mkdir -p "$(dirname "$dest/$path")"
            read_file "$path" > "$dest/$path.part"
            mv "$dest/$path.part" "$dest/$path"
            echo "$path ($size bytes)"
            ;;
        folder)
            mkdir -p "$dest/$path"
            tail -n +2 <<< "$listing" | while IFS=$'\t' read -r kind size modified name; do
                [[ -n $name ]] || continue
                case $kind in
                    file | folder) pull "$path/$name" "$dest" ;;
                    *) echo "$path/$name: a $kind, left" >&2 ;;
                esac
            done
            ;;
        *)
            echo "android-files: $path: a ${kind:-nothing}, not pulled" >&2
            ;;
    esac
}

command -v "$adb" > /dev/null || { echo "android-files: no adb (Android's platform tools; ADB names another)." >&2; exit 77; }

[[ $# -ge 1 ]] || usage
verb=$1
shift
case $verb in
    list)
        [[ $# -le 1 ]] || usage
        path=$(clean "${1:-}")
        rows "$path" | while IFS=$'\t' read -r kind size modified name; do
            [[ $size == NULL ]] && size=-
            [[ $modified == NULL ]] && modified=-
            [[ $kind == folder && $name != . ]] && name="$name/"
            printf '%-6s %12s  %-20s  %s\n' "$kind" "$size" "$modified" "$name"
        done
        ;;
    read)
        [[ $# -eq 1 ]] || usage
        path=$(clean "$1")
        [[ -n $path ]] || usage
        read_file "$path"
        ;;
    pull)
        [[ $# -ge 1 && $# -le 2 ]] || usage
        path=$(clean "$1")
        [[ -n $path ]] || usage
        dest=${2:-${XDG_CACHE_HOME:-$HOME/.cache}/sioul-android-files}
        # Its nearest folder that exists, asked whether a repository holds it, before anything is made.
        probe=$dest
        while [[ ! -d $probe && $probe != "$(dirname "$probe")" ]]; do probe=$(dirname "$probe"); done
        if git -C "$probe" rev-parse --is-inside-work-tree > /dev/null 2>&1; then
            echo "android-files: $dest is inside a git work tree: choose a folder outside any repository, so that the phone's files cannot be committed." >&2
            exit 1
        fi
        mkdir -p "$dest"
        echo "android-files: into $dest" >&2
        pull "$path" "$dest"
        ;;
    *)
        usage
        ;;
esac
