#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> --target <rust-target-triple>

Examples:
  $0 --version v0.2.0-beta.2 --target x86_64-unknown-linux-gnu
  $0 --version v0.2.0-beta.2 --target aarch64-apple-darwin
  $0 --version v0.2.0-beta.2 --target x86_64-pc-windows-msvc
USAGE
}

VERSION=""
TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --target)
      TARGET="${2:-}"
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

if [[ -z "$VERSION" || -z "$TARGET" ]]; then
  echo "error: --version and --target are required" >&2
  usage >&2
  exit 1
fi

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CLI_DIR"

target_to_platform() {
  case "$1" in
    aarch64-apple-darwin) echo "darwin-arm64" ;;
    x86_64-apple-darwin) echo "darwin-x64" ;;
    x86_64-unknown-linux-gnu) echo "linux-x64" ;;
    aarch64-unknown-linux-gnu) echo "linux-arm64" ;;
    x86_64-pc-windows-msvc) echo "windows-x64" ;;
    aarch64-pc-windows-msvc) echo "windows-arm64" ;;
    *)
      echo "error: unsupported release target '$1'" >&2
      exit 1
      ;;
  esac
}

sha256_file() {
  local file_path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" | awk '{print $1}'
    return
  fi
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" | awk '{print $1}'
    return
  fi
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file_path" | awk '{print $NF}'
    return
  fi
  echo "error: no SHA256 tool found (sha256sum/shasum/openssl)" >&2
  exit 1
}

to_windows_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    echo "$1"
  fi
}

PLATFORM="$(target_to_platform "$TARGET")"
IS_WINDOWS=0
BIN_NAME="mosaic"
ASSET_EXT="tar.gz"
if [[ "$PLATFORM" == windows-* ]]; then
  IS_WINDOWS=1
  BIN_NAME="mosaic.exe"
  ASSET_EXT="zip"
fi

RELEASE_BIN="$CLI_DIR/target/$TARGET/release/$BIN_NAME"
if [[ ! -f "$RELEASE_BIN" ]]; then
  echo "error: release binary not found at $RELEASE_BIN" >&2
  echo "hint: run cargo build --release -p mosaic-cli --target $TARGET" >&2
  exit 1
fi

DIST_DIR="$CLI_DIR/dist/$VERSION"
ASSET_BASE="mosaic-${VERSION}-${PLATFORM}"
PKG_DIR="$DIST_DIR/$ASSET_BASE"
ASSET_PATH="$DIST_DIR/${ASSET_BASE}.${ASSET_EXT}"
mkdir -p "$PKG_DIR"

cp "$RELEASE_BIN" "$PKG_DIR/"
cp "$CLI_DIR/README.md" "$PKG_DIR/"
cp "$CLI_DIR/install.sh" "$PKG_DIR/"
cp "$CLI_DIR/install.ps1" "$PKG_DIR/"
cp "$CLI_DIR/docs/parity-map.md" "$PKG_DIR/"
cp "$CLI_DIR/docs/regression-runbook.md" "$PKG_DIR/"
cp "$CLI_DIR/docs/distribution.md" "$PKG_DIR/"

if [[ "$IS_WINDOWS" == "1" ]]; then
  if command -v zip >/dev/null 2>&1; then
    (
      cd "$DIST_DIR"
      rm -f "$ASSET_PATH"
      zip -rq "${ASSET_BASE}.zip" "$ASSET_BASE"
    )
  else
    PKG_DIR_WIN="$(to_windows_path "$PKG_DIR")"
    ASSET_PATH_WIN="$(to_windows_path "$ASSET_PATH")"
    pwsh -NoLogo -NoProfile -Command \
      "Compress-Archive -Path '${PKG_DIR_WIN}\\*' -DestinationPath '${ASSET_PATH_WIN}' -Force"
  fi
else
  (
    cd "$DIST_DIR"
    rm -f "$ASSET_PATH"
    tar -czf "${ASSET_BASE}.tar.gz" "$ASSET_BASE"
  )
fi

HASH="$(sha256_file "$ASSET_PATH")"
echo "$HASH  $(basename "$ASSET_PATH")" > "${ASSET_PATH}.sha256"

echo "target: $TARGET"
echo "platform: $PLATFORM"
echo "package_dir: $PKG_DIR"
echo "asset: $ASSET_PATH"
echo "sha256: $HASH"
