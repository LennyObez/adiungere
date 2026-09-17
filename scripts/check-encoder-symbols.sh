#!/bin/sh
#
# Refuse a release command line that links an encoder.
#
# The lossless export copies the recorder's samples into a new container and never re-encodes them. That is
# a promise about a binary, so it is checked on the binary: the command line is built in the release profile
# with its symbol table kept, every symbol it defines or imports is read with the toolchain's own symbol
# reader, and any symbol that contains a fragment from tools/forbidden-encoder-symbols.txt turns the check
# red. Imported symbols count as much as defined ones, because an encoder reached through a shared library
# is still an encoder in the binary.
#
# Before the product is judged, the check judges a fixture that carries one forbidden name and nothing
# else. A check that has never been seen to find anything is a check that may find nothing; this one finds
# the fixture first, every time it runs, or it stops there.
#
# Usage:
#     scripts/check-encoder-symbols.sh              the fixture, then the release command line
#     scripts/check-encoder-symbols.sh --self-test  the fixture only, which is what guarantee G25 runs
#
# The symbol reader is the `llvm-tools` component of the pinned toolchain, so the same reader runs on every
# platform the product is built on. A missing component is a check that cannot run, and a check that did not
# run is not a check that passed, so the script refuses rather than skipping.

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
list="$root/tools/forbidden-encoder-symbols.txt"
fixture="$root/scripts/encoder-symbol-fixture.rs"
mode=${1:-all}

case "$mode" in
    all | --self-test) ;;
    *)
        printf 'check-encoder-symbols: unknown argument %s\n' "$mode" >&2
        exit 2
        ;;
esac

refuse() {
    printf 'check-encoder-symbols: %s\n' "$1" >&2
    exit 1
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

# The symbol reader that ships with the toolchain, found through the compiler so that the pinned toolchain
# and nothing else decides which one runs.
host=$(rustc -vV | awk '$1 == "host:" { print $2 }')
reader="$(rustc --print sysroot)/lib/rustlib/$host/bin/llvm-nm"
if [ ! -x "$reader" ]; then
    refuse "the symbol reader is not installed at $reader; the llvm-tools component of rust-toolchain.toml provides it (rustup component add llvm-tools)"
fi

# The fragments, without the comments and blank lines of the list. An empty list would make every binary
# pass, so it is refused.
grep -v '^#' "$list" | grep -v '^[[:space:]]*$' > "$work/fragments" || true
if [ ! -s "$work/fragments" ]; then
    refuse "$list names no fragment; this check is inert"
fi

# Reads every symbol of a binary into a file and refuses a binary that has none, because a binary with no
# symbol table is one this check cannot read, and an unreadable binary must never pass as a clean one.
symbols_of() {
    "$reader" --demangle "$1" > "$2"
    if [ ! -s "$2" ]; then
        refuse "$1 carries no symbol; this check cannot read it"
    fi
}

# The fixture: compiled here, read here, and refused here. Its one forbidden name must be found, and found
# under the expected fragment, before the product is looked at.
if ! rustc --edition 2024 -O -C debuginfo=0 -o "$work/fixture" "$fixture" > "$work/fixture.log" 2>&1; then
    cat "$work/fixture.log" >&2
    refuse "the fixture did not compile"
fi
symbols_of "$work/fixture" "$work/fixture.symbols"
if grep -F -f "$work/fragments" "$work/fixture.symbols" > "$work/fixture.matches"; then
    if ! grep -q 'x264_encoder_open' "$work/fixture.matches"; then
        cat "$work/fixture.matches" >&2
        refuse "the fixture was refused, but not for the symbol it carries"
    fi
    printf 'The fixture is refused, as it must be, for:\n'
    sed 's/^/    /' "$work/fixture.matches"
else
    status=$?
    if [ "$status" -ne 1 ]; then
        refuse "the symbol search failed with status $status"
    fi
    refuse "the fixture carries a forbidden symbol and the check did not find it; nothing this check says about the product can be believed"
fi

if [ "$mode" = "--self-test" ]; then
    exit 0
fi

# The product, in the release profile. That profile strips the symbol table, which is right for a
# distributed binary and wrong for this reading of it, so stripping alone is switched off for this build;
# every other setting of the profile, link-time optimisation included, is the one the released binary gets,
# which matters because it is that optimisation that discards what nothing references.
workspace="$root/core/Cargo.toml"
target_dir=${CARGO_TARGET_DIR:-$root/core/target}
if ! CARGO_PROFILE_RELEASE_STRIP=none cargo build --manifest-path "$workspace" --release --locked -p adiungere-cli \
    > "$work/build.log" 2>&1; then
    cat "$work/build.log" >&2
    refuse "the release command line did not build"
fi
binary="$target_dir/release/adiungere"
if [ ! -f "$binary" ]; then
    binary="$binary.exe"
fi
if [ ! -f "$binary" ]; then
    refuse "the release command line was built but is not at $binary"
fi

symbols_of "$binary" "$work/product.symbols"
symbols=$(wc -l < "$work/product.symbols" | tr -d ' ')
if grep -F -f "$work/fragments" "$work/product.symbols" > "$work/product.matches"; then
    printf 'The release command line links an encoder. These symbols are forbidden:\n' >&2
    sed 's/^/    /' "$work/product.matches" >&2
    exit 1
else
    status=$?
    if [ "$status" -ne 1 ]; then
        refuse "the symbol search failed with status $status"
    fi
fi

printf 'The release command line carries %s symbols and none of them names an encoder.\n' "$symbols"
