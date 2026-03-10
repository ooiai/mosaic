#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> --assets-dir <dir> [options]

Options:
  --version <tag>             Release tag (for example: v0.2.0-beta.6)
  --assets-dir <dir>          Directory containing packaged archives
  --json                      Emit JSON result
  -h, --help                  Show help

Examples:
  $0 --version v0.2.0-beta.6 --assets-dir ./release-assets
  $0 --version v0.2.0-beta.6 --assets-dir ./release-assets --json
USAGE
}

VERSION=""
ASSETS_DIR=""
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

ASSETS_DIR="$(cd "$ASSETS_DIR" && pwd)"

json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\n'/\\n}"
  s="${s//$'\r'/\\r}"
  s="${s//$'\t'/\\t}"
  printf '%s' "$s"
}

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

list_archive_entries() {
  local archive_path="$1"
  case "$archive_path" in
    *.tar.gz)
      tar -tzf "$archive_path"
      ;;
    *.zip)
      if command -v zipinfo >/dev/null 2>&1; then
        zipinfo -1 "$archive_path"
      elif command -v unzip >/dev/null 2>&1; then
        unzip -Z1 "$archive_path"
      else
        echo "error: no zip listing tool found (zipinfo/unzip)" >&2
        return 2
      fi
      ;;
    *)
      echo "error: unsupported archive type: $archive_path" >&2
      return 2
      ;;
  esac
}

add_error() {
  errors+=("$1")
}

errors=()

required_archives=(
  "mosaic-${VERSION}-darwin-arm64.tar.gz"
  "mosaic-${VERSION}-darwin-x64.tar.gz"
  "mosaic-${VERSION}-linux-x64.tar.gz"
  "mosaic-${VERSION}-windows-x64.zip"
)

required_common=(
  "README.md"
  "install.sh"
  "install.ps1"
  "parity-map.md"
  "regression-runbook.md"
  "distribution.md"
)

verified_archives=()

for archive_name in "${required_archives[@]}"; do
  archive_path="$(find_asset "$archive_name" || true)"
  if [[ -z "$archive_path" ]]; then
    add_error "missing archive: $archive_name"
    continue
  fi

  entries="$(list_archive_entries "$archive_path" 2>/dev/null || true)"
  if [[ -z "$entries" ]]; then
    add_error "unable to list archive entries: $archive_name"
    continue
  fi

  platform="${archive_name#mosaic-${VERSION}-}"
  platform="${platform%.tar.gz}"
  platform="${platform%.zip}"
  prefix="mosaic-${VERSION}-${platform}/"

  if ! printf '%s\n' "$entries" | grep -Fxq "$prefix"; then
    add_error "archive missing top-level directory: $archive_name -> $prefix"
  fi

  for rel in "${required_common[@]}"; do
    if ! printf '%s\n' "$entries" | grep -Fxq "${prefix}${rel}"; then
      add_error "archive missing file: $archive_name -> ${prefix}${rel}"
    fi
  done

  binary_name="mosaic"
  case "$archive_name" in
    *.zip) binary_name="mosaic.exe" ;;
  esac
  if ! printf '%s\n' "$entries" | grep -Fxq "${prefix}${binary_name}"; then
    add_error "archive missing binary: $archive_name -> ${prefix}${binary_name}"
  fi

  verified_archives+=("$archive_name")
done

ok=true
if [[ ${#errors[@]} -gt 0 ]]; then
  ok=false
fi

if [[ "$JSON_MODE" == "1" ]]; then
  printf '{\n'
  printf '  "ok": %s,\n' "$ok"
  printf '  "version": "%s",\n' "$(json_escape "$VERSION")"
  printf '  "assets_dir": "%s",\n' "$(json_escape "$ASSETS_DIR")"

  printf '  "verified_archives": ['
  idx=0
  for item in "${verified_archives[@]-}"; do
    if [[ -z "$item" ]]; then
      continue
    fi
    if [[ $idx -gt 0 ]]; then
      printf ', '
    fi
    printf '"%s"' "$(json_escape "$item")"
    idx=$((idx + 1))
  done
  printf '],\n'

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
  printf ']\n'
  printf '}\n'
else
  echo "release archive verification"
  echo "version: $VERSION"
  echo "assets_dir: $ASSETS_DIR"

  if [[ "$ok" == "true" ]]; then
    echo "status: OK"
  else
    echo "status: FAILED"
  fi

  if [[ ${#verified_archives[@]} -gt 0 ]]; then
    echo "verified_archives:"
    for item in "${verified_archives[@]}"; do
      echo "- $item"
    done
  fi

  if [[ ${#errors[@]} -gt 0 ]]; then
    echo "errors:"
    for msg in "${errors[@]}"; do
      echo "- $msg"
    done
  fi
fi

if [[ "$ok" != "true" ]]; then
  exit 1
fi
