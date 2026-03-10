#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 [--version <tag>]

Builds temporary release-asset fixtures and validates:
  - release_verify_archives.sh pass/fail paths
  - release_verify_assets.sh pass/fail paths
  - release_prepare.sh dry-run summary output
USAGE
}

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

resolve_default_version() {
  local cargo_toml="$CLI_DIR/crates/mosaic-cli/Cargo.toml"
  local parsed=""
  if [[ -f "$cargo_toml" ]]; then
    parsed="$(awk -F'"' '/^version[[:space:]]*=[[:space:]]*"/ { print $2; exit }' "$cargo_toml" || true)"
  fi
  if [[ -n "$parsed" ]]; then
    printf 'v%s' "$parsed"
  else
    printf 'v0.2.0-beta.6'
  fi
}

VERSION="$(resolve_default_version)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
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

for required_cmd in tar zip awk grep; do
  if ! command -v "$required_cmd" >/dev/null 2>&1; then
    echo "error: required command '$required_cmd' is not available in PATH" >&2
    exit 1
  fi
done

TMP_DIR="$(mktemp -d)"
ASSETS_DIR="$TMP_DIR/release-assets"
mkdir -p "$ASSETS_DIR"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

hash_file() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return
  fi
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
    return
  fi
  echo "error: no sha256 tool found (sha256sum/shasum)" >&2
  return 1
}

create_archive() {
  local platform="$1"
  local ext="$2"
  local binary_name="$3"
  local root="$ASSETS_DIR/mosaic-${VERSION}-${platform}"

  mkdir -p "$root"
  printf 'binary\n' > "$root/$binary_name"
  printf 'doc\n' > "$root/README.md"
  printf 'doc\n' > "$root/install.sh"
  printf 'doc\n' > "$root/install.ps1"
  printf 'doc\n' > "$root/parity-map.md"
  printf 'doc\n' > "$root/regression-runbook.md"
  printf 'doc\n' > "$root/distribution.md"

  if [[ "$ext" == "tar.gz" ]]; then
    (cd "$ASSETS_DIR" && tar -czf "mosaic-${VERSION}-${platform}.tar.gz" "mosaic-${VERSION}-${platform}")
  else
    (cd "$ASSETS_DIR" && zip -qr "mosaic-${VERSION}-${platform}.zip" "mosaic-${VERSION}-${platform}")
  fi

  rm -rf "$root"
}

create_archive "darwin-arm64" "tar.gz" "mosaic"
create_archive "darwin-x64" "tar.gz" "mosaic"
create_archive "linux-x64" "tar.gz" "mosaic"
create_archive "windows-x64" "zip" "mosaic.exe"

darwin_arm64_archive="mosaic-${VERSION}-darwin-arm64.tar.gz"
darwin_x64_archive="mosaic-${VERSION}-darwin-x64.tar.gz"
linux_x64_archive="mosaic-${VERSION}-linux-x64.tar.gz"
windows_x64_archive="mosaic-${VERSION}-windows-x64.zip"

darwin_arm64_hash="$(hash_file "$ASSETS_DIR/$darwin_arm64_archive")"
darwin_x64_hash="$(hash_file "$ASSETS_DIR/$darwin_x64_archive")"
linux_x64_hash="$(hash_file "$ASSETS_DIR/$linux_x64_archive")"
windows_x64_hash="$(hash_file "$ASSETS_DIR/$windows_x64_archive")"

printf '%s\n' "$darwin_arm64_hash" > "$ASSETS_DIR/$darwin_arm64_archive.sha256"
printf '%s\n' "$darwin_x64_hash" > "$ASSETS_DIR/$darwin_x64_archive.sha256"
printf '%s\n' "$linux_x64_hash" > "$ASSETS_DIR/$linux_x64_archive.sha256"
printf '%s\n' "$windows_x64_hash" > "$ASSETS_DIR/$windows_x64_archive.sha256"

cat > "$ASSETS_DIR/SHA256SUMS" <<SUMS
$darwin_arm64_hash  $darwin_arm64_archive
$darwin_x64_hash  $darwin_x64_archive
$linux_x64_hash  $linux_x64_archive
$windows_x64_hash  $windows_x64_archive
SUMS

cat > "$ASSETS_DIR/mosaic.rb" <<FORMULA
class Mosaic < Formula
  version "${VERSION#v}"
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-darwin-arm64.tar.gz"
      sha256 "$darwin_arm64_hash"
    else
      url "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-darwin-x64.tar.gz"
      sha256 "$darwin_x64_hash"
    end
  end

  on_linux do
    url "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-linux-x64.tar.gz"
    sha256 "$linux_x64_hash"
  end
end
FORMULA

cat > "$ASSETS_DIR/mosaic.json" <<JSON
{
  "version": "${VERSION#v}",
  "url": "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-windows-x64.zip",
  "hash": "$windows_x64_hash"
}
JSON

printf '#!/usr/bin/env bash\n' > "$ASSETS_DIR/install.sh"
printf 'Write-Output "mosaic"\n' > "$ASSETS_DIR/install.ps1"

assert_contains() {
  local file="$1"
  local pattern="$2"
  if ! grep -Fq "$pattern" "$file"; then
    echo "error: expected pattern not found: $pattern" >&2
    echo "file: $file" >&2
    return 1
  fi
}

ARCHIVE_OK_JSON="$TMP_DIR/verify_archives_ok.json"
ASSET_OK_JSON="$TMP_DIR/verify_assets_ok.json"

"$CLI_DIR/scripts/release_verify_archives.sh" --version "$VERSION" --assets-dir "$ASSETS_DIR" --json > "$ARCHIVE_OK_JSON"
assert_contains "$ARCHIVE_OK_JSON" '"ok": true'

"$CLI_DIR/scripts/release_verify_assets.sh" --version "$VERSION" --assets-dir "$ASSETS_DIR" --json > "$ASSET_OK_JSON"
assert_contains "$ASSET_OK_JSON" '"ok": true'

BAD_ARCHIVE_DIR="$TMP_DIR/release-assets-bad-archive"
mkdir -p "$BAD_ARCHIVE_DIR"
cp "$ASSETS_DIR"/* "$BAD_ARCHIVE_DIR"/
mkdir -p "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64"
printf 'doc\n' > "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64/README.md"
printf 'doc\n' > "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64/install.sh"
printf 'doc\n' > "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64/install.ps1"
printf 'doc\n' > "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64/parity-map.md"
printf 'doc\n' > "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64/regression-runbook.md"
printf 'doc\n' > "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64/distribution.md"
(cd "$BAD_ARCHIVE_DIR" && tar -czf "mosaic-${VERSION}-linux-x64.tar.gz" "mosaic-${VERSION}-linux-x64")
rm -rf "$BAD_ARCHIVE_DIR/mosaic-${VERSION}-linux-x64"

set +e
"$CLI_DIR/scripts/release_verify_archives.sh" --version "$VERSION" --assets-dir "$BAD_ARCHIVE_DIR" --json > "$TMP_DIR/verify_archives_bad.json" 2>&1
bad_archive_exit=$?
set -e
if [[ "$bad_archive_exit" -eq 0 ]]; then
  echo "error: expected release_verify_archives.sh to fail on bad archive" >&2
  exit 1
fi
assert_contains "$TMP_DIR/verify_archives_bad.json" 'archive missing binary'

BAD_ASSET_DIR="$TMP_DIR/release-assets-bad-assets"
mkdir -p "$BAD_ASSET_DIR"
cp "$ASSETS_DIR"/* "$BAD_ASSET_DIR"/
rm -f "$BAD_ASSET_DIR/mosaic-${VERSION}-windows-x64.zip.sha256"

set +e
"$CLI_DIR/scripts/release_verify_assets.sh" --version "$VERSION" --assets-dir "$BAD_ASSET_DIR" --json > "$TMP_DIR/verify_assets_bad.json" 2>&1
bad_asset_exit=$?
set -e
if [[ "$bad_asset_exit" -eq 0 ]]; then
  echo "error: expected release_verify_assets.sh to fail on missing checksum sidecar" >&2
  exit 1
fi
assert_contains "$TMP_DIR/verify_assets_bad.json" '"ok": false'

SUMMARY_OUT="$TMP_DIR/release-prepare-summary.json"
"$CLI_DIR/scripts/release_prepare.sh" \
  --version "$VERSION" \
  --skip-check \
  --dry-run \
  --assets-dir "$ASSETS_DIR" \
  --output-dir "$ASSETS_DIR" \
  --summary-out "$SUMMARY_OUT" \
  > "$TMP_DIR/release_prepare_dryrun.log"

if [[ ! -f "$SUMMARY_OUT" ]]; then
  echo "error: release_prepare dry-run did not write summary file" >&2
  exit 1
fi
assert_contains "$SUMMARY_OUT" '"dry_run": true'
assert_contains "$SUMMARY_OUT" '"manifests_planned": true'
assert_contains "$SUMMARY_OUT" '"archives_verify_planned": true'
assert_contains "$SUMMARY_OUT" '"assets_verify_planned": true'

echo "release_tooling_smoke: OK"
