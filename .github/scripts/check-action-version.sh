#!/usr/bin/env bash
# Check that the GitHub Action installs the version a release tag names:
# every `QGATE_VERSION:` line of action.yml must be `<version>` for the tag
# `v<version>` (the `releases` spec, design D7 of `add-github-action`).
#
# Usage: check-action-version.sh <tag> [action.yml]
# Exit codes: 0 when every line matches, 1 when one does not, 2 for a
# usage error or an action.yml without the lines.
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ] || [ -z "$1" ]; then
  echo "usage: $0 <tag> [action.yml]" >&2
  exit 2
fi
tag=$1
file=${2:-action.yml}
want=${tag#v}

versions=$(sed -n 's/^[[:space:]]*QGATE_VERSION:[[:space:]]*["'\'']\{0,1\}\([^"'\''[:space:]]*\).*$/\1/p' "$file")
if [ -z "$versions" ]; then
  echo "error: $file has no QGATE_VERSION line" >&2
  exit 2
fi

status=0
while IFS= read -r version; do
  if [ "$version" != "$want" ]; then
    echo "error: $file installs qgate $version, but the tag $tag releases $want; set every QGATE_VERSION in $file to $want" >&2
    status=1
  fi
done < <(sort -u <<<"$versions")
if [ "$status" -eq 0 ]; then
  echo "every QGATE_VERSION in $file is $want, as the tag $tag says"
fi
exit "$status"
