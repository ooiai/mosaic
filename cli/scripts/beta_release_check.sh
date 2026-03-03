#!/usr/bin/env bash
set -euo pipefail

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CLI_DIR"

STAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
REPORT_DIR="$CLI_DIR/reports"
REPORT_PATH="$REPORT_DIR/beta-readiness-${STAMP}.log"
LATEST_PATH="$REPORT_DIR/beta-readiness-latest.log"

mkdir -p "$REPORT_DIR"

log() {
  printf "\n[%s] %s\n" "$(date +%H:%M:%S)" "$*"
}

run() {
  log "$*"
  "$@"
}

{
  echo "Mosaic CLI beta readiness check"
  echo "UTC: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  run cargo fmt --all --check
  run cargo check -p mosaic-cli
  run cargo clippy -p mosaic-cli -- -D warnings

  run cargo test --workspace

  run cargo test -p mosaic-cli --test command_surface
  run cargo test -p mosaic-cli --test help_snapshot
  run cargo test -p mosaic-cli --test error_codes
  run cargo test -p mosaic-cli --test json_contract
  run cargo test -p mosaic-cli --test json_contract_modules

  run env SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh
  run ./scripts/update_regression_catalog.sh

  log "beta readiness checks passed"
} 2>&1 | tee "$REPORT_PATH"

cp "$REPORT_PATH" "$LATEST_PATH"

echo "report: $REPORT_PATH"
echo "latest: $LATEST_PATH"
