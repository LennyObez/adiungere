#!/bin/sh
#
# Run the whole gate sequence from docs/testing.md, in order.
#
# Two rules from that document are the reason this script exists rather than a chain of commands. No step is
# piped, because a pipeline reports the last stage's status and not the step's. And every exit code is read
# before the next step runs, because a sequence that keeps going after a red step is measuring a different
# thing from the one it claims to measure.
#
# Each step writes its own output to .gate/, which is untracked. On a failure the output is printed and the
# script stops there.

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
output="$root/.gate"
workspace="$root/core/Cargo.toml"

rm -rf "$output"
mkdir -p "$output"

cd "$root"

step=0

run() {
    name=$1
    shift

    step=$((step + 1))
    slug=$(printf '%s' "$name" | tr ' [:upper:]' '-[:lower:]')
    log="$output/$(printf '%02d' "$step")-$slug.log"

    printf '%2d. %-52s' "$step" "$name"

    if "$@" >"$log" 2>&1; then
        printf 'ok\n'
        return 0
    fi

    status=$?
    printf 'FAILED\n\n'
    cat "$log"
    printf '\nStep %d failed with status %d.\nIts output is in %s\n' "$step" "$status" "$log"

    exit 1
}

printf 'Gate sequence, from docs/testing.md\n\n'

run 'formatting' cargo fmt --manifest-path "$workspace" --all --check
run 'lints' cargo clippy --manifest-path "$workspace" --workspace --all-targets --all-features -- -D warnings
run 'unit tests' cargo test --manifest-path "$workspace" --workspace --lib
run 'integration and guarantee tests' cargo test --manifest-path "$workspace" --workspace --tests
run 'documentation tests' cargo test --manifest-path "$workspace" --workspace --doc
run 'the lock file matches the manifests' cargo metadata --manifest-path "$workspace" --locked --format-version 1
run 'evidence register agrees with the roadmap' \
    cargo run --quiet --manifest-path "$workspace" -p adiungere-cli -- probes check
run 'the published site' "$root/scripts/check-site.sh"

if command -v shellcheck >/dev/null 2>&1; then
    run 'shell scripts' shellcheck "$root/scripts/check-site.sh" "$root/scripts/gate.sh"
else
    printf '%2d. %-52s%s\n' "$((step + 1))" 'shell scripts' 'NOT RUN'
    printf '\nshellcheck is not installed, so the scripts were not checked.\n'
    printf 'A step that did not run is not a step that passed.\n'
    exit 1
fi

if command -v cargo-deny >/dev/null 2>&1; then
    run 'advisories, licences, sources and bans' cargo deny --manifest-path "$workspace" check
else
    printf '%2d. %-52s%s\n' "$((step + 1))" 'advisories, licences, sources and bans' 'NOT RUN'
    printf '\ncargo-deny is not installed, so the supply-chain policy was not checked.\n'
    printf 'A step that did not run is not a step that passed. Install it and run this again:\n'
    printf '    cargo install cargo-deny --locked\n'
    exit 1
fi

printf '\nEvery step ran and every step passed. Output is in %s\n' "$output"
