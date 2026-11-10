#!/usr/bin/env bash
# Check that a release tag names the workspace version, as `v<version>`
# (the `releases` spec, design D3 of `add-release-binaries`).
#
# Usage: check-version.sh <tag>
# Exit codes: 0 when the tag matches, 1 when it does not, 2 for a usage
# error or a version that cannot be read.
set -euo pipefail

if [ "$#" -ne 1 ] || [ -z "$1" ]; then
  echo "usage: $0 <tag>" >&2
  exit 2
fi
tag=$1

version=$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "qgate-cli") | .version')
if [ -z "$version" ]; then
  echo "error: cannot read the version of qgate-cli from cargo metadata" >&2
  exit 2
fi

if [ "$tag" != "v$version" ]; then
  echo "error: the tag $tag does not match the workspace version $version; tag v$version to release it" >&2
  exit 1
fi
echo "the tag $tag matches the workspace version $version"
