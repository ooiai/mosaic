#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

WORKLOG_SUMMARY=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --worklog-summary)
      WORKLOG_SUMMARY="${2:-}"
      shift 2
      ;;
    *)
      echo "usage: $0 [--worklog-summary \"summary text\"]" >&2
      exit 1
      ;;
  esac
done

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found in PATH" >&2
  exit 1
fi

if ! command -v bash >/dev/null 2>&1; then
  echo "error: bash not found in PATH" >&2
  exit 1
fi

REPORT_DIR="$ROOT_DIR/reports"
mkdir -p "$REPORT_DIR"
STAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
REPORT_FILE="$REPORT_DIR/regression-$STAMP.log"
LATEST_FILE="$REPORT_DIR/regression-latest.log"
START_EPOCH="$(date +%s)"

run_step() {
  local title="$1"
  shift
  {
    echo
    echo "============================================================"
    echo "[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] $title"
    echo "============================================================"
    "$@"
  } | tee -a "$REPORT_FILE"
}

{
  echo "# Mosaic CLI Regression Report"
  echo
  echo "- Started at (UTC): $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "- Workspace: $ROOT_DIR"
} | tee "$REPORT_FILE" >/dev/null

run_step "Generate regression catalog" "$ROOT_DIR/scripts/update_regression_catalog.sh"
run_step "Run release tooling smoke" "$ROOT_DIR/scripts/release_tooling_smoke.sh"
run_step "Run release install smoke" "$ROOT_DIR/scripts/release_install_smoke.sh"
run_step "Run cargo test --workspace" cargo test --workspace
run_step "Run from scratch smoke (no duplicate workspace test)" \
  env SKIP_WORKSPACE_TESTS=1 "$ROOT_DIR/scripts/from_scratch_smoke.sh"

END_EPOCH="$(date +%s)"
DURATION="$((END_EPOCH - START_EPOCH))"
{
  echo
  echo "- Finished at (UTC): $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "- Duration seconds: $DURATION"
  echo "- Report file: $REPORT_FILE"
} | tee -a "$REPORT_FILE" >/dev/null

cp "$REPORT_FILE" "$LATEST_FILE"
echo "regression report: $REPORT_FILE"
echo "latest report: $LATEST_FILE"

if [[ -n "$WORKLOG_SUMMARY" ]]; then
  "$ROOT_DIR/scripts/worklog_append.sh" \
    --summary "$WORKLOG_SUMMARY" \
    --tests "./scripts/run_regression_suite.sh (report: cli/reports/regression-$STAMP.log)"
fi
