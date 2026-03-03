#!/usr/bin/env bash
set -euo pipefail

REPO="ooiai/mosaic"
VERSION=""
INSTALL_DIR="${MOSAIC_INSTALL_DIR:-$HOME/.local/bin}"

usage() {
  cat <<USAGE
Install Mosaic CLI from GitHub Releases.

Usage:
  $0 [--version <tag>] [--install-dir <path>]

Examples:
  $0
  $0 --version v0.2.0-beta.2
  $0 --install-dir /usr/local/bin
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument '$1'" >&2
      usage >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd tar

detect_platform() {
  local os
  local arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "darwin-arm64" ;;
        x86_64) echo "darwin-x64" ;;
        *)
          echo "error: unsupported macOS architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64) echo "linux-x64" ;;
        aarch64|arm64) echo "linux-arm64" ;;
        *)
          echo "error: unsupported Linux architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "error: unsupported operating system: $os" >&2
      exit 1
      ;;
  esac
}

resolve_latest_version() {
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  local response
  response="$(curl -fsSL "$api_url")"
  local tag
  tag="$(printf '%s\n' "$response" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  if [[ -z "$tag" ]]; then
    echo "error: failed to resolve latest release tag from $api_url" >&2
    echo "hint: pass --version <tag> explicitly" >&2
    exit 1
  fi
  echo "$tag"
}

PLATFORM="$(detect_platform)"
if [[ -z "$VERSION" ]]; then
  VERSION="$(resolve_latest_version)"
fi

ASSET="mosaic-${VERSION}-${PLATFORM}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Installing mosaic ${VERSION} (${PLATFORM})"
echo "Download: $URL"

curl -fL "$URL" -o "$TMP_DIR/$ASSET"
tar -xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR"

EXTRACTED_DIR="$TMP_DIR/mosaic-${VERSION}-${PLATFORM}"
if [[ ! -x "$EXTRACTED_DIR/mosaic" ]]; then
  echo "error: extracted package does not contain executable 'mosaic'" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 0755 "$EXTRACTED_DIR/mosaic" "$INSTALL_DIR/mosaic"

echo "Installed: $INSTALL_DIR/mosaic"
if command -v mosaic >/dev/null 2>&1; then
  echo "mosaic is now available on PATH."
else
  echo "Add to PATH if needed:"
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo "Verify:"
echo "  mosaic --help"
