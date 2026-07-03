#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:?VERSION is required}"
COMMIT="${COMMIT:?COMMIT is required}"
RELEASE_BASE_URL="${RELEASE_BASE_URL:?RELEASE_BASE_URL is required}"
OUTPUT="${OUTPUT:-artifacts/release.json}"
LINUX_ASSET="${LINUX_ASSET:-artifacts/dw-linux-x64/dw-linux-x64.tar.gz}"
WINDOWS_ASSET="${WINDOWS_ASSET:-artifacts/dw-win-x64/dw-win-x64.zip}"

linux_hash="$(sha256sum "$LINUX_ASSET" | awk '{print $1}')"
windows_hash="$(sha256sum "$WINDOWS_ASSET" | awk '{print $1}')"
mkdir -p "$(dirname "$OUTPUT")"

cat > "$OUTPUT" <<EOF
{
  "schema": 1,
  "version": "$VERSION",
  "commit": "$COMMIT",
  "channel": "stable",
  "assets": [
    {
      "rid": "linux-x64",
      "fileName": "dw-linux-x64.tar.gz",
      "sha256": "$linux_hash",
      "url": "$RELEASE_BASE_URL/dw-linux-x64.tar.gz"
    },
    {
      "rid": "win-x64",
      "fileName": "dw-win-x64.zip",
      "sha256": "$windows_hash",
      "url": "$RELEASE_BASE_URL/dw-win-x64.zip"
    }
  ]
}
EOF

echo "Wrote $OUTPUT"
