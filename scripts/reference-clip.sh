#!/bin/sh
#
# The reference recording, in the nightly pipeline and on a maintainer's machine.
#
#     scripts/reference-clip.sh fetch PATH     fetch the recording from $REFERENCE_CLIP_URL into PATH and
#                                              refuse it unless its digest is the pinned one
#     scripts/reference-clip.sh check PATH     fingerprint the recording at PATH and compare every number
#                                              with core/fixtures/reference.json
#
# The recording is private. It is never written into the repository, its location is a secret, and the
# only things this script prints are digests and counts that are already public in the pinned file.

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
pinned="$root/core/fixtures/reference.json"
workspace="$root/core/Cargo.toml"

fetch() {
    destination=$1
    if [ -z "${REFERENCE_CLIP_URL:-}" ]; then
        printf 'reference-clip: REFERENCE_CLIP_URL is not set. The nightly pipeline needs it as a secret; a\n' >&2
        printf 'reference-clip: missing recording is a red job, not a skipped one.\n' >&2
        exit 1
    fi
    curl --fail --silent --show-error --location --output "$destination" "$REFERENCE_CLIP_URL"
    observed=$(sha256sum "$destination" | cut -c1-64)
    expected=$(jq -r '.sha256' "$pinned")
    if [ "$observed" != "$expected" ]; then
        rm -f "$destination"
        printf 'reference-clip: the fetched file has digest %s; the pinned recording has %s\n' \
            "$observed" "$expected" >&2
        exit 1
    fi
}

check() {
    recording=$1
    manifest=$(mktemp)
    cargo run --quiet --release --manifest-path "$workspace" -p adiungere-cli -- \
        fingerprint "$recording" --format json > "$manifest"

    # Every pinned number, compared one by one so a failure names the number.
    jq -e --slurpfile pin "$pinned" '
        def expect(name; actual; wanted):
            if actual == wanted then empty
            else error("\(name): the product says \(actual), the pin says \(wanted)") end;
        expect("size"; .file.size; $pin[0].size),
        expect("sha256"; .file.sha256; $pin[0].sha256),
        expect("structural"; .file.structure.structural_fingerprint.sha256; $pin[0].structural_sha256),
        (.tracks | length) as $count | expect("track count"; $count; ($pin[0].tracks | length)),
        ($pin[0].tracks[] as $track |
            .tracks[$track.index] as $ours |
            expect("track \($track.index) coding"; $ours.coding; $track.coding),
            expect("track \($track.index) samples"; $ours.sample_count; $track.sample_count),
            expect("track \($track.index) payload"; $ours.fingerprint.payload_sha256; $track.payload_sha256),
            expect("track \($track.index) configuration"; $ours.fingerprint.configuration_sha256; $track.configuration_sha256),
            expect("track \($track.index) stream"; ($ours.elementary_stream.sha256 // null); $track.stream_sha256),
            expect("track \($track.index) stream bytes"; ($ours.elementary_stream.stream_bytes // null); $track.stream_bytes)),
        (.vendor.boxes | length) as $vendors | expect("vendor box count"; $vendors; ($pin[0].vendor | length)),
        . as $manifest |
        ($pin[0].vendor | to_entries[] | .key as $i | .value as $box |
            $manifest.vendor.boxes[$i] as $ours |
            expect("vendor \($box.kind) location"; $ours.location; $box.location),
            expect("vendor \($box.kind) size"; $ours.size; $box.size),
            expect("vendor \($box.kind) digest"; $ours.sha256; $box.sha256)),
        true
    ' "$manifest" > /dev/null

    rm -f "$manifest"
    printf 'reference-clip: every pinned number was reproduced\n'
}

case "${1:-}" in
    fetch) fetch "$2" ;;
    check) check "$2" ;;
    *)
        printf 'usage: scripts/reference-clip.sh fetch PATH | check PATH\n' >&2
        exit 2
        ;;
esac
