#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT_DIR/.." && pwd)"

PROFILE="${PROFILE:-az-openai}"
BASE_URL="${BASE_URL:-${AZURE_OPENAI_BASE_URL:-}}"
API_KEY_ENV="${API_KEY_ENV:-AZURE_OPENAI_API_KEY}"
MODEL="${MODEL:-gpt-4o-mini}"
FALLBACK_MODEL="${FALLBACK_MODEL:-}"
WORKDIR="${WORKDIR:-$REPO_ROOT}"
RUN_REGRESSION="${RUN_REGRESSION:-0}"
SKIP_CHANNELS="${SKIP_CHANNELS:-0}"
SKIP_GATEWAY="${SKIP_GATEWAY:-0}"
SKIP_POLICY="${SKIP_POLICY:-0}"
JSON_SUMMARY="${JSON_SUMMARY:-0}"
SUMMARY_OUT="${SUMMARY_OUT:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --base-url)
      BASE_URL="${2:-}"
      shift 2
      ;;
    --api-key-env)
      API_KEY_ENV="${2:-}"
      shift 2
      ;;
    --model)
      MODEL="${2:-}"
      shift 2
      ;;
    --fallback-model)
      FALLBACK_MODEL="${2:-}"
      shift 2
      ;;
    --workdir)
      WORKDIR="${2:-}"
      shift 2
      ;;
    --run-regression)
      RUN_REGRESSION=1
      shift
      ;;
    --skip-channels)
      SKIP_CHANNELS=1
      shift
      ;;
    --skip-gateway)
      SKIP_GATEWAY=1
      shift
      ;;
    --skip-policy)
      SKIP_POLICY=1
      shift
      ;;
    --json-summary)
      JSON_SUMMARY=1
      shift
      ;;
    --summary-out)
      SUMMARY_OUT="${2:-}"
      shift 2
      ;;
    -h|--help)
      cat <<USAGE
Usage: ./scripts/azure_ops_playbook.sh [options]

Options:
  --profile <name>         profile name (default: az-openai)
  --base-url <url>         Azure OpenAI base URL (default: AZURE_OPENAI_BASE_URL)
  --api-key-env <ENV>      API key env var (default: AZURE_OPENAI_API_KEY)
  --model <model>          model id (default: gpt-4o-mini)
  --fallback-model <id>    optional fallback model to add if missing
  --workdir <path>         target project dir for --project-state (default: repo root)
  --run-regression         run tutorial regression with LIVE=1 after baseline
  --skip-channels          skip channels setup/test steps
  --skip-gateway           skip gateway lifecycle steps
  --skip-policy            skip approvals/sandbox/safety steps
  --json-summary           print final JSON summary to stdout
  --summary-out <path>     write final JSON summary to target path
  -h, --help               show this help

Environment equivalents:
  PROFILE, BASE_URL, API_KEY_ENV, MODEL, FALLBACK_MODEL, WORKDIR,
  RUN_REGRESSION=1, SKIP_CHANNELS=1, SKIP_GATEWAY=1, SKIP_POLICY=1,
  JSON_SUMMARY=1, SUMMARY_OUT=/path/summary.json
USAGE
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$BASE_URL" ]]; then
  echo "error: missing Azure base URL. use --base-url or export AZURE_OPENAI_BASE_URL" >&2
  exit 1
fi

if [[ "$BASE_URL" == *"/v1/v1"* ]]; then
  echo "error: invalid base URL ($BASE_URL): duplicated /v1 detected" >&2
  exit 1
fi

if [[ -z "${!API_KEY_ENV:-}" ]]; then
  echo "error: env var $API_KEY_ENV is empty" >&2
  exit 1
fi

if [[ ! -d "$WORKDIR" ]]; then
  echo "error: workdir does not exist: $WORKDIR" >&2
  exit 1
fi

if [[ -n "${MOSAIC_BIN:-}" ]]; then
  MOSAIC_CMD=("$MOSAIC_BIN")
elif command -v mosaic >/dev/null 2>&1; then
  MOSAIC_CMD=("mosaic")
else
  MOSAIC_CMD=("cargo" "run" "-q" "-p" "mosaic-cli" "--")
fi

REPORT_DIR="$ROOT_DIR/reports"
mkdir -p "$REPORT_DIR"
STAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
SHORT_STAMP="$(date -u +"%H%M%S")"
REPORT_FILE="$REPORT_DIR/azure-ops-playbook-$STAMP.log"
LATEST_FILE="$REPORT_DIR/azure-ops-playbook-latest.log"
START_TS="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
START_EPOCH="$(date +%s)"
CHANNEL_ID=""
REGRESSION_EXECUTED=0

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

json_bool() {
  if [[ "$1" == "1" ]]; then
    printf 'true'
  else
    printf 'false'
  fi
}

emit_summary() {
  local code="$?"
  local finish_ts duration ok status summary_target summary_latest channel_value

  finish_ts="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  duration="$(( $(date +%s) - START_EPOCH ))"
  if [[ "$code" -eq 0 ]]; then
    ok=true
    status="success"
  else
    ok=false
    status="failed"
  fi

  if [[ -n "$SUMMARY_OUT" ]]; then
    summary_target="$SUMMARY_OUT"
  else
    summary_target="$REPORT_DIR/azure-ops-playbook-summary-$STAMP.json"
  fi
  summary_latest="$REPORT_DIR/azure-ops-playbook-summary-latest.json"
  mkdir -p "$(dirname "$summary_target")"

  if [[ -n "$CHANNEL_ID" ]]; then
    channel_value="\"$(json_escape "$CHANNEL_ID")\""
  else
    channel_value="null"
  fi

  cat >"$summary_target" <<JSON
{
  "ok": $ok,
  "status": "$(json_escape "$status")",
  "exit_code": $code,
  "started_at": "$(json_escape "$START_TS")",
  "finished_at": "$(json_escape "$finish_ts")",
  "duration_seconds": $duration,
  "profile": "$(json_escape "$PROFILE")",
  "base_url": "$(json_escape "$BASE_URL")",
  "api_key_env": "$(json_escape "$API_KEY_ENV")",
  "model": "$(json_escape "$MODEL")",
  "fallback_model": "$(json_escape "${FALLBACK_MODEL:--}")",
  "workdir": "$(json_escape "$WORKDIR")",
  "report_file": "$(json_escape "$REPORT_FILE")",
  "channel_id": $channel_value,
  "flags": {
    "run_regression": $(json_bool "$RUN_REGRESSION"),
    "skip_channels": $(json_bool "$SKIP_CHANNELS"),
    "skip_gateway": $(json_bool "$SKIP_GATEWAY"),
    "skip_policy": $(json_bool "$SKIP_POLICY"),
    "regression_executed": $(json_bool "$REGRESSION_EXECUTED")
  }
}
JSON

  if [[ "$summary_target" != "$summary_latest" ]]; then
    cp "$summary_target" "$summary_latest" 2>/dev/null || true
  fi

  if [[ "$JSON_SUMMARY" == "1" ]]; then
    cat "$summary_target"
  fi
  if [[ "$JSON_SUMMARY" == "1" || -n "$SUMMARY_OUT" ]]; then
    echo "azure ops summary: $summary_target"
  fi

  return "$code"
}

trap 'emit_summary' EXIT

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
    cd "$WORKDIR"
    "${MOSAIC_CMD[@]}" --project-state --profile "$PROFILE" "$@"
  )
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
  echo "# Mosaic Azure Ops Playbook"
  echo
  echo "- Started at (UTC): $START_TS"
  echo "- Workdir: $WORKDIR"
  echo "- Profile: $PROFILE"
  echo "- Base URL: $BASE_URL"
  echo "- API key env: $API_KEY_ENV"
  echo "- Model: $MODEL"
  echo "- Fallback model: ${FALLBACK_MODEL:--}"
  echo "- Mosaic command: ${MOSAIC_CMD[*]}"
  echo "- Skip channels: $SKIP_CHANNELS"
  echo "- Skip gateway: $SKIP_GATEWAY"
  echo "- Skip policy: $SKIP_POLICY"
  echo "- Run regression: $RUN_REGRESSION"
} | tee "$REPORT_FILE" >/dev/null

run_step "CLI version" "${MOSAIC_CMD[@]}" --version
run_step "State directory checks" run_mosaic directory --ensure --check-writable

run_step "Setup Azure profile" run_mosaic setup --base-url "$BASE_URL" --api-key-env "$API_KEY_ENV" --model "$MODEL"
run_step "Show config" run_mosaic configure --show
run_step "Models list" run_mosaic models list
run_step "Models status" run_mosaic models status
run_step "Set fast alias" run_mosaic models aliases set fast "$MODEL"
run_step "List aliases" run_mosaic models aliases list

if [[ -n "$FALLBACK_MODEL" ]]; then
  if run_mosaic models fallbacks list | grep -Fq "$FALLBACK_MODEL"; then
    echo "fallback model already present: $FALLBACK_MODEL" | tee -a "$REPORT_FILE"
  else
    run_step "Add fallback model" run_mosaic models fallbacks add "$FALLBACK_MODEL"
  fi
fi
run_step "List fallback chain" run_mosaic models fallbacks list

run_step "Ask smoke" run_mosaic ask "reply in one short line: azure playbook ok"
run_step "Session list" run_mosaic session list

if [[ "$SKIP_CHANNELS" == "0" ]]; then
  channel_name="azure-playbook-terminal-${SHORT_STAMP}"
  capture_step channel_add_json "Add terminal channel" run_mosaic --json channels add --name "$channel_name" --kind terminal
  CHANNEL_ID="$(extract_channel_id "$channel_add_json")"
  if [[ -z "$CHANNEL_ID" ]]; then
    echo "error: failed to parse channel id from channels add output" | tee -a "$REPORT_FILE" >&2
    exit 1
  fi
  echo "terminal channel id: $CHANNEL_ID" | tee -a "$REPORT_FILE"

  run_step "Channels status" run_mosaic channels status
  run_step "Channels test" run_mosaic channels test "$CHANNEL_ID"
  run_step "Channels send" run_mosaic channels send "$CHANNEL_ID" --text "azure playbook send ok"
  run_step "Channels logs" run_mosaic channels logs --channel "$CHANNEL_ID" --tail 20
else
  echo "channels steps skipped" | tee -a "$REPORT_FILE"
fi

if [[ "$SKIP_GATEWAY" == "0" ]]; then
  run_step "Gateway install" run_mosaic gateway install --host 127.0.0.1 --port 8787
  run_step "Gateway start" run_mosaic gateway start
  run_step "Gateway probe" run_mosaic gateway probe
  run_step "Gateway call status" run_mosaic gateway call status --params '{"detail":true}'
  run_step "Gateway health" run_mosaic gateway health --verbose
else
  echo "gateway steps skipped" | tee -a "$REPORT_FILE"
fi

if [[ "$SKIP_POLICY" == "0" ]]; then
  run_step "Approvals set confirm" run_mosaic approvals set confirm
  run_step "Approvals allowlist add git status" run_mosaic approvals allowlist add "git status"
  run_step "Sandbox set standard" run_mosaic sandbox set standard
  run_step "Safety check git status" run_mosaic safety check --command "git status"
  run_step "Safety check curl" run_mosaic safety check --command "curl https://example.com"
  run_step "Safety report" run_mosaic safety report --audit-tail 50 --compare-window 24h
else
  echo "policy steps skipped" | tee -a "$REPORT_FILE"
fi

run_step "Status" run_mosaic status
run_step "Health" run_mosaic health
run_step "Doctor" run_mosaic doctor
run_step "Dashboard" run_mosaic dashboard
run_step "Logs tail" run_mosaic logs --tail 120

if [[ "$SKIP_GATEWAY" == "0" ]]; then
  run_step "Gateway stop" run_mosaic gateway stop
fi

if [[ "$RUN_REGRESSION" == "1" ]]; then
  REGRESSION_EXECUTED=1
  run_step "Run tutorial regression (live)" env LIVE=1 BASE_URL="$BASE_URL" API_KEY_ENV="$API_KEY_ENV" MODEL="$MODEL" PROFILE="$PROFILE" ./scripts/tutorial_regression.sh --profile "$PROFILE"
else
  echo "tutorial regression skipped (use --run-regression to enable)" | tee -a "$REPORT_FILE"
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
echo "azure ops playbook report: $REPORT_FILE"
echo "latest report: $LATEST_FILE"
