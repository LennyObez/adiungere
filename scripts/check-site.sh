#!/bin/sh
#
# Two properties of the published site that are cheap to check and expensive to notice late.
#
# The first is that the page loads nothing from another host. That is not a performance preference: this
# product tells people their footage never leaves their device, and a page that quietly fetches a typeface
# from somewhere else has already told a different story about what the project considers acceptable.
#
# The second is that the security contact has not expired. The standard that defines the file requires an
# expiry, and an expired contact is read by a scanner as no contact at all.

set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
published="$root/apps/site/public"
contact="$published/.well-known/security.txt"
failures=0

# How close to expiry the security contact may come before this refuses.
days_of_warning=45

report() {
    printf '%s\n' "$1" >&2
    failures=$((failures + 1))
}

# Seconds since the epoch for an RFC 3339 instant, on either family of `date`. The GNU form reads the whole
# string; the BSD form wants an explicit pattern and no fractional seconds. Prints nothing if neither reads it.
epoch_of() {
    instant=$1
    if date -u -d "$instant" +%s 2>/dev/null; then
        return 0
    fi
    date -j -u -f '%Y-%m-%dT%H:%M:%S' "${instant%%.*}" +%s 2>/dev/null || printf ''
}

if [ ! -d "$published" ]; then
    printf 'check-site: %s does not exist.\n' "$published" >&2
    exit 1
fi

pages=$(find "$published" -type f -name '*.html' | sort)

if [ -z "$pages" ]; then
    printf 'check-site: no page was found, so the checks below would pass by reading nothing.\n' >&2
    exit 1
fi

for page in $pages; do
    if grep -Eq 'src[[:space:]]*=[[:space:]]*["'"'"']https?:' "$page"; then
        report "check-site: $page loads a script, image or frame from another host."
    fi

    if grep -Eq '<link[^>]+rel[[:space:]]*=[[:space:]]*["'"'"'](stylesheet|preconnect|dns-prefetch|preload|modulepreload)' "$page"; then
        report "check-site: $page asks the browser to reach another host before it needs to."
    fi

    if grep -Eq '@import|url\([[:space:]]*["'"'"']?https?:' "$page"; then
        report "check-site: $page pulls a stylesheet or a resource from a URL."
    fi
done

if [ ! -f "$contact" ]; then
    report "check-site: $contact is missing, so the site publishes no security contact."
else
    expires=$(sed -n 's/^Expires:[[:space:]]*//p' "$contact" | head -n 1)

    if [ -z "$expires" ]; then
        report 'check-site: the security contact carries no expiry, which the standard requires.'
    else
        deadline=$(epoch_of "$expires")

        if [ -z "$deadline" ]; then
            report "check-site: the security contact expiry \"$expires\" is not a date this can read."
        else
            now=$(date -u +%s)
            remaining=$(( (deadline - now) / 86400 ))

            if [ "$remaining" -lt "$days_of_warning" ]; then
                report "check-site: the security contact expires in $remaining days. Extend it."
            fi
        fi
    fi
fi

if [ "$failures" -ne 0 ]; then
    printf '\n%d problem(s) with the published site.\n' "$failures" >&2
    exit 1
fi

printf 'The site loads nothing from another host, and its security contact is current.\n'
