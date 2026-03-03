#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> --assets-dir <dir> [--output-dir <dir>]

Examples:
  $0 --version v0.2.0-beta.2 --assets-dir ./release-assets
  $0 --version v0.2.0-beta.2 --assets-dir ./release-assets --output-dir ./release-assets
USAGE
}

VERSION=""
ASSETS_DIR=""
OUTPUT_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --assets-dir)
      ASSETS_DIR="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
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

if [[ -z "$VERSION" || -z "$ASSETS_DIR" ]]; then
  echo "error: --version and --assets-dir are required" >&2
  usage >&2
  exit 1
fi

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$CLI_DIR/.." && pwd)"
if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR="$ROOT_DIR/packaging"
fi

ASSETS_DIR="$(cd "$ASSETS_DIR" && pwd)"
mkdir -p "$OUTPUT_DIR/homebrew" "$OUTPUT_DIR/scoop"

find_asset() {
  local base="$1"
  if [[ -f "$ASSETS_DIR/$base" ]]; then
    echo "$ASSETS_DIR/$base"
    return 0
  fi
  local found
  found="$(find "$ASSETS_DIR" -maxdepth 3 -type f -name "$base" | head -n 1 || true)"
  if [[ -n "$found" ]]; then
    echo "$found"
    return 0
  fi
  return 1
}

compute_sha256() {
  local file_path="$1"
  if [[ -f "${file_path}.sha256" ]]; then
    awk '{print $1}' "${file_path}.sha256"
    return
  fi
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

asset_or_fail() {
  local base="$1"
  local path
  path="$(find_asset "$base" || true)"
  if [[ -z "$path" ]]; then
    echo "error: required asset not found: $base (searched under $ASSETS_DIR)" >&2
    exit 1
  fi
  echo "$path"
}

DARWIN_ARM_ASSET="mosaic-${VERSION}-darwin-arm64.tar.gz"
DARWIN_X64_ASSET="mosaic-${VERSION}-darwin-x64.tar.gz"
LINUX_X64_ASSET="mosaic-${VERSION}-linux-x64.tar.gz"
WINDOWS_X64_ASSET="mosaic-${VERSION}-windows-x64.zip"

DARWIN_ARM_PATH="$(asset_or_fail "$DARWIN_ARM_ASSET")"
DARWIN_X64_PATH="$(asset_or_fail "$DARWIN_X64_ASSET")"
LINUX_X64_PATH="$(asset_or_fail "$LINUX_X64_ASSET")"
WINDOWS_X64_PATH="$(asset_or_fail "$WINDOWS_X64_ASSET")"

DARWIN_ARM_SHA="$(compute_sha256 "$DARWIN_ARM_PATH")"
DARWIN_X64_SHA="$(compute_sha256 "$DARWIN_X64_PATH")"
LINUX_X64_SHA="$(compute_sha256 "$LINUX_X64_PATH")"
WINDOWS_X64_SHA="$(compute_sha256 "$WINDOWS_X64_PATH")"

VERSION_STRIPPED="${VERSION#v}"
DOWNLOAD_BASE="https://github.com/ooiai/mosaic/releases/download/${VERSION}"

HOMEBREW_OUT="$OUTPUT_DIR/homebrew/mosaic.rb"
cat > "$HOMEBREW_OUT" <<RUBY
class Mosaic < Formula
  desc "Mosaic CLI local agent runtime"
  homepage "https://github.com/ooiai/mosaic"
  version "${VERSION_STRIPPED}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "${DOWNLOAD_BASE}/${DARWIN_ARM_ASSET}"
      sha256 "${DARWIN_ARM_SHA}"
    else
      url "${DOWNLOAD_BASE}/${DARWIN_X64_ASSET}"
      sha256 "${DARWIN_X64_SHA}"
    end
  end

  on_linux do
    url "${DOWNLOAD_BASE}/${LINUX_X64_ASSET}"
    sha256 "${LINUX_X64_SHA}"
  end

  def install
    bin.install "mosaic"
  end

  test do
    assert_match "mosaic", shell_output("#{bin}/mosaic --help")
  end
end
RUBY

SCOOP_OUT="$OUTPUT_DIR/scoop/mosaic.json"
cat > "$SCOOP_OUT" <<JSON
{
  "version": "${VERSION_STRIPPED}",
  "description": "Mosaic CLI local agent runtime",
  "homepage": "https://github.com/ooiai/mosaic",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "${DOWNLOAD_BASE}/${WINDOWS_X64_ASSET}",
      "hash": "${WINDOWS_X64_SHA}"
    }
  },
  "bin": "mosaic.exe",
  "checkver": {
    "github": "https://github.com/ooiai/mosaic"
  },
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/ooiai/mosaic/releases/download/\$tag/mosaic-\$tag-windows-x64.zip"
      }
    }
  }
}
JSON

CHECKSUMS_OUT="$OUTPUT_DIR/SHA256SUMS"
{
  echo "${DARWIN_ARM_SHA}  ${DARWIN_ARM_ASSET}"
  echo "${DARWIN_X64_SHA}  ${DARWIN_X64_ASSET}"
  echo "${LINUX_X64_SHA}  ${LINUX_X64_ASSET}"
  echo "${WINDOWS_X64_SHA}  ${WINDOWS_X64_ASSET}"
} > "$CHECKSUMS_OUT"

echo "generated:"
echo "- $HOMEBREW_OUT"
echo "- $SCOOP_OUT"
echo "- $CHECKSUMS_OUT"
