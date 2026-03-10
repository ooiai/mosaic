#!/usr/bin/env bash
set -euo pipefail

REPO="ooiai/mosaic"
GIT_URL="https://github.com/${REPO}.git"
VERSION=""
INSTALL_DIR="${MOSAIC_INSTALL_DIR:-$HOME/.local/bin}"
ASSETS_DIR=""
FROM_SOURCE=0
RELEASE_ONLY=0

usage() {
  cat <<USAGE
Install Mosaic CLI.

Usage:
  $0 [--version <tag>] [--install-dir <path>] [--assets-dir <path>] [--from-source] [--release-only]

Examples:
  $0
  $0 --version v0.2.0-beta.5
  $0 --version v0.2.0-beta.5 --assets-dir ./release-assets --release-only
  $0 --from-source
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
    --assets-dir)
      ASSETS_DIR="${2:-}"
      shift 2
      ;;
    --from-source)
      FROM_SOURCE=1
      shift 1
      ;;
    --release-only)
      RELEASE_ONLY=1
      shift 1
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

if [[ "$FROM_SOURCE" -eq 1 && "$RELEASE_ONLY" -eq 1 ]]; then
  echo "error: --from-source and --release-only cannot be used together" >&2
  usage >&2
  exit 1
fi

if [[ -n "$ASSETS_DIR" ]]; then
  if [[ ! -d "$ASSETS_DIR" ]]; then
    echo "error: --assets-dir does not exist: $ASSETS_DIR" >&2
    exit 1
  fi
  ASSETS_DIR="$(cd "$ASSETS_DIR" && pwd)"
fi

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: required command not found: $1" >&2
    exit 1
  fi
}

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
          echo "unsupported" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64) echo "linux-x64" ;;
        aarch64|arm64) echo "linux-arm64" ;;
        *)
          echo "unsupported" ;;
      esac
      ;;
    *)
      echo "unsupported" ;;
  esac
}

resolve_latest_version() {
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  local response
  if ! response="$(curl -fsSL "$api_url")"; then
    return 1
  fi
  local tag
  tag="$(printf '%s\n' "$response" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  [[ -n "$tag" ]] || return 1
  echo "$tag"
}

install_from_source() {
  require_cmd cargo
  local source_root="$TMP_DIR/cargo-root"
  mkdir -p "$source_root"
  local cmd=(cargo install --git "$GIT_URL" --locked --force --root "$source_root")
  if [[ -n "$VERSION" ]]; then
    cmd+=(--tag "$VERSION")
  fi
  cmd+=(mosaic-cli)
  echo "Installing mosaic from source"
  "${cmd[@]}"
  if [[ ! -x "$source_root/bin/mosaic" ]]; then
    echo "error: cargo install succeeded but mosaic binary not found" >&2
    exit 1
  fi
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$source_root/bin/mosaic" "$INSTALL_DIR/mosaic"
}

install_from_release() {
  require_cmd tar

  if [[ -z "$VERSION" ]]; then
    if [[ -n "$ASSETS_DIR" ]]; then
      echo "error: --version is required when using --assets-dir" >&2
      return 1
    fi
    VERSION="$(resolve_latest_version || true)"
    if [[ -z "$VERSION" ]]; then
      return 1
    fi
  fi

  local platform
  platform="$(detect_platform)"
  case "$platform" in
    darwin-arm64|darwin-x64|linux-x64) ;;
    *)
      return 1 ;;
  esac

  local asset="mosaic-${VERSION}-${platform}.tar.gz"
  if [[ -n "$ASSETS_DIR" ]]; then
    local local_asset="$ASSETS_DIR/$asset"
    if [[ ! -f "$local_asset" ]]; then
      local found
      found="$(find "$ASSETS_DIR" -maxdepth 3 -type f -name "$asset" | head -n 1 || true)"
      if [[ -n "$found" ]]; then
        local_asset="$found"
      else
        echo "error: release asset not found in --assets-dir: $asset" >&2
        return 1
      fi
    fi
    echo "Installing mosaic ${VERSION} (${platform}) from local assets"
    echo "Asset: $local_asset"
    cp "$local_asset" "$TMP_DIR/$asset"
  else
    require_cmd curl
    local url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
    echo "Installing mosaic ${VERSION} (${platform})"
    echo "Download: $url"
    if ! curl -fL "$url" -o "$TMP_DIR/$asset"; then
      return 1
    fi
  fi
  tar -xzf "$TMP_DIR/$asset" -C "$TMP_DIR"
  local extracted_dir="$TMP_DIR/mosaic-${VERSION}-${platform}"
  if [[ ! -x "$extracted_dir/mosaic" ]]; then
    return 1
  fi
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$extracted_dir/mosaic" "$INSTALL_DIR/mosaic"
}

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if [[ "$FROM_SOURCE" -eq 1 ]]; then
  install_from_source
else
  if ! install_from_release; then
    if [[ "$RELEASE_ONLY" -eq 1 ]]; then
      echo "error: release install failed and --release-only is set" >&2
      exit 1
    fi
    echo "warning: release asset install unavailable; falling back to source build" >&2
    install_from_source
  fi
fi

echo "Installed: $INSTALL_DIR/mosaic"
if command -v mosaic >/dev/null 2>&1; then
  echo "mosaic is now available on PATH."
else
  echo "Add to PATH if needed:"
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi
echo "Verify:"
echo "  mosaic --help"
