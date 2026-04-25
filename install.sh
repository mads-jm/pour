#!/usr/bin/env sh
# Pour installer for Linux / macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mads-jm/pour/main/install.sh | sh
#
# Optional: pass a version, or set POUR_VERSION, to pin a specific release.
#   curl -fsSL ...install.sh | sh -s -- 0.2.2

set -eu

REPO="mads-jm/pour"
INSTALL_DIR="${POUR_INSTALL_DIR:-$HOME/.local/share/pour}"
BIN_DIR="${POUR_BIN_DIR:-$HOME/.local/bin}"

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
case "$os-$arch" in
  linux-x86_64)           target="x86_64-unknown-linux-gnu" ;;
  darwin-arm64|darwin-aarch64) target="aarch64-apple-darwin" ;;
  *)
    printf 'Unsupported platform: %s-%s\n' "$os" "$arch" >&2
    printf 'Prebuilt targets: linux x86_64, macOS arm64.\n' >&2
    printf 'Build from source: cargo install --git https://github.com/%s\n' "$REPO" >&2
    exit 1 ;;
esac

version="${1-${POUR_VERSION-}}"
if [ -z "$version" ]; then
  printf 'Looking up latest release...\n'
  version=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep -m1 '"tag_name"' \
    | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/')
  if [ -z "$version" ]; then
    printf 'Failed to determine latest version.\n' >&2
    exit 1
  fi
fi
# Accept both "v0.2.2" and "0.2.2"
num="${version#v}"
tag="v$num"

asset="pour-$num-$target.tar.gz"
url="https://github.com/$REPO/releases/download/$tag/$asset"

printf 'Installing pour %s to %s\n' "$tag" "$INSTALL_DIR"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

printf 'Downloading %s\n' "$url"
curl -fsSL "$url" -o "$tmp/$asset"

printf 'Extracting...\n'
tar -xzf "$tmp/$asset" -C "$tmp"

extracted=$(find "$tmp" -maxdepth 1 -type d -name 'pour-*' | head -n 1)
if [ -z "$extracted" ]; then
  printf 'Archive layout unexpected: no pour-* folder in %s\n' "$tmp" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR" "$BIN_DIR"
# Clear previous install but don't rm -rf the dir itself — user may have it open.
find "$INSTALL_DIR" -mindepth 1 -delete 2>/dev/null || true
cp -R "$extracted"/. "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/pour"

ln -sfn "$INSTALL_DIR/pour" "$BIN_DIR/pour"

printf '\npour %s installed.\n' "$tag"
printf '  binary:    %s/pour\n' "$BIN_DIR"
printf '  resources: %s/resources\n' "$INSTALL_DIR"

case ":$PATH:" in
  *":$BIN_DIR:"*)
    printf '\nRun `pour` to get started.\n' ;;
  *)
    printf '\nWARN: %s is not on your PATH.\n' "$BIN_DIR"
    printf 'Add this to your shell rc (~/.bashrc, ~/.zshrc, etc.):\n'
    printf '  export PATH="$HOME/.local/bin:$PATH"\n' ;;
esac
