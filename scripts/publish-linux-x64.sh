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
  DW_COMMIT="$COMMIT" cargo build --locked --release -p dw-cli
)

cp "$repo_root/target/release/dw-cli" "$output_path/dw"
chmod 755 "$output_path/dw"

if grep -aFq '/nix/store' "$output_path/dw"; then
  echo "Refusing to package a binary containing /nix/store references" >&2
  exit 1
fi

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
