#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> --assets-dir <dir> [options]

Options:
  --version <tag>             Release tag (for example: v0.2.0-beta.6)
  --assets-dir <dir>          Directory containing release assets
  --notes <path>              Release notes path (default: cli/docs/release-notes-<version>.md)
  --json                      Emit JSON result
  -h, --help                  Show help

Examples:
  $0 --version v0.2.0-beta.6 --assets-dir ./release-assets
  $0 --version v0.2.0-beta.6 --assets-dir ./release-assets --json
  $0 --version v0.2.0-beta.6 --assets-dir ./release-assets --notes ./docs/release-notes-v0.2.0-beta.6.md
USAGE
}

VERSION=""
ASSETS_DIR=""
NOTES_PATH=""
JSON_MODE=0

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
    --notes)
      NOTES_PATH="${2:-}"
      shift 2
      ;;
    --json)
      JSON_MODE=1
      shift
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
if [[ -z "$NOTES_PATH" ]]; then
  NOTES_PATH="$CLI_DIR/docs/release-notes-${VERSION}.md"
fi
if [[ "$NOTES_PATH" != /* ]]; then
  NOTES_PATH="$CLI_DIR/$NOTES_PATH"
fi

ASSETS_DIR="$(cd "$ASSETS_DIR" && pwd)"

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

sha256_of() {
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

add_error() {
  errors+=("$1")
}

add_warning() {
  warnings+=("$1")
}

hash_entries=()
path_entries=()

set_entry() {
  local key="$1"
  local value="$2"
  local pair
  pair="${key}"$'\t'"${value}"
  path_entries+=("$pair")
}

set_hash() {
  local key="$1"
  local value="$2"
  local pair
  pair="${key}"$'\t'"${value}"
  hash_entries+=("$pair")
}

get_from_entries() {
  local key="$1"
  shift
  local entry k v
  for entry in "$@"; do
    k="${entry%%$'\t'*}"
    v="${entry#*$'\t'}"
    if [[ "$k" == "$key" ]]; then
      echo "$v"
      return 0
    fi
  done
  return 1
}

json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\n'/\\n}"
  s="${s//$'\r'/\\r}"
  s="${s//$'\t'/\\t}"
  printf '%s' "$s"
}

errors=()
warnings=()

required_archives=(
  "mosaic-${VERSION}-darwin-arm64.tar.gz"
  "mosaic-${VERSION}-darwin-x64.tar.gz"
  "mosaic-${VERSION}-linux-x64.tar.gz"
  "mosaic-${VERSION}-windows-x64.zip"
)

required_aux=(
  "mosaic.rb"
  "mosaic.json"
  "SHA256SUMS"
  "install.sh"
  "install.ps1"
)

for asset in "${required_archives[@]}"; do
  asset_path="$(find_asset "$asset" || true)"
  if [[ -z "$asset_path" ]]; then
    add_error "missing archive: $asset"
    continue
  fi
  set_entry "$asset" "$asset_path"

  actual_hash="$(sha256_of "$asset_path")"
  set_hash "$asset" "$actual_hash"

  sidecar_path="${asset_path}.sha256"
  if [[ ! -f "$sidecar_path" ]]; then
    add_error "missing sidecar checksum: $(basename "$sidecar_path")"
  else
    sidecar_hash="$(awk 'NF{print $1; exit}' "$sidecar_path")"
    if [[ -z "$sidecar_hash" ]]; then
      add_error "empty sidecar checksum: $(basename "$sidecar_path")"
    elif [[ "$sidecar_hash" != "$actual_hash" ]]; then
      add_error "sidecar checksum mismatch: $asset (expected $sidecar_hash actual $actual_hash)"
    fi
  fi
done

for aux in "${required_aux[@]}"; do
  aux_path="$(find_asset "$aux" || true)"
  if [[ -z "$aux_path" ]]; then
    add_error "missing file: $aux"
    continue
  fi
  set_entry "$aux" "$aux_path"
done

sums_path="$(get_from_entries "SHA256SUMS" "${path_entries[@]-}" || true)"
if [[ -n "$sums_path" ]]; then
  for asset in "${required_archives[@]}"; do
    asset_hash="$(get_from_entries "$asset" "${hash_entries[@]-}" || true)"
    if [[ -z "$asset_hash" ]]; then
      continue
    fi
    sums_hash="$(awk -v a="$asset" '$2==a{print $1; exit}' "$sums_path")"
    if [[ -z "$sums_hash" ]]; then
      add_error "SHA256SUMS missing entry for $asset"
    elif [[ "$sums_hash" != "$asset_hash" ]]; then
      add_error "SHA256SUMS mismatch for $asset (expected $sums_hash actual $asset_hash)"
    fi
  done
fi

version_stripped="${VERSION#v}"
rb_path="$(get_from_entries "mosaic.rb" "${path_entries[@]-}" || true)"
if [[ -n "$rb_path" ]]; then
  if ! grep -Fq "version \"${version_stripped}\"" "$rb_path"; then
    add_error "mosaic.rb version mismatch (expected ${version_stripped})"
  fi

  rb_expected_urls=(
    "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-darwin-arm64.tar.gz"
    "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-darwin-x64.tar.gz"
    "https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-linux-x64.tar.gz"
  )
  for expected_url in "${rb_expected_urls[@]}"; do
    if ! grep -Fq "$expected_url" "$rb_path"; then
      add_error "mosaic.rb missing URL: $expected_url"
    fi
  done
fi

json_path="$(get_from_entries "mosaic.json" "${path_entries[@]-}" || true)"
if [[ -n "$json_path" ]]; then
  if ! grep -Fq "\"version\": \"${version_stripped}\"" "$json_path"; then
    add_error "mosaic.json version mismatch (expected ${version_stripped})"
  fi

  expected_windows_url="https://github.com/ooiai/mosaic/releases/download/${VERSION}/mosaic-${VERSION}-windows-x64.zip"
  if ! grep -Fq "$expected_windows_url" "$json_path"; then
    add_error "mosaic.json missing URL: $expected_windows_url"
  fi

  windows_hash="$(get_from_entries "mosaic-${VERSION}-windows-x64.zip" "${hash_entries[@]-}" || true)"
  if [[ -n "$windows_hash" ]]; then
    if ! grep -Fq "\"hash\": \"${windows_hash}\"" "$json_path"; then
      add_error "mosaic.json hash mismatch for windows-x64"
    fi
  fi
fi

if [[ ! -f "$NOTES_PATH" ]]; then
  add_warning "release notes file not found: $NOTES_PATH"
fi

ok=true
if [[ ${#errors[@]} -gt 0 ]]; then
  ok=false
fi

if [[ "$JSON_MODE" == "1" ]]; then
  printf '{\n'
  printf '  "ok": %s,\n' "$ok"
  printf '  "version": "%s",\n' "$(json_escape "$VERSION")"
  printf '  "assets_dir": "%s",\n' "$(json_escape "$ASSETS_DIR")"
  printf '  "notes_path": "%s",\n' "$(json_escape "$NOTES_PATH")"

  printf '  "errors": ['
  idx=0
  for msg in "${errors[@]-}"; do
    if [[ -z "$msg" ]]; then
      continue
    fi
    if [[ $idx -gt 0 ]]; then
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$msg")"
    idx=$((idx + 1))
  done
  printf '],\n'

  printf '  "warnings": ['
  idx=0
  for msg in "${warnings[@]-}"; do
    if [[ -z "$msg" ]]; then
      continue
    fi
    if [[ $idx -gt 0 ]]; then
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$msg")"
    idx=$((idx + 1))
  done
  printf ']\n'
  printf '}\n'
else
  echo "release asset verification"
  echo "version: $VERSION"
  echo "assets_dir: $ASSETS_DIR"
  echo "notes: $NOTES_PATH"

  if [[ ${#errors[@]} -eq 0 ]]; then
    echo "status: OK"
  else
    echo "status: FAILED"
  fi

  if [[ ${#errors[@]} -gt 0 ]]; then
    echo "errors:"
    for msg in "${errors[@]}"; do
      echo "- $msg"
    done
  fi

  if [[ ${#warnings[@]} -gt 0 ]]; then
    echo "warnings:"
    for msg in "${warnings[@]}"; do
      echo "- $msg"
    done
  fi
fi

if [[ "$ok" != "true" ]]; then
  exit 1
fi
