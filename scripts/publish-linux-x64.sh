#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-0.0.0-local}"
COMMIT="${COMMIT:-dev}"
OUTPUT="${OUTPUT:-artifacts/linux-x64}"
RELEASE_BASE_URL="${RELEASE_BASE_URL:-}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_path="$repo_root/$OUTPUT"

dotnet publish "$repo_root/src/Dw.Cli/Dw.Cli.csproj" \
  --configuration Release \
  --runtime linux-x64 \
  --self-contained false \
  -p:PublishSingleFile=true \
  -p:DebugType=embedded \
  -p:VersionPrefix="$VERSION" \
  -p:SourceRevisionId="$COMMIT" \
  --output "$output_path"

exe="$output_path/dw"
hash="$(sha256sum "$exe" | awk '{print $1}')"
url=""
if [[ -n "$RELEASE_BASE_URL" ]]; then
  url="$RELEASE_BASE_URL/dw"
fi

cat > "$output_path/release.json" <<EOF
{
  "schema": 1,
  "version": "$VERSION",
  "commit": "$COMMIT",
  "channel": "stable",
  "assets": [
    {
      "rid": "linux-x64",
      "fileName": "dw",
      "sha256": "$hash",
      "url": "$url"
    }
  ]
}
EOF

echo "Published $exe"
echo "SHA256 $hash"
