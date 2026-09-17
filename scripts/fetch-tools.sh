#!/bin/sh
#
# Fetches the pinned external tools into `.tools/`, verifying each archive against the digest recorded in
# `tools/versions.toml`. Nothing is installed system-wide and nothing is trusted that does not match.
#
# The file is read with the shell rather than a parser: the keys are fixed and the values are quoted
# strings on their own lines, which is a deliberate constraint on the file so that a pipeline can read it
# without a dependency.

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
versions="$root/tools/versions.toml"
tools="$root/.tools"

# The value of a key inside one table of the versions file.
value_of() {
    table=$1
    key=$2
    awk -v table="[$table]" -v key="$key" '
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

platform() {
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) printf 'linux-x86_64' ;;
        *)
            printf 'fetch-tools: no pinned build of the media tool for %s-%s\n' "$(uname -s)" "$(uname -m)" >&2
            exit 1
            ;;
    esac
}

fetch_ffmpeg() {
    table="ffmpeg.$(platform)"
    url=$(value_of "$table" url)
    digest=$(value_of "$table" sha256)
    binary=$(value_of "$table" binary)
    version=$(value_of ffmpeg version)

    if [ -z "$url" ] || [ -z "$digest" ] || [ -z "$binary" ] || [ -z "$version" ]; then
        printf 'fetch-tools: tools/versions.toml does not describe the media tool for this platform\n' >&2
        exit 1
    fi

    mkdir -p "$tools"
    archive="$tools/ffmpeg-$version.tar.xz"

    if [ -x "$tools/$binary" ]; then
        printf 'fetch-tools: the media tool is already present at %s\n' "$tools/$binary"
        return 0
    fi

    printf 'fetch-tools: fetching the media tool %s\n' "$version"
    curl --fail --silent --show-error --location --output "$archive" "$url"

    observed=$(sha256sum "$archive" | cut -c1-64)
    if [ "$observed" != "$digest" ]; then
        rm -f "$archive"
        printf 'fetch-tools: the archive digest is %s and the pin says %s; nothing was installed\n' \
            "$observed" "$digest" >&2
        exit 1
    fi

    tar -xJf "$archive" -C "$tools"
    rm -f "$archive"

    if [ ! -x "$tools/$binary" ]; then
        printf 'fetch-tools: the archive did not contain %s\n' "$binary" >&2
        exit 1
    fi

    printf 'fetch-tools: %s\n' "$("$tools/$binary" -version | head -n 1)"
}

fetch_ffmpeg
