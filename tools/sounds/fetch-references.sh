#!/bin/sh
# Downloads the CC0 recordings Sioul's sounds of nature were tuned against
# (tools/sounds/references.tsv) into DIR/<kind>/: Freesound's previews, 128 kb/s
# MP3, enough for what these sounds hold (under 10 kHz). They are references
# to measure against, never shipped nor kept in the repository.
#
#     tools/sounds/fetch-references.sh DIR
set -eu
dir=${1:?"a folder to download into"}
list=$(dirname "$0")/references.tsv
tail -n +2 "$list" | while IFS="$(printf '\t')" read -r kind preview page licence title; do
    id=${preview%%_*}
    mkdir -p "$dir/$kind"
    [ -s "$dir/$kind/fs-$id.mp3" ] && continue
    echo "$kind: $title ($page)"
    curl -sSf --max-time 900 -o "$dir/$kind/fs-$id.mp3.part" "https://cdn.freesound.org/previews/$((id / 1000))/$preview-hq.mp3"
    mv "$dir/$kind/fs-$id.mp3.part" "$dir/$kind/fs-$id.mp3"
done
