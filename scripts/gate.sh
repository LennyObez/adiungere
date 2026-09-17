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
# script stops there. A tool that is missing is a step that did not run, and a step that did not run is not
# a step that passed, so the script refuses rather than skipping.

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

    # The status is read inside the branch that has it. Read after the `if`, it would be the status of the
    # `if` itself, which is zero whenever no branch ran.
    if "$@" >"$log" 2>&1; then
        printf 'ok\n'
    else
        status=$?
        printf 'FAILED\n\n'
        cat "$log"
        printf '\nStep %d failed with status %d.\nIts output is in %s\n' "$step" "$status" "$log"
        exit 1
    fi
}

require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf '\n%s is not installed, so the gate cannot run in full.\n' "$1"
        printf 'A step that did not run is not a step that passed. Install it and run this again:\n'
        printf '    %s\n' "$2"
        exit 1
    fi
}

require cargo-deny 'cargo install cargo-deny --locked'
require shellcheck 'your package manager, for example: apt install shellcheck'
require python3 'your package manager; the reference fingerprint script needs the line in tools/versions.toml'

# The pinned media tool is an oracle of the guarantee suite. It is fetched by its digest, never linked,
# and the guarantee that compares against it fails rather than skips when it is absent; so it is checked
# here, where the message can say what to run.
if ! ls "$root"/.tools/*/bin/ffmpeg >/dev/null 2>&1; then
    printf '\nThe pinned media tool is not present under .tools, so the gate cannot run in full.\n'
    printf 'A step that did not run is not a step that passed. Fetch it and run this again:\n'
    printf '    scripts/fetch-tools.sh\n'
    exit 1
fi

# The policy checker at the version pinned in tools/versions.toml, which is the version the supply-chain
# pipeline's action bundles: the gate and the pipeline then read the policy the same way, and a version
# that moves is a pin that moves. The pin is read from the file, never written here.
deny_pinned=$(awk '$0 == "[cargo-deny]" { inside = 1; next } /^\[/ { inside = 0 } inside && $1 == "version" { gsub(/"/, "", $3); print $3; exit }' "$root/tools/versions.toml")
deny_installed=$(cargo deny --version | awk '{ print $2 }')
if [ -z "$deny_pinned" ] || [ "$deny_installed" != "$deny_pinned" ]; then
    printf '\ncargo-deny %s is installed and tools/versions.toml pins %s, so the gate cannot run in full.\n' \
        "$deny_installed" "$deny_pinned"
    printf 'A step that did not run is not a step that passed. Install the pinned version and run this again:\n'
    printf '    cargo install cargo-deny --locked --version %s\n' "$deny_pinned"
    exit 1
fi

printf 'Gate sequence, from docs/testing.md\n\n'

run 'formatting' cargo fmt --manifest-path "$workspace" --all --check
run 'lints' cargo clippy --manifest-path "$workspace" --workspace --all-targets --all-features -- -D warnings
run 'unit tests' cargo test --manifest-path "$workspace" --workspace --lib
run 'integration and guarantee tests' cargo test --manifest-path "$workspace" --workspace --tests
run 'documentation tests' cargo test --manifest-path "$workspace" --workspace --doc
run 'documentation builds without a warning' \
    env RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path "$workspace" --workspace --no-deps --document-private-items
run 'the lock file matches the manifests' cargo metadata --manifest-path "$workspace" --locked --format-version 1
run 'evidence register agrees with the roadmap' \
    cargo run --quiet --manifest-path "$workspace" -p adiungere-cli -- probes check
run 'the published site' "$root/scripts/check-site.sh"
run 'shell scripts' shellcheck "$root"/scripts/*.sh
run 'advisories, licences, sources and bans' cargo deny --manifest-path "$workspace" check
# The fuzzing project is its own package, outside the workspace, with its own lock file: the same policy
# applies to it, or its engine's licence goes unchecked.
run 'the same policy over the fuzzing project' \
    cargo deny --manifest-path "$root/core/fuzz/Cargo.toml" --config "$root/core/deny.toml" check

printf '\nEvery step ran and every step passed. Output is in %s\n' "$output"
