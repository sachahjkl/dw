#!/bin/sh
set -eu

repository="${DW_REPOSITORY:-sachahjkl/dw}"
version="${DW_VERSION:-latest}"
install_dir="${DW_INSTALL_DIR:-$HOME/.local/bin}"
no_path_update="${DW_NO_PATH_UPDATE:-0}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repository)
      repository="$2"
      shift 2
      ;;
    --version)
      version="$2"
      shift 2
      ;;
    --install-dir)
      install_dir="$2"
      shift 2
      ;;
    --no-path-update)
      no_path_update="1"
      shift
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 2
      ;;
  esac
done

ensure_line() {
  file="$1"
  line="$2"
  dir="$(dirname "$file")"
  mkdir -p "$dir"
  touch "$file"
  if ! grep -Fqx "$line" "$file"; then
    {
      echo ""
      echo "# dw"
      echo "$line"
    } >> "$file"
    echo "Updated $file"
  else
    echo "PATH already configured in $file"
  fi
}

update_shell_path() {
  export PATH="$install_dir:$PATH"

  shell_name="$(basename "${SHELL:-}")"
  case "$shell_name" in
    bash)
      ensure_line "$HOME/.bashrc" "export PATH=\"$install_dir:\$PATH\""
      ;;
    zsh)
      ensure_line "$HOME/.zshrc" "export PATH=\"$install_dir:\$PATH\""
      ;;
    fish)
      ensure_line "$HOME/.config/fish/config.fish" "fish_add_path \"$install_dir\""
      ;;
    nu)
      ensure_line "$HOME/.config/nushell/env.nu" "\$env.PATH = ([\"$install_dir\"] | append \$env.PATH)"
      ;;
    pwsh|powershell)
      ensure_line "$HOME/.config/powershell/Microsoft.PowerShell_profile.ps1" "\$env:PATH = \"$install_dir:\$env:PATH\""
      ;;
    *)
      if [ -n "${ZSH_VERSION:-}" ]; then
        ensure_line "$HOME/.zshrc" "export PATH=\"$install_dir:\$PATH\""
      elif [ -n "${BASH_VERSION:-}" ]; then
        ensure_line "$HOME/.bashrc" "export PATH=\"$install_dir:\$PATH\""
      else
        ensure_line "$HOME/.profile" "export PATH=\"$install_dir:\$PATH\""
      fi
      ;;
  esac

  echo "PATH for current session updated."
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_command curl
require_command tar
require_command mktemp

if command -v sha256sum >/dev/null 2>&1; then
  sha256_file() {
    sha256sum "$1" | awk '{print $1}'
  }
elif command -v shasum >/dev/null 2>&1; then
  sha256_file() {
    shasum -a 256 "$1" | awk '{print $1}'
  }
else
  echo "Missing required command: sha256sum or shasum" >&2
  exit 1
fi

mkdir -p "$install_dir"

if [ "$version" = "latest" ]; then
  release_base_url="https://github.com/$repository/releases/latest/download"
else
  case "$version" in
    v*) release_tag="$version" ;;
    *) release_tag="v$version" ;;
  esac
  release_base_url="https://github.com/$repository/releases/download/$release_tag"
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/dw-install.XXXXXX")"
candidate=""
cleanup() {
  rm -rf "$tmp"
  if [ -n "$candidate" ]; then
    rm -f "$candidate"
  fi
}
trap cleanup 0
trap 'exit 1' 1 2 15

archive="$tmp/dw-linux-x64.tar.gz"
manifest="$tmp/release.json"
echo "Downloading $release_base_url/release.json..."
curl -fsSL "$release_base_url/release.json" -o "$manifest"

asset_fields="$(awk '
function field(name, value) {
  if (!match($0, "\"" name "\"[[:space:]]*:[[:space:]]*\"")) return ""
  value = substr($0, RSTART + RLENGTH)
  sub(/\".*/, "", value)
  return value
}
BEGIN { RS = "{" }
field("rid") == "linux-x64" { print field("fileName"), field("sha256") }
' "$manifest")"
set -- $asset_fields
if [ "$#" -ne 2 ] || [ "$1" != "dw-linux-x64.tar.gz" ]; then
  echo "release.json does not contain exactly one supported linux-x64 asset" >&2
  exit 1
fi
asset_name="$1"
expected_sha256="$(printf '%s' "$2" | tr 'A-F' 'a-f')"
case "$expected_sha256" in
  *[!0-9a-f]*|'')
    echo "release.json contains an invalid SHA-256" >&2
    exit 1
    ;;
esac
if [ "${#expected_sha256}" -ne 64 ]; then
  echo "release.json contains an invalid SHA-256" >&2
  exit 1
fi

asset_url="$release_base_url/$asset_name"
echo "Downloading $asset_url..."
curl -fsSL "$asset_url" -o "$archive"
actual_sha256="$(sha256_file "$archive" | tr 'A-F' 'a-f')"
if [ "$actual_sha256" != "$expected_sha256" ]; then
  echo "SHA-256 mismatch for $asset_name" >&2
  exit 1
fi

entries="$(LC_ALL=C tar -tzf "$archive")"
listing="$(LC_ALL=C tar -tvzf "$archive")"
if [ "$entries" != "dw" ]; then
  echo "Archive must contain exactly one entry named dw" >&2
  exit 1
fi
case "$listing" in
  -*) ;;
  *)
    echo "Archive entry dw is not a regular file" >&2
    exit 1
    ;;
esac

extract_dir="$tmp/extract"
mkdir "$extract_dir"
tar -xzf "$archive" -C "$extract_dir" dw
if [ ! -f "$extract_dir/dw" ] || [ -L "$extract_dir/dw" ]; then
  echo "Extracted dw is not a regular file" >&2
  exit 1
fi
chmod 755 "$extract_dir/dw"
"$extract_dir/dw" version >/dev/null

candidate="$(mktemp "$install_dir/.dw.install.XXXXXX")"
cp "$extract_dir/dw" "$candidate"
chmod 755 "$candidate"
"$candidate" version >/dev/null
mv -f "$candidate" "$install_dir/dw"
candidate=""

echo "dw installed in $install_dir"

if [ "$no_path_update" != "1" ]; then
  update_shell_path
fi

"$install_dir/dw" version
