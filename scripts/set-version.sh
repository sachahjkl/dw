#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 1 ]]; then
  echo "Usage: $0 [version]" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -eq 0 ]]; then
  base="$(date -u +'%Y.%m.%d')"
  count="$(git -C "$repo_root" tag --list "v${base}.*" | wc -l | tr -d ' ')"
  build_id="$((count + 1))"
  version="${base}.${build_id}"
else
  version="$1"
fi

if [[ ! "$version" =~ ^[0-9]+(\.[0-9]+)+$ ]]; then
  echo "Invalid version: $version" >&2
  echo "Expected a numeric dotted version, for example: 2026.06.20.1" >&2
  exit 2
fi

printf '%s\n' "$version" > "$repo_root/VERSION"
echo "Updated VERSION to $version"
