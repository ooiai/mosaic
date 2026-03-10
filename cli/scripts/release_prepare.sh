#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 --version <version> [options]

Options:
  --version <tag>                Release tag (for example: v0.2.0-beta.6)
  --target <triple>              Rust target triple (default: host target)
  --notes-from-date <ISO8601>    Filter WORKLOG entries from this timestamp
  --notes-max-entries <n>        Max WORKLOG entries for release notes (default: 20)
  --notes-out <path>             Release notes output path (default: docs/release-notes-<version>.md)
  --assets-dir <dir>             If set, generate brew/scoop manifests from this assets directory
  --output-dir <dir>             Output dir for generated manifests/checksums
  --summary-out <path>           Write a JSON summary file
  --skip-check                   Skip beta readiness checks
  --skip-archive-check           Skip archive-content verification step
  --skip-verify                  Skip asset verification step
  --dry-run                      Print commands without executing
  -h, --help                     Show help

Examples:
  $0 --version v0.2.0-beta.6
  $0 --version v0.2.0-beta.6 --target aarch64-apple-darwin
  $0 --version v0.2.0-beta.6 --assets-dir ../release-assets --output-dir ../release-assets
  $0 --version v0.2.0-beta.6 --assets-dir ../release-assets --notes-out docs/release-notes-v0.2.0-beta.6.md
  $0 --version v0.2.0-beta.6 --skip-check --dry-run
USAGE
}

VERSION=""
TARGET=""
NOTES_FROM_DATE=""
NOTES_MAX_ENTRIES="20"
NOTES_OUT=""
ASSETS_DIR=""
OUTPUT_DIR=""
SUMMARY_OUT=""
SKIP_CHECK=0
SKIP_ARCHIVE_CHECK=0
SKIP_VERIFY=0
DRY_RUN=0

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
    --notes-from-date)
      NOTES_FROM_DATE="${2:-}"
      shift 2
      ;;
    --notes-max-entries)
      NOTES_MAX_ENTRIES="${2:-}"
      shift 2
      ;;
    --notes-out)
      NOTES_OUT="${2:-}"
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
    --summary-out)
      SUMMARY_OUT="${2:-}"
      shift 2
      ;;
    --skip-check)
      SKIP_CHECK=1
      shift
      ;;
    --skip-archive-check)
      SKIP_ARCHIVE_CHECK=1
      shift
      ;;
    --skip-verify)
      SKIP_VERIFY=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
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

if [[ -z "$VERSION" ]]; then
  echo "error: --version is required" >&2
  usage >&2
  exit 1
fi

if ! [[ "$NOTES_MAX_ENTRIES" =~ ^[0-9]+$ ]] || [[ "$NOTES_MAX_ENTRIES" -le 0 ]]; then
  echo "error: --notes-max-entries must be a positive integer" >&2
  exit 1
fi

if [[ -n "$OUTPUT_DIR" && -z "$ASSETS_DIR" ]]; then
  echo "error: --output-dir requires --assets-dir" >&2
  exit 1
fi

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="$(cd "$CLI_DIR/.." && pwd)"
cd "$CLI_DIR"

log() {
  printf "[%s] %s\n" "$(date +%H:%M:%S)" "$*"
}

print_cmd() {
  local part
  for part in "$@"; do
    printf "%q " "$part"
  done
  printf "\n"
}

run_cmd() {
  if [[ "$DRY_RUN" == "1" ]]; then
    print_cmd "$@"
  else
    "$@"
  fi
}

host_target() {
  rustc -vV | awk '/^host:/ {print $2}'
}

target_to_platform() {
  case "$1" in
    aarch64-apple-darwin) echo "darwin-arm64" ;;
    x86_64-apple-darwin) echo "darwin-x64" ;;
    x86_64-unknown-linux-gnu) echo "linux-x64" ;;
    aarch64-unknown-linux-gnu) echo "linux-arm64" ;;
    x86_64-pc-windows-msvc) echo "windows-x64" ;;
    aarch64-pc-windows-msvc) echo "windows-arm64" ;;
    *)
      echo "error: unsupported target '$1'" >&2
      exit 1
      ;;
  esac
}

asset_exists() {
  local assets_root="$1"
  local base_name="$2"
  if [[ -f "$assets_root/$base_name" ]]; then
    return 0
  fi
  if find "$assets_root" -maxdepth 3 -type f -name "$base_name" | grep -q .; then
    return 0
  fi
  return 1
}

if [[ -z "$TARGET" ]]; then
  TARGET="$(host_target)"
fi

if [[ -z "$NOTES_OUT" ]]; then
  NOTES_OUT="docs/release-notes-${VERSION}.md"
fi

NOTES_OUT_ABS="$NOTES_OUT"
if [[ "$NOTES_OUT_ABS" != /* ]]; then
  NOTES_OUT_ABS="$CLI_DIR/$NOTES_OUT_ABS"
fi

PLATFORM="$(target_to_platform "$TARGET")"
ASSET_EXT="tar.gz"
if [[ "$PLATFORM" == windows-* ]]; then
  ASSET_EXT="zip"
fi
ASSET_PATH="$CLI_DIR/dist/$VERSION/mosaic-${VERSION}-${PLATFORM}.${ASSET_EXT}"

log "release prepare start"
log "version: $VERSION"
log "target: $TARGET"
log "notes: $NOTES_OUT"

if [[ "$SKIP_CHECK" != "1" ]]; then
  log "running beta readiness checks"
  run_cmd ./scripts/beta_release_check.sh
else
  log "skip checks enabled"
fi

notes_cmd=(./scripts/release_notes_from_worklog.sh --version "$VERSION" --max-entries "$NOTES_MAX_ENTRIES" --out "$NOTES_OUT")
if [[ -n "$NOTES_FROM_DATE" ]]; then
  notes_cmd+=(--from-date "$NOTES_FROM_DATE")
fi

log "generating release notes draft"
run_cmd "${notes_cmd[@]}"

log "building release binary for target"
run_cmd cargo build --release -p mosaic-cli --target "$TARGET"

log "packaging release asset"
run_cmd ./scripts/package_release_asset.sh --version "$VERSION" --target "$TARGET"

MANIFESTS_GENERATED=0
MANIFESTS_PLANNED=0
ARCHIVES_VERIFIED=0
ARCHIVES_VERIFY_PLANNED=0
ASSETS_VERIFIED=0
ASSETS_VERIFY_PLANNED=0
if [[ -n "$ASSETS_DIR" ]]; then
  if [[ "$DRY_RUN" != "1" ]]; then
    missing_assets=()
    required_assets=(
      "mosaic-${VERSION}-darwin-arm64.tar.gz"
      "mosaic-${VERSION}-darwin-x64.tar.gz"
      "mosaic-${VERSION}-linux-x64.tar.gz"
      "mosaic-${VERSION}-windows-x64.zip"
    )
    for required in "${required_assets[@]}"; do
      if ! asset_exists "$ASSETS_DIR" "$required"; then
        missing_assets+=("$required")
      fi
    done
    if [[ ${#missing_assets[@]} -gt 0 ]]; then
      echo "error: --assets-dir does not contain full release matrix for version '$VERSION'" >&2
      for missing in "${missing_assets[@]}"; do
        echo "  missing: $missing" >&2
      done
      echo "hint: download/upload all release archives before generating manifests" >&2
      exit 1
    fi
  fi

  manifest_cmd=(./scripts/update_distribution_manifests.sh --version "$VERSION" --assets-dir "$ASSETS_DIR")
  if [[ -n "$OUTPUT_DIR" ]]; then
    manifest_cmd+=(--output-dir "$OUTPUT_DIR")
  fi
  log "generating distribution manifests"
  run_cmd "${manifest_cmd[@]}"
  if [[ "$DRY_RUN" == "1" ]]; then
    MANIFESTS_PLANNED=1
  else
    MANIFESTS_GENERATED=1
  fi

  if [[ "$SKIP_ARCHIVE_CHECK" != "1" ]]; then
    archive_verify_cmd=(./scripts/release_verify_archives.sh --version "$VERSION" --assets-dir "$ASSETS_DIR")
    log "verifying archive contents"
    run_cmd "${archive_verify_cmd[@]}"
    if [[ "$DRY_RUN" == "1" ]]; then
      ARCHIVES_VERIFY_PLANNED=1
    else
      ARCHIVES_VERIFIED=1
    fi
  else
    log "skip archive verification enabled"
  fi

  if [[ "$SKIP_VERIFY" != "1" ]]; then
    verify_cmd=(./scripts/release_verify_assets.sh --version "$VERSION" --assets-dir "$ASSETS_DIR" --notes "$NOTES_OUT")
    log "verifying release assets"
    run_cmd "${verify_cmd[@]}"
    if [[ "$DRY_RUN" == "1" ]]; then
      ASSETS_VERIFY_PLANNED=1
    else
      ASSETS_VERIFIED=1
    fi
  else
    log "skip asset verification enabled"
  fi
fi

if [[ "$DRY_RUN" != "1" && ! -f "$ASSET_PATH" ]]; then
  echo "error: expected asset not found after packaging: $ASSET_PATH" >&2
  exit 1
fi

if [[ -n "$SUMMARY_OUT" ]]; then
  SUMMARY_PATH="$SUMMARY_OUT"
  if [[ "$SUMMARY_PATH" != /* ]]; then
    SUMMARY_PATH="$ROOT_DIR/$SUMMARY_PATH"
  fi
  mkdir -p "$(dirname "$SUMMARY_PATH")"

  summary_json="{\n"
  summary_json+="  \"ok\": true,\n"
  summary_json+="  \"dry_run\": $( [[ "$DRY_RUN" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"version\": \"$VERSION\",\n"
  summary_json+="  \"target\": \"$TARGET\",\n"
  summary_json+="  \"platform\": \"$PLATFORM\",\n"
  summary_json+="  \"notes_out\": \"$NOTES_OUT_ABS\",\n"
  summary_json+="  \"asset_path\": \"$ASSET_PATH\",\n"
  summary_json+="  \"manifests_generated\": $( [[ "$MANIFESTS_GENERATED" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"manifests_planned\": $( [[ "$MANIFESTS_PLANNED" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"archives_verified\": $( [[ "$ARCHIVES_VERIFIED" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"archives_verify_planned\": $( [[ "$ARCHIVES_VERIFY_PLANNED" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"assets_verified\": $( [[ "$ASSETS_VERIFIED" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"assets_verify_planned\": $( [[ "$ASSETS_VERIFY_PLANNED" == "1" ]] && echo true || echo false ),\n"
  summary_json+="  \"assets_dir\": \"$ASSETS_DIR\",\n"
  summary_json+="  \"output_dir\": \"$OUTPUT_DIR\",\n"
  summary_json+="  \"generated_at_utc\": \"$(date -u +"%Y-%m-%dT%H:%M:%SZ")\"\n"
  summary_json+="}\n"

  printf "%b" "$summary_json" > "$SUMMARY_PATH"
  if [[ "$DRY_RUN" == "1" ]]; then
    log "dry-run summary: $SUMMARY_PATH"
    printf "%b" "$summary_json"
  else
    log "summary: $SUMMARY_PATH"
  fi
fi

log "release prepare done"
log "notes: $NOTES_OUT_ABS"
log "asset: $ASSET_PATH"
if [[ "$MANIFESTS_GENERATED" == "1" ]]; then
  log "manifests: generated"
elif [[ "$MANIFESTS_PLANNED" == "1" ]]; then
  log "manifests: planned (dry-run)"
else
  log "manifests: skipped"
fi
if [[ "$ARCHIVES_VERIFIED" == "1" ]]; then
  log "archive-check: passed"
elif [[ "$ARCHIVES_VERIFY_PLANNED" == "1" ]]; then
  log "archive-check: planned (dry-run)"
elif [[ -n "$ASSETS_DIR" ]]; then
  log "archive-check: skipped"
fi
if [[ "$ASSETS_VERIFIED" == "1" ]]; then
  log "verify: passed"
elif [[ "$ASSETS_VERIFY_PLANNED" == "1" ]]; then
  log "verify: planned (dry-run)"
elif [[ -n "$ASSETS_DIR" ]]; then
  log "verify: skipped"
fi
