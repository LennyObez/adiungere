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

# Fetches one tool: its table in the versions file names the version, and the platform sub-table the
# archive, its digest and the binary inside it. The archive's compression is read from its name.
fetch_tool() {
    name=$1
    what=$2
    version_flag=$3
    table="$name.$(platform)"
    url=$(value_of "$table" url)
    digest=$(value_of "$table" sha256)
    binary=$(value_of "$table" binary)
    version=$(value_of "$name" version)

    if [ -z "$url" ] || [ -z "$digest" ] || [ -z "$binary" ] || [ -z "$version" ]; then
        printf 'fetch-tools: tools/versions.toml does not describe the %s for this platform\n' "$what" >&2
        exit 1
    fi

    mkdir -p "$tools"
    case "$url" in
        *.tar.xz) archive="$tools/$name-$version.tar.xz" ;;
        *.tar.gz) archive="$tools/$name-$version.tar.gz" ;;
        *)
            printf 'fetch-tools: the archive of the %s is neither .tar.xz nor .tar.gz\n' "$what" >&2
            exit 1
            ;;
    esac
    # What an install records about itself: the archive digest it came from and the digest of the binary it
    # left. A later run trusts the install only if both still match, so a pin that moved or a binary that
    # changed under the same name is fetched again rather than accepted.
    record="$tools/$name-$version.installed"

    if [ -x "$tools/$binary" ] && [ -f "$record" ]; then
        recorded_archive=$(sed -n 1p "$record")
        recorded_binary=$(sed -n 2p "$record")
        current_binary=$(sha256sum "$tools/$binary" | cut -c1-64)
        if [ "$recorded_archive" = "$digest" ] && [ "$recorded_binary" = "$current_binary" ]; then
            printf 'fetch-tools: the %s is already present at %s and matches its pin\n' "$what" "$tools/$binary"
            return 0
        fi
        printf 'fetch-tools: the install at %s no longer matches its pin; fetching again\n' "$tools/$binary"
    fi
    rm -rf "${tools:?}/${binary%%/*}" "$record"

    printf 'fetch-tools: fetching the %s %s\n' "$what" "$version"
    curl --fail --silent --show-error --location --output "$archive" "$url"

    observed=$(sha256sum "$archive" | cut -c1-64)
    if [ "$observed" != "$digest" ]; then
        rm -f "$archive"
        printf 'fetch-tools: the archive digest is %s and the pin says %s; nothing was installed\n' \
            "$observed" "$digest" >&2
        exit 1
    fi

    case "$archive" in
        *.tar.xz) tar -xJf "$archive" -C "$tools" ;;
        *) tar -xzf "$archive" -C "$tools" ;;
    esac
    rm -f "$archive"

    if [ ! -x "$tools/$binary" ]; then
        printf 'fetch-tools: the archive did not contain %s\n' "$binary" >&2
        exit 1
    fi
    printf '%s\n%s\n' "$digest" "$(sha256sum "$tools/$binary" | cut -c1-64)" > "$record"

    printf 'fetch-tools: %s\n' "$("$tools/$binary" "$version_flag" | head -n 1)"
}

fetch_tool ffmpeg 'media tool' -version
fetch_tool c2patool 'credentials validator' --version
