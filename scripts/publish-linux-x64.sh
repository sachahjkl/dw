#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-0.0.0-local}"
COMMIT="${COMMIT:-dev}"
OUTPUT="${OUTPUT:-artifacts/linux-x64}"
RELEASE_BASE_URL="${RELEASE_BASE_URL:-}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
if [[ "$OUTPUT" = /* ]]; then
  output_path="$OUTPUT"
else
  output_path="$repo_root/$OUTPUT"
fi
archive_name="dw-linux-x64.tar.gz"
archive_path="$output_path/$archive_name"

mkdir -p "$output_path"

(
  cd "$repo_root"
  CGO_ENABLED=0 \
  GOOS=linux \
  GOARCH=amd64 \
  GOTOOLCHAIN=local \
    go build \
      -trimpath \
      -buildvcs=false \
      -tags timetzdata \
      -ldflags "-s -w -X github.com/sachahjkl/dw/internal/buildinfo.Version=$VERSION -X github.com/sachahjkl/dw/internal/buildinfo.Commit=$COMMIT" \
      -o "$output_path/dw" \
      ./cmd/dw
)
chmod 755 "$output_path/dw"

nix_path_pattern='/nix/store/[0-9a-z]{32}-mailcap-[-A-Za-z0-9._+?=]+/etc/mime\.types|/nix/store/[0-9a-z]{32}-tzdata-[-A-Za-z0-9._+?=]+/share/zoneinfo|/nix/store/[0-9a-z]{32}-iana-etc-[-A-Za-z0-9._+?=]+/|/nix/store/[0-9a-z]{32}-[-A-Za-z0-9._+?=]+/'
while IFS= read -r -d '' nix_path; do
  if [[ "$nix_path" =~ ^/nix/store/[0-9a-z]{32}-mailcap-[^/]+/etc/mime\.types$ \
    || "$nix_path" =~ ^/nix/store/[0-9a-z]{32}-tzdata-[^/]+/share/zoneinfo$ \
    || "$nix_path" =~ ^/nix/store/[0-9a-z]{32}-iana-etc-[^/]+/$ ]]; then
    continue
  fi
  echo "Refusing to package a binary containing concrete Nix store reference: $nix_path" >&2
  exit 1
done < <(grep -aozE "$nix_path_pattern" "$output_path/dw" || true)

"$output_path/dw" version >/dev/null
tar -czf "$archive_path" -C "$output_path" dw

hash="$(sha256sum "$archive_path" | awk '{print $1}')"
url=""
if [[ -n "$RELEASE_BASE_URL" ]]; then
  url="$RELEASE_BASE_URL/$archive_name"
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
      "fileName": "$archive_name",
      "sha256": "$hash",
      "url": "$url"
    }
  ]
}
EOF

echo "Published $archive_path"
echo "SHA256 $hash"
