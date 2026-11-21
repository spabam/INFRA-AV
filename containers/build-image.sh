#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
if (( $# > 1 )); then
  printf 'Usage: %s [image-repository]\n' "$0" >&2
  exit 2
fi
repository="${1:-vantage/antivirus}"
bash "$root/scripts/version.sh" check
release="$(bash "$root/scripts/version.sh" value)"
exec docker build --file "$root/containers/Dockerfile" \
  --label "org.opencontainers.image.version=$release" --tag "$repository:$release" "$root"
