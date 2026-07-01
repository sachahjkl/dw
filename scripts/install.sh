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

mkdir -p "$install_dir"

if [ "$version" = "latest" ]; then
  asset_url="https://github.com/$repository/releases/latest/download/dw-linux-x64.tar.gz"
else
  asset_url="https://github.com/$repository/releases/download/$version/dw-linux-x64.tar.gz"
fi

tmp="${TMPDIR:-/tmp}/dw-install-$$"
mkdir -p "$tmp"
trap 'rm -rf "$tmp"' EXIT INT TERM

archive="$tmp/dw-linux-x64.tar.gz"
echo "Downloading $asset_url..."
curl -fsSL "$asset_url" -o "$archive"
tar -xzf "$archive" -C "$install_dir"
chmod +x "$install_dir/dw"

echo "dw installed in $install_dir"

if [ "$no_path_update" != "1" ]; then
  update_shell_path
fi

"$install_dir/dw" version
