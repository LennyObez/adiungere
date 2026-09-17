#!/bin/sh
#
# Probe P01: a platform's own reader against the product's fingerprint, on every corpus recording.
#
#     scripts/oracles.sh browser DIR     the browser-side demuxing library, through node
#     scripts/oracles.sh platform DIR    the Apple reader, through the Swift harness, on macOS only
#
# DIR receives the corpus. For each recording and each track, the reader's payload digest must equal the
# manifest's track fingerprint; for video, the reader's decoder description must equal the manifest's
# configuration digest. A disagreement names the recording and the track and fails the script.

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
workspace="$root/core/Cargo.toml"
reader=$1
corpus=$2

cargo run --quiet --manifest-path "$workspace" -p adiungere-fixtures -- "$corpus" > /dev/null
rm -f "$corpus/beyond-4gib.mp4"

compared=0
failed=0

for recording in "$corpus"/*.mp4; do
    manifest=$(mktemp)
    cargo run --quiet --manifest-path "$workspace" -p adiungere-cli -- \
        fingerprint "$recording" --format json > "$manifest"

    case "$reader" in
        browser)
            observed=$(node "$root/tools/oracles/browser-reader.mjs" "$recording")
            ;;
        platform)
            observed=$(swift "$root/tools/oracles/platform-reader.swift" "$recording")
            ;;
        *)
            printf 'usage: scripts/oracles.sh browser DIR | platform DIR\n' >&2
            exit 2
            ;;
    esac

    # Tracks are compared by position: the readers number them from one, the manifest from zero.
    count=$(jq '.tracks | length' "$manifest")
    index=0
    while [ "$index" -lt "$count" ]; do
        ours=$(jq -r ".tracks[$index].fingerprint.payload_sha256" "$manifest")
        theirs=$(printf '%s' "$observed" | jq -r ".[$index].payload_sha256")
        kind=$(jq -r ".tracks[$index].kind" "$manifest")
        if [ "$ours" != "$theirs" ]; then
            printf '%s track %s: the product says %s, the %s reader says %s\n' \
                "$(basename "$recording")" "$index" "$ours" "$reader" "$theirs" >&2
            failed=$((failed + 1))
        fi
        if [ "$reader" = browser ] && [ "$kind" = video ]; then
            configuration=$(jq -r ".tracks[$index].fingerprint.configuration_sha256" "$manifest")
            description=$(printf '%s' "$observed" | jq -r ".[$index].description_sha256")
            if [ "$configuration" != "$description" ]; then
                printf '%s track %s: the configuration digest %s differs from the reader description %s\n' \
                    "$(basename "$recording")" "$index" "$configuration" "$description" >&2
                failed=$((failed + 1))
            fi
        fi
        compared=$((compared + 1))
        index=$((index + 1))
    done
    rm -f "$manifest"
done

if [ "$compared" -lt 20 ]; then
    printf 'oracles: only %s tracks were compared; this check is inert\n' "$compared" >&2
    exit 1
fi
if [ "$failed" -ne 0 ]; then
    printf 'oracles: %s disagreements across %s tracks\n' "$failed" "$compared" >&2
    exit 1
fi
printf 'oracles: the %s reader agrees with the product on all %s tracks\n' "$reader" "$compared"
