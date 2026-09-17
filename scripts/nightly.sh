#!/bin/sh
#
# The nightly checks that need the dated nightly compiler: fuzzing, undefined-behaviour checking and
# mutation testing. The channel comes from `tools/versions.toml` and nowhere else; the product's own
# compiler is pinned in `rust-toolchain.toml` and this script never touches it.
#
#     scripts/nightly.sh install            install the dated nightly with the components the jobs need
#     scripts/nightly.sh seeds DIR          write the synthetic corpus into DIR as fuzzing seeds
#     scripts/nightly.sh fuzz TARGET DIR S  run one fuzz target for S seconds, seeded from DIR
#     scripts/nightly.sh miri               run the unit tests of the reader and the fingerprints under Miri
#     scripts/nightly.sh mutants            run mutation testing on the reader and the fingerprints

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
versions="$root/tools/versions.toml"
workspace="$root/core/Cargo.toml"

# The value of a key inside one table of the versions file.
value_of() {
    awk -v table="[$1]" -v key="$2" '
        $0 == table { inside = 1; next }
        /^\[/ { inside = 0 }
        inside && $1 == key {
            value = $0
            sub(/^[^=]*=[[:space:]]*"/, "", value)
            sub(/"[[:space:]]*$/, "", value)
            print value
            exit
        }
    ' "$versions"
}

# Installs a cargo subcommand at its pinned version unless that exact version is already present.
install_pinned() {
    name=$1
    version=$(value_of "$name" version)
    if [ -z "$version" ]; then
        printf 'nightly: tools/versions.toml pins no version for %s\n' "$name" >&2
        exit 1
    fi
    if cargo install --list | grep -q "^$name v$version:"; then
        return 0
    fi
    cargo install "$name" --locked --version "$version"
}

channel=$(value_of rust-nightly channel)

if [ -z "$channel" ]; then
    printf 'nightly: tools/versions.toml pins no nightly channel\n' >&2
    exit 1
fi

case "${1:-}" in
    install)
        rustup toolchain install "$channel" --profile minimal --component rust-src,miri,llvm-tools
        rustup run "$channel" cargo --version
        install_pinned cargo-fuzz
        ;;
    seeds)
        cargo run --quiet --manifest-path "$workspace" -p adiungere-fixtures -- "$2"
        # The sparse recording is a hole four gigabytes wide; a seed that size teaches the fuzzer nothing.
        rm -f "$2/beyond-4gib.mp4"
        ;;
    fuzz)
        target=$2
        seeds=$3
        seconds=$4
        cd "$root/core/fuzz"
        rustup run "$channel" cargo fuzz run --target x86_64-unknown-linux-gnu "$target" "$seeds" -- \
            -max_total_time="$seconds" -max_len=262144 -rss_limit_mb=2048
        ;;
    miri)
        cd "$root/core"
        MIRIFLAGS='-Zmiri-strict-provenance' rustup run "$channel" cargo miri test \
            -p adiungere-isobmff -p adiungere-fingerprint -p adiungere-manifest -p adiungere-scan --lib
        ;;
    mutants)
        install_pinned cargo-mutants
        cd "$root/core"
        cargo mutants --package adiungere-isobmff --package adiungere-fingerprint --package adiungere-manifest \
            --package adiungere-scan --timeout 300 --jobs 2
        ;;
    *)
        printf 'usage: scripts/nightly.sh install | seeds DIR | fuzz TARGET DIR SECONDS | miri | mutants\n' >&2
        exit 2
        ;;
esac
