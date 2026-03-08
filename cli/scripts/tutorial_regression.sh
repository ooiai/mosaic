#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

LIVE="${LIVE:-0}"
KEEP_TEMP="${KEEP_TEMP:-0}"
PROFILE="${PROFILE:-default}"
BASE_URL="${BASE_URL:-https://api.openai.com}"
MODEL="${MODEL:-gpt-4o-mini}"
API_KEY_ENV="${API_KEY_ENV:-OPENAI_API_KEY}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --live)
      LIVE=1
      shift
      ;;
    --keep-temp)
      KEEP_TEMP=1
      shift
      ;;
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --base-url)
      BASE_URL="${2:-}"
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      shift 2
      ;;
    --api-key-env)
      API_KEY_ENV="${2:-}"
      shift 2
      ;;
    -h|--help)
      cat <<'USAGE'
Usage: ./scripts/tutorial_regression.sh [options]

Options:
  --live                 Enable real provider smoke tests (same as LIVE=1)
  --keep-temp            Keep temporary project directory
  --profile <name>       Profile name for setup/models/ask (default: default)
  --base-url <url>       Provider base URL for live smoke (default: https://api.openai.com)
  --model <model>        Model name for live smoke (default: gpt-4o-mini)
  --api-key-env <ENV>    API key env var name for live smoke (default: OPENAI_API_KEY)
  -h, --help             Show this help

Environment:
  LIVE=1                 Same as --live
  KEEP_TEMP=1            Same as --keep-temp
  MOSAIC_BIN=/path/mosaic  Use a specific mosaic binary instead of cargo run/mosaic in PATH
  USE_SYSTEM_MOSAIC=1    Prefer `mosaic` from PATH (default uses workspace cargo run)
USAGE
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -n "${MOSAIC_BIN:-}" ]]; then
  MOSAIC_CMD=("$MOSAIC_BIN")
elif [[ "${USE_SYSTEM_MOSAIC:-0}" == "1" ]] && command -v mosaic >/dev/null 2>&1; then
  MOSAIC_CMD=("mosaic")
else
  MOSAIC_CMD=("cargo" "run" "-q" "--manifest-path" "$ROOT_DIR/Cargo.toml" "-p" "mosaic-cli" "--")
fi

REPORT_DIR="$ROOT_DIR/reports"
mkdir -p "$REPORT_DIR"
STAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
REPORT_FILE="$REPORT_DIR/tutorial-regression-$STAMP.log"
LATEST_FILE="$REPORT_DIR/tutorial-regression-latest.log"
START_EPOCH="$(date +%s)"

TMP_PROJECT="$(mktemp -d "${TMPDIR:-/tmp}/mosaic-tutorial-regression.XXXXXX")"

cleanup() {
  if [[ "$KEEP_TEMP" -eq 1 ]]; then
    echo "temp project preserved: $TMP_PROJECT" | tee -a "$REPORT_FILE"
  else
    rm -rf "$TMP_PROJECT"
  fi
}
trap cleanup EXIT

print_header() {
  local title="$1"
  {
    echo
    echo "============================================================"
    echo "[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] $title"
    echo "============================================================"
  } | tee -a "$REPORT_FILE"
}

run_step() {
  local title="$1"
  shift
  print_header "$title"
  "$@" 2>&1 | tee -a "$REPORT_FILE"
}

capture_step() {
  local __var_name="$1"
  shift
  local title="$1"
  shift
  local output
  print_header "$title"
  output="$($@ 2>&1)"
  printf '%s\n' "$output" | tee -a "$REPORT_FILE"
  printf -v "$__var_name" '%s' "$output"
}

run_mosaic() {
  (
    cd "$TMP_PROJECT"
    "${MOSAIC_CMD[@]}" --project-state --profile "$PROFILE" "$@"
  )
}

prepare_knowledge_docs() {
  mkdir -p "$TMP_PROJECT/docs"
  cat >"$TMP_PROJECT/docs/ops.md" <<'DOC'
# Ops Retry Policy
Use exponential backoff with jitter and capped retries for transient upstream failures.
DOC
}

extract_channel_id() {
  local payload="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s\n' "$payload" | jq -r '.channel.id // empty'
  else
    printf '%s\n' "$payload" | sed -n 's/^[[:space:]]*"id":[[:space:]]*"\(ch_[^"]*\)".*/\1/p' | head -n 1
  fi
}

{
  echo "# Mosaic Tutorial Regression"
  echo ""
  echo "- Started at (UTC): $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "- CLI root: $ROOT_DIR"
  echo "- Temp project: $TMP_PROJECT"
  echo "- Live smoke: $LIVE"
  echo "- Profile: $PROFILE"
  echo "- Provider base URL: $BASE_URL"
  echo "- Model: $MODEL"
  echo "- API key env: $API_KEY_ENV"
  echo "- Mosaic command: ${MOSAIC_CMD[*]}"
} | tee "$REPORT_FILE" >/dev/null

run_step "CLI availability" "${MOSAIC_CMD[@]}" --version
run_step "CLI top-level help" "${MOSAIC_CMD[@]}" --help
run_step "Project-state directory checks" run_mosaic directory --ensure --check-writable

capture_step channel_add_json "Add terminal channel" run_mosaic --json channels add --name local-terminal --kind terminal
CHANNEL_ID="$(extract_channel_id "$channel_add_json")"
if [[ -z "$CHANNEL_ID" ]]; then
  echo "error: failed to parse channel id from channels add output" | tee -a "$REPORT_FILE" >&2
  exit 1
fi

echo "parsed terminal channel id: $CHANNEL_ID" | tee -a "$REPORT_FILE"

run_step "Channels list/status/capabilities/resolve" run_mosaic channels list
run_step "Channels status" run_mosaic channels status
run_step "Channels capabilities" run_mosaic channels capabilities
run_step "Channels resolve terminal" run_mosaic channels resolve --channel terminal local
run_step "Channels test terminal" run_mosaic channels test "$CHANNEL_ID"
run_step "Channels send terminal" run_mosaic channels send "$CHANNEL_ID" --text "tutorial regression terminal send"
run_step "Channels logs" run_mosaic channels logs --channel "$CHANNEL_ID" --tail 20

run_step "Gateway install" run_mosaic gateway install --host 127.0.0.1 --port 8787
run_step "Gateway start" run_mosaic gateway start
run_step "Gateway status deep" run_mosaic gateway status --deep
run_step "Gateway health verbose" run_mosaic gateway health --verbose
run_step "Gateway probe" run_mosaic gateway probe
run_step "Gateway discover" run_mosaic gateway discover
run_step "Gateway call status" run_mosaic gateway call status --params '{"detail":true}'
run_step "Gateway restart" run_mosaic gateway restart
run_step "Gateway stop" run_mosaic gateway stop
run_step "Gateway uninstall" run_mosaic gateway uninstall

run_step "MCP list" run_mosaic mcp list
run_step "MCP check all" run_mosaic --json mcp check --all
run_step "MCP deep check all" run_mosaic --json mcp check --all --deep --timeout-ms 300 --report-out .mosaic/reports/tutorial-mcp-check-deep.json

run_step "Approvals policy flow" run_mosaic approvals get
run_step "Set approvals confirm" run_mosaic approvals set confirm
run_step "Approvals allowlist add" run_mosaic approvals allowlist add "git status"
run_step "Sandbox flow" run_mosaic sandbox get
run_step "Set sandbox standard" run_mosaic sandbox set standard
run_step "Sandbox explain restricted" run_mosaic sandbox explain --profile restricted

run_step "System + observability" run_mosaic status
run_step "Health" run_mosaic health
run_step "Doctor" run_mosaic doctor
run_step "Logs tail" run_mosaic logs --tail 50
run_step "System presence" run_mosaic system presence
run_step "System event" run_mosaic system event tutorial.regression --data '{"source":"tutorial_regression.sh"}'
run_step "Prepare local knowledge corpus" prepare_knowledge_docs
run_step "Knowledge ingest local markdown" run_mosaic knowledge ingest --source local_md --path docs --namespace tutorial-kb --incremental --report-out .mosaic/reports/tutorial-knowledge-ingest.json
run_step "Knowledge search" run_mosaic knowledge search "retry jitter" --namespace tutorial-kb --limit 5 --min-score 4
run_step "Knowledge ask references-only" run_mosaic --json knowledge ask "How should retry jitter be configured?" --namespace tutorial-kb --top-k 5 --min-score 3 --references-only
run_step "Knowledge evaluate batch" run_mosaic --json knowledge evaluate --query "retry jitter" --query "missing topic" --namespace tutorial-kb --top-k 5 --min-score 3 --report-out .mosaic/reports/tutorial-knowledge-eval.json
run_step "Knowledge evaluate baseline update" run_mosaic --json knowledge evaluate --query "retry jitter" --namespace tutorial-kb --top-k 5 --min-score 3 --history-window 20 --update-baseline
run_step "Knowledge evaluate baseline gate" run_mosaic --json knowledge evaluate --query "retry jitter" --namespace tutorial-kb --top-k 5 --min-score 3 --history-window 20 --max-coverage-drop 0.05 --max-avg-top-score-drop 1.0 --fail-on-regression
run_step "Knowledge datasets list" run_mosaic --json knowledge datasets list
run_step "Knowledge datasets remove dry-run" run_mosaic --json knowledge datasets remove tutorial-kb --dry-run
run_step "Knowledge datasets remove" run_mosaic --json knowledge datasets remove tutorial-kb
run_step "Knowledge datasets list after remove" run_mosaic --json knowledge datasets list --namespace tutorial-kb

if [[ "$LIVE" == "1" ]]; then
  LIVE_KEY="${!API_KEY_ENV:-}"
  if [[ -z "$LIVE_KEY" ]]; then
    echo "error: LIVE=1 but env var $API_KEY_ENV is empty" | tee -a "$REPORT_FILE" >&2
    exit 1
  fi

  run_step "Live setup" run_mosaic setup --base-url "$BASE_URL" --api-key-env "$API_KEY_ENV" --model "$MODEL"
  run_step "Live models list" run_mosaic models list
  run_step "Live models status" run_mosaic models status
  run_step "Live ask smoke" run_mosaic ask "reply with one short line: live regression ok"
  run_step "Live session list" run_mosaic session list
else
  echo "live smoke skipped (set LIVE=1 to enable provider checks)" | tee -a "$REPORT_FILE"
fi

END_EPOCH="$(date +%s)"
DURATION="$((END_EPOCH - START_EPOCH))"
{
  echo
  echo "- Finished at (UTC): $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "- Duration seconds: $DURATION"
  echo "- Report file: $REPORT_FILE"
} | tee -a "$REPORT_FILE"

cp "$REPORT_FILE" "$LATEST_FILE"
echo "tutorial regression report: $REPORT_FILE"
echo "latest report: $LATEST_FILE"
