#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found in PATH" >&2
  exit 1
fi

if ! command -v rg >/dev/null 2>&1; then
  echo "error: rg not found in PATH" >&2
  exit 1
fi

log() {
  printf "\n[%s] %s\n" "$(date +%H:%M:%S)" "$*"
}

require_contains() {
  local file="$1"
  local pattern="$2"
  if ! rg -q "$pattern" "$file"; then
    echo "error: expected pattern '$pattern' in $file" >&2
    echo "----- $file -----" >&2
    cat "$file" >&2
    echo "-----------------" >&2
    exit 1
  fi
}

extract_first_match() {
  local file="$1"
  local pattern="$2"
  local value
  value="$(rg -o "$pattern" "$file" | head -n1 || true)"
  if [[ -z "$value" ]]; then
    echo "error: cannot extract pattern '$pattern' from $file" >&2
    cat "$file" >&2
    exit 1
  fi
  printf "%s" "$value"
}

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/mosaic-smoke-XXXXXX")"
SRC_DIR="$TMP_ROOT/src"
DST_DIR="$TMP_ROOT/dst"
mkdir -p "$SRC_DIR" "$DST_DIR"

cleanup() {
  if [[ "${KEEP_TMP:-0}" == "1" ]]; then
    log "KEEP_TMP=1, preserving temp dir: $TMP_ROOT"
  else
    rm -rf "$TMP_ROOT"
  fi
}
trap cleanup EXIT

if [[ "${SKIP_WORKSPACE_TESTS:-0}" != "1" ]]; then
  log "Running cargo test --workspace"
  cargo test --workspace
else
  log "Skipping workspace tests (SKIP_WORKSPACE_TESTS=1)"
fi

log "Step 1: setup/models/ask/session smoke in isolated workspace"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state setup --base-url mock://mock-model --model mock-model >/dev/null)

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json models list >"$TMP_ROOT/models.json")
require_contains "$TMP_ROOT/models.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/models.json" '"mock-model"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json models list --query mock --limit 1 >"$TMP_ROOT/models_filtered.json")
require_contains "$TMP_ROOT/models_filtered.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/models_filtered.json" '"query"[[:space:]]*:[[:space:]]*"mock"'
require_contains "$TMP_ROOT/models_filtered.json" '"returned_models"[[:space:]]*:[[:space:]]*1'

(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json ask "hello from smoke" >"$TMP_ROOT/ask.json")
require_contains "$TMP_ROOT/ask.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/ask.json" '"response"[[:space:]]*:[[:space:]]*"mock-answer"'
SESSION_ID="$(sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP_ROOT/ask.json" | head -n1)"
if [[ -z "$SESSION_ID" ]]; then
  echo "error: failed to parse session_id from ask output" >&2
  cat "$TMP_ROOT/ask.json" >&2
  exit 1
fi
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json session show "$SESSION_ID" >"$TMP_ROOT/session_show.json")
require_contains "$TMP_ROOT/session_show.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/session_show.json" '"events"'

printf "ask prompt from file\n" >"$SRC_DIR/ask-prompt.txt"
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-file-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json ask --prompt-file ask-prompt.txt >"$TMP_ROOT/ask_prompt_file.json")
require_contains "$TMP_ROOT/ask_prompt_file.json" '"response"[[:space:]]*:[[:space:]]*"mock-file-answer"'

cat >"$SRC_DIR/ask-script.txt" <<'EOF'
first ask scripted prompt
second ask scripted prompt
EOF
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-ask-script-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json ask --script ask-script.txt >"$TMP_ROOT/ask_script.json")
require_contains "$TMP_ROOT/ask_script.json" '"mode"[[:space:]]*:[[:space:]]*"script"'
require_contains "$TMP_ROOT/ask_script.json" '"run_count"[[:space:]]*:[[:space:]]*2'

(cd "$SRC_DIR" && printf "stdin ask scripted first\nstdin ask scripted second\n" | MOSAIC_MOCK_CHAT_RESPONSE="mock-ask-script-stdin-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json ask --script - >"$TMP_ROOT/ask_script_stdin.json")
require_contains "$TMP_ROOT/ask_script_stdin.json" '"mode"[[:space:]]*:[[:space:]]*"script"'
require_contains "$TMP_ROOT/ask_script_stdin.json" '"run_count"[[:space:]]*:[[:space:]]*2'

printf "chat prompt from file\n" >"$SRC_DIR/chat-prompt.txt"
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-chat-file-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json chat --prompt-file chat-prompt.txt >"$TMP_ROOT/chat_prompt_file.json")
require_contains "$TMP_ROOT/chat_prompt_file.json" '"response"[[:space:]]*:[[:space:]]*"mock-chat-file-answer"'

cat >"$SRC_DIR/chat-script.txt" <<'EOF'
first scripted prompt

second scripted prompt
EOF
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-chat-script-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json chat --script chat-script.txt >"$TMP_ROOT/chat_script.json")
require_contains "$TMP_ROOT/chat_script.json" '"mode"[[:space:]]*:[[:space:]]*"script"'
require_contains "$TMP_ROOT/chat_script.json" '"run_count"[[:space:]]*:[[:space:]]*2'

printf "clawbot ask prompt file\n" >"$SRC_DIR/clawbot-ask.txt"
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-clawbot-file-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot ask --prompt-file clawbot-ask.txt >"$TMP_ROOT/clawbot_ask_prompt_file.json")
require_contains "$TMP_ROOT/clawbot_ask_prompt_file.json" '"response"[[:space:]]*:[[:space:]]*"mock-clawbot-file-answer"'

cat >"$SRC_DIR/clawbot-ask-script.txt" <<'EOF'
clawbot ask scripted first
clawbot ask scripted second
EOF
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-clawbot-ask-script-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot ask --script clawbot-ask-script.txt >"$TMP_ROOT/clawbot_ask_script.json")
require_contains "$TMP_ROOT/clawbot_ask_script.json" '"mode"[[:space:]]*:[[:space:]]*"script"'
require_contains "$TMP_ROOT/clawbot_ask_script.json" '"run_count"[[:space:]]*:[[:space:]]*2'

(cd "$SRC_DIR" && printf "stdin clawbot ask scripted first\nstdin clawbot ask scripted second\n" | MOSAIC_MOCK_CHAT_RESPONSE="mock-clawbot-ask-script-stdin-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot ask --script - >"$TMP_ROOT/clawbot_ask_script_stdin.json")
require_contains "$TMP_ROOT/clawbot_ask_script_stdin.json" '"mode"[[:space:]]*:[[:space:]]*"script"'
require_contains "$TMP_ROOT/clawbot_ask_script_stdin.json" '"run_count"[[:space:]]*:[[:space:]]*2'

cat >"$SRC_DIR/clawbot-chat-script.txt" <<'EOF'
clawbot scripted first
clawbot scripted second
EOF
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-clawbot-script-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot chat --script clawbot-chat-script.txt >"$TMP_ROOT/clawbot_chat_script.json")
require_contains "$TMP_ROOT/clawbot_chat_script.json" '"mode"[[:space:]]*:[[:space:]]*"script"'
require_contains "$TMP_ROOT/clawbot_chat_script.json" '"run_count"[[:space:]]*:[[:space:]]*2'

printf "clawbot send text file\n" >"$SRC_DIR/clawbot-send.txt"
(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="mock-clawbot-send-file-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot send --text-file clawbot-send.txt >"$TMP_ROOT/clawbot_send_text_file.json")
require_contains "$TMP_ROOT/clawbot_send_text_file.json" '"response"[[:space:]]*:[[:space:]]*"mock-clawbot-send-file-answer"'

(cd "$SRC_DIR" && printf "stdin clawbot send text file\n" | MOSAIC_MOCK_CHAT_RESPONSE="mock-clawbot-send-stdin-file-answer" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot send --text-file - >"$TMP_ROOT/clawbot_send_text_file_stdin.json")
require_contains "$TMP_ROOT/clawbot_send_text_file_stdin.json" '"response"[[:space:]]*:[[:space:]]*"mock-clawbot-send-stdin-file-answer"'

log "Step 2: channels export/import (with success + strict-failure reports)"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels add --name src-slack --kind slack_webhook --endpoint mock-http://200 >"$TMP_ROOT/add_src_slack.json")
SRC_CHANNEL_ID="$(extract_first_match "$TMP_ROOT/add_src_slack.json" 'ch_[0-9a-f-]{8,}')"

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels export --out "$TMP_ROOT/channels-export.json" >"$TMP_ROOT/export.json")
require_contains "$TMP_ROOT/export.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/channels-export.json" '"schema"[[:space:]]*:[[:space:]]*"mosaic\.channels\.export\.v1"'

(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels import --file "$TMP_ROOT/channels-export.json" --report-out "$TMP_ROOT/import-report.json" >"$TMP_ROOT/import_ok.json")
require_contains "$TMP_ROOT/import_ok.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/import_ok.json" '"imported"[[:space:]]*:[[:space:]]*1'
require_contains "$TMP_ROOT/import_ok.json" '"report_path"'
require_contains "$TMP_ROOT/import-report.json" '"schema"[[:space:]]*:[[:space:]]*"mosaic\.channels\.import-report\.v1"'
require_contains "$TMP_ROOT/import-report.json" '"ok"[[:space:]]*:[[:space:]]*true'

set +e
(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels import --file "$TMP_ROOT/channels-export.json" --strict --report-out "$TMP_ROOT/import-strict-report.json" >"$TMP_ROOT/import_strict_fail.json" 2>/dev/null)
STRICT_EXIT=$?
set -e
if [[ $STRICT_EXIT -eq 0 ]]; then
  echo "error: strict import should fail when conflicts exist" >&2
  exit 1
fi
require_contains "$TMP_ROOT/import_strict_fail.json" '"ok"[[:space:]]*:[[:space:]]*false'
require_contains "$TMP_ROOT/import_strict_fail.json" '"code"[[:space:]]*:[[:space:]]*"validation"'
require_contains "$TMP_ROOT/import-strict-report.json" '"schema"[[:space:]]*:[[:space:]]*"mosaic\.channels\.import-report\.v1"'
require_contains "$TMP_ROOT/import-strict-report.json" '"ok"[[:space:]]*:[[:space:]]*false'

log "Step 3: rotate-token-env with dry-run + apply + report"
(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels add --name tg-rotate --kind telegram_bot --chat-id=-1001234567890 --endpoint mock-http://200 --token-env MOSAIC_TELEGRAM_BOT_TOKEN_OLD >"$TMP_ROOT/add_tg.json")
TG_CHANNEL_ID="$(extract_first_match "$TMP_ROOT/add_tg.json" 'ch_[0-9a-f-]{8,}')"
(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels add --name term-rotate --kind terminal >"$TMP_ROOT/add_term.json")

(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels rotate-token-env --all --to MOSAIC_TELEGRAM_BOT_TOKEN_NEW --dry-run >"$TMP_ROOT/rotate_dry.json")
require_contains "$TMP_ROOT/rotate_dry.json" '"dry_run"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/rotate_dry.json" '"updated"[[:space:]]*:[[:space:]]*[1-9][0-9]*'
require_contains "$TMP_ROOT/rotate_dry.json" '"skipped_unsupported"[[:space:]]*:[[:space:]]*1'

(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels rotate-token-env --all --kind telegram_bot --from MOSAIC_TELEGRAM_BOT_TOKEN_OLD --to MOSAIC_TELEGRAM_BOT_TOKEN_NEW --report-out "$TMP_ROOT/rotation-report.json" >"$TMP_ROOT/rotate_apply.json")
require_contains "$TMP_ROOT/rotate_apply.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/rotate_apply.json" '"updated"[[:space:]]*:[[:space:]]*1'
require_contains "$TMP_ROOT/rotate_apply.json" '"report_path"'
require_contains "$TMP_ROOT/rotation-report.json" '"schema"[[:space:]]*:[[:space:]]*"mosaic\.channels\.token-rotation-report\.v1"'
require_contains "$TMP_ROOT/rotation-report.json" '"to_token_env"[[:space:]]*:[[:space:]]*"MOSAIC_TELEGRAM_BOT_TOKEN_NEW"'

(cd "$DST_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels login "$TG_CHANNEL_ID" >"$TMP_ROOT/login_after_rotate.json")
require_contains "$TMP_ROOT/login_after_rotate.json" '"token_env"[[:space:]]*:[[:space:]]*"MOSAIC_TELEGRAM_BOT_TOKEN_NEW"'

log "Step 4: channels runtime ops (status/capabilities/resolve/send/test/logs)"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels add --name src-terminal --kind terminal >"$TMP_ROOT/add_src_terminal.json")
TERMINAL_CHANNEL_ID="$(extract_first_match "$TMP_ROOT/add_src_terminal.json" 'ch_[0-9a-f-]{8,}')"

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels send "$TERMINAL_CHANNEL_ID" --text "terminal smoke" >"$TMP_ROOT/channels_send_terminal.json")
require_contains "$TMP_ROOT/channels_send_terminal.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels test "$SRC_CHANNEL_ID" >"$TMP_ROOT/channels_test_src.json")
require_contains "$TMP_ROOT/channels_test_src.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels status >"$TMP_ROOT/channels_status.json")
require_contains "$TMP_ROOT/channels_status.json" '"total_channels"[[:space:]]*:[[:space:]]*2'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels capabilities --target "$SRC_CHANNEL_ID" >"$TMP_ROOT/channels_capabilities.json")
require_contains "$TMP_ROOT/channels_capabilities.json" '"slack_webhook"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels resolve --channel slack_webhook src >"$TMP_ROOT/channels_resolve.json")
require_contains "$TMP_ROOT/channels_resolve.json" "$SRC_CHANNEL_ID"

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json channels logs --channel "$SRC_CHANNEL_ID" --tail 20 >"$TMP_ROOT/channels_logs.json")
require_contains "$TMP_ROOT/channels_logs.json" '"events"'

log "Step 5: gateway/nodes/devices/pairing control plane"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json nodes list >"$TMP_ROOT/nodes_list.json")
require_contains "$TMP_ROOT/nodes_list.json" '"id"[[:space:]]*:[[:space:]]*"local"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json pairing request --device dev-smoke --node local --reason "smoke pairing" >"$TMP_ROOT/pairing_request.json")
PAIRING_REQUEST_ID="$(extract_first_match "$TMP_ROOT/pairing_request.json" 'pr-[0-9-]+')"
require_contains "$TMP_ROOT/pairing_request.json" '"status"[[:space:]]*:[[:space:]]*"pending"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json pairing approve "$PAIRING_REQUEST_ID" >"$TMP_ROOT/pairing_approve.json")
require_contains "$TMP_ROOT/pairing_approve.json" '"status"[[:space:]]*:[[:space:]]*"approved"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json pairing request --device dev-smoke-reject --node local --reason "smoke reject pairing" >"$TMP_ROOT/pairing_request_reject.json")
PAIRING_REJECT_REQUEST_ID="$(extract_first_match "$TMP_ROOT/pairing_request_reject.json" 'pr-[0-9-]+')"
require_contains "$TMP_ROOT/pairing_request_reject.json" '"status"[[:space:]]*:[[:space:]]*"pending"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json pairing reject "$PAIRING_REJECT_REQUEST_ID" --reason "smoke rejected" >"$TMP_ROOT/pairing_reject.json")
require_contains "$TMP_ROOT/pairing_reject.json" '"status"[[:space:]]*:[[:space:]]*"rejected"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json pairing list --status rejected >"$TMP_ROOT/pairing_list_rejected.json")
require_contains "$TMP_ROOT/pairing_list_rejected.json" "$PAIRING_REJECT_REQUEST_ID"

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json devices rotate dev-smoke >"$TMP_ROOT/devices_rotate.json")
require_contains "$TMP_ROOT/devices_rotate.json" '"token_version"[[:space:]]*:[[:space:]]*2'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json gateway start >"$TMP_ROOT/gateway_start.json")
require_contains "$TMP_ROOT/gateway_start.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json gateway probe >"$TMP_ROOT/gateway_probe.json")
require_contains "$TMP_ROOT/gateway_probe.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json gateway discover >"$TMP_ROOT/gateway_discover.json")
require_contains "$TMP_ROOT/gateway_discover.json" '"nodes.run"'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json gateway call status >"$TMP_ROOT/gateway_call_status.json")
require_contains "$TMP_ROOT/gateway_call_status.json" '"service"[[:space:]]*:[[:space:]]*"mosaic-gateway"'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json nodes run local --command "echo smoke-run" >"$TMP_ROOT/nodes_run.json")
require_contains "$TMP_ROOT/nodes_run.json" '"accepted"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json nodes invoke local status --params '{"detail":true}' >"$TMP_ROOT/nodes_invoke.json")
require_contains "$TMP_ROOT/nodes_invoke.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && MOSAIC_GATEWAY_TEST_MODE=1 cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json gateway stop >"$TMP_ROOT/gateway_stop.json")
require_contains "$TMP_ROOT/gateway_stop.json" '"stopped"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json nodes status local >"$TMP_ROOT/nodes_status.json")
require_contains "$TMP_ROOT/nodes_status.json" '"pairings"'
require_contains "$TMP_ROOT/nodes_status.json" '"total"[[:space:]]*:[[:space:]]*2'
require_contains "$TMP_ROOT/nodes_status.json" '"pending"[[:space:]]*:[[:space:]]*0'
require_contains "$TMP_ROOT/nodes_status.json" '"approved"[[:space:]]*:[[:space:]]*1'
require_contains "$TMP_ROOT/nodes_status.json" '"rejected"[[:space:]]*:[[:space:]]*1'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json devices revoke dev-smoke --reason "smoke cleanup" >"$TMP_ROOT/devices_revoke.json")
require_contains "$TMP_ROOT/devices_revoke.json" '"status"[[:space:]]*:[[:space:]]*"revoked"'

log "Step 6: hooks/webhooks/cron/system/logs"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json hooks add --name deploy-hook --event deploy --command "echo hook-smoke-ok" >"$TMP_ROOT/hooks_add.json")
require_contains "$TMP_ROOT/hooks_add.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json system event deploy --data '{"release":"smoke"}' >"$TMP_ROOT/system_event_deploy.json")
require_contains "$TMP_ROOT/system_event_deploy.json" '"hooks"'
require_contains "$TMP_ROOT/system_event_deploy.json" '"triggered"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json system list --tail 20 --name deploy >"$TMP_ROOT/system_list.json")
require_contains "$TMP_ROOT/system_list.json" '"events"'
require_contains "$TMP_ROOT/system_list.json" '"name"[[:space:]]*:[[:space:]]*"deploy"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json webhooks add --name deploy-wh --event deploy --path /inbound/deploy --method post >"$TMP_ROOT/webhooks_add.json")
require_contains "$TMP_ROOT/webhooks_add.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json webhooks resolve --path /inbound/deploy --method post --data '{"source":"webhook"}' >"$TMP_ROOT/webhooks_resolve.json")
require_contains "$TMP_ROOT/webhooks_resolve.json" '"result"'
require_contains "$TMP_ROOT/webhooks_resolve.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json cron add --name deploy-cron --event deploy --every 1 --data '{"source":"cron"}' >"$TMP_ROOT/cron_add.json")
require_contains "$TMP_ROOT/cron_add.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json cron tick >"$TMP_ROOT/cron_tick.json")
require_contains "$TMP_ROOT/cron_tick.json" '"triggered"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json cron logs --tail 20 >"$TMP_ROOT/cron_logs.json")
require_contains "$TMP_ROOT/cron_logs.json" '"events"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json hooks logs --tail 20 >"$TMP_ROOT/hooks_logs.json")
require_contains "$TMP_ROOT/hooks_logs.json" '"events"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json webhooks logs --tail 20 >"$TMP_ROOT/webhooks_logs.json")
require_contains "$TMP_ROOT/webhooks_logs.json" '"events"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json logs --tail 50 >"$TMP_ROOT/logs_tail.json")
require_contains "$TMP_ROOT/logs_tail.json" '"source"[[:space:]]*:[[:space:]]*"system"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json logs --tail 50 --source system >"$TMP_ROOT/logs_tail_system.json")
require_contains "$TMP_ROOT/logs_tail_system.json" '"source"[[:space:]]*:[[:space:]]*"system"'

log "Step 7: browser/runtime presence"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser start >"$TMP_ROOT/browser_start.json")
require_contains "$TMP_ROOT/browser_start.json" '"running"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser navigate --url "mock://ok?title=Smoke+Docs" >"$TMP_ROOT/browser_open.json")
require_contains "$TMP_ROOT/browser_open.json" '"http_status"[[:space:]]*:[[:space:]]*200'
BROWSER_VISIT_ID="$(sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP_ROOT/browser_open.json" | head -n1)"
if [[ -z "$BROWSER_VISIT_ID" ]]; then
  echo "error: failed to parse browser visit id" >&2
  cat "$TMP_ROOT/browser_open.json" >&2
  exit 1
fi

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser history --tail 10 >"$TMP_ROOT/browser_history.json")
require_contains "$TMP_ROOT/browser_history.json" '"visits"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser tabs --tail 10 >"$TMP_ROOT/browser_tabs.json")
require_contains "$TMP_ROOT/browser_tabs.json" '"active_visit_id"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser status >"$TMP_ROOT/browser_status.json")
require_contains "$TMP_ROOT/browser_status.json" '"running"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser focus "$BROWSER_VISIT_ID" >"$TMP_ROOT/browser_focus.json")
require_contains "$TMP_ROOT/browser_focus.json" '"active_visit_id"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser snapshot >"$TMP_ROOT/browser_snapshot.json")
require_contains "$TMP_ROOT/browser_snapshot.json" '"snapshot"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser screenshot --out "$TMP_ROOT/browser-shot.txt" >"$TMP_ROOT/browser_screenshot.json")
require_contains "$TMP_ROOT/browser_screenshot.json" '"output"'
require_contains "$TMP_ROOT/browser-shot.txt" 'MOSAIC_BROWSER_SCREENSHOT_V1'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser stop >"$TMP_ROOT/browser_stop.json")
require_contains "$TMP_ROOT/browser_stop.json" '"running"[[:space:]]*:[[:space:]]*false'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json system presence >"$TMP_ROOT/system_presence.json")
require_contains "$TMP_ROOT/system_presence.json" '"presence"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json logs --tail 500 >"$TMP_ROOT/logs_after_browser.json")
require_contains "$TMP_ROOT/logs_after_browser.json" '"source"[[:space:]]*:[[:space:]]*"browser"'

log "Step 8: approvals/sandbox policies"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals get >"$TMP_ROOT/approvals_get.json")
require_contains "$TMP_ROOT/approvals_get.json" '"mode"[[:space:]]*:[[:space:]]*"confirm"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals check --command "echo smoke" >"$TMP_ROOT/approvals_check_confirm.json")
require_contains "$TMP_ROOT/approvals_check_confirm.json" '"decision"[[:space:]]*:[[:space:]]*"confirm"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals set deny >"$TMP_ROOT/approvals_set_deny.json")
require_contains "$TMP_ROOT/approvals_set_deny.json" '"mode"[[:space:]]*:[[:space:]]*"deny"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals allowlist add "echo" >"$TMP_ROOT/approvals_allowlist_add.json")
require_contains "$TMP_ROOT/approvals_allowlist_add.json" '"allowlist"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals allowlist list >"$TMP_ROOT/approvals_allowlist_list.json")
require_contains "$TMP_ROOT/approvals_allowlist_list.json" '"allowlist"'
require_contains "$TMP_ROOT/approvals_allowlist_list.json" '"echo"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json sandbox get >"$TMP_ROOT/sandbox_get.json")
require_contains "$TMP_ROOT/sandbox_get.json" '"profile"[[:space:]]*:[[:space:]]*"standard"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json sandbox set restricted >"$TMP_ROOT/sandbox_set.json")
require_contains "$TMP_ROOT/sandbox_set.json" '"profile"[[:space:]]*:[[:space:]]*"restricted"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json sandbox check --command "curl https://example.com" >"$TMP_ROOT/sandbox_check.json")
require_contains "$TMP_ROOT/sandbox_check.json" '"decision"[[:space:]]*:[[:space:]]*"deny"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json sandbox list >"$TMP_ROOT/sandbox_list.json")
require_contains "$TMP_ROOT/sandbox_list.json" '"profiles"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json sandbox explain --profile restricted >"$TMP_ROOT/sandbox_explain.json")
require_contains "$TMP_ROOT/sandbox_explain.json" '"profile"'

log "Step 9: agents routing + ask"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json agents add --id writer --name Writer --model mock-model --set-default --route ask >"$TMP_ROOT/agents_add.json")
require_contains "$TMP_ROOT/agents_add.json" '"id"[[:space:]]*:[[:space:]]*"writer"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json agents route resolve --route ask >"$TMP_ROOT/agents_resolve.json")
require_contains "$TMP_ROOT/agents_resolve.json" '"agent_id"[[:space:]]*:[[:space:]]*"writer"'

(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="agents-smoke-ok" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json ask "hello agents" >"$TMP_ROOT/agents_ask.json")
require_contains "$TMP_ROOT/agents_ask.json" '"response"[[:space:]]*:[[:space:]]*"agents-smoke-ok"'

log "Step 10: memory index/search/status/clear"
printf "Rust memory smoke test document\n" >"$SRC_DIR/memory-smoke.txt"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory index --path . --max-files 200 >"$TMP_ROOT/memory_index.json")
require_contains "$TMP_ROOT/memory_index.json" '"indexed_documents"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory search rust --limit 5 >"$TMP_ROOT/memory_search.json")
require_contains "$TMP_ROOT/memory_search.json" '"total_hits"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory status >"$TMP_ROOT/memory_status.json")
require_contains "$TMP_ROOT/memory_status.json" '"indexed_documents"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory clear >"$TMP_ROOT/memory_clear.json")
require_contains "$TMP_ROOT/memory_clear.json" '"removed_index"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/memory_clear.json" '"removed_status"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory status >"$TMP_ROOT/memory_status_after_clear.json")
require_contains "$TMP_ROOT/memory_status_after_clear.json" '"indexed_documents"[[:space:]]*:[[:space:]]*0'

log "Step 11: security audit + baseline + sarif"
printf "API_KEY = \"sk-live-secret-value-123456\"\n" >"$SRC_DIR/secrets.env"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json security audit --path . --deep >"$TMP_ROOT/security_audit.json")
require_contains "$TMP_ROOT/security_audit.json" '"findings"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json security audit --path . --update-baseline >"$TMP_ROOT/security_update_baseline.json")
require_contains "$TMP_ROOT/security_update_baseline.json" '"updated"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json security baseline show >"$TMP_ROOT/security_baseline_show.json")
require_contains "$TMP_ROOT/security_baseline_show.json" '"exists"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json security audit --path . --sarif-output scan.sarif >"$TMP_ROOT/security_sarif_output.json")
require_contains "$TMP_ROOT/security_sarif_output.json" '"sarif_output"'
require_contains "$SRC_DIR/scan.sarif" '"version"[[:space:]]*:[[:space:]]*"2\.1\.0"'

log "Step 12: plugins/skills install/list/check/remove"
mkdir -p "$SRC_DIR/sample-plugin" "$SRC_DIR/writer"
cat >"$SRC_DIR/sample-plugin/plugin.toml" <<'EOF'
[plugin]
id = "sample_plugin"
name = "Sample Plugin"
version = "0.1.0"
EOF
cat >"$SRC_DIR/writer/SKILL.md" <<'EOF'
# Writer
Generate concise release notes.
EOF

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json plugins install --path sample-plugin >"$TMP_ROOT/plugins_install.json")
require_contains "$TMP_ROOT/plugins_install.json" '"id"[[:space:]]*:[[:space:]]*"sample_plugin"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json skills install --path writer >"$TMP_ROOT/skills_install.json")
require_contains "$TMP_ROOT/skills_install.json" '"id"[[:space:]]*:[[:space:]]*"writer"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json plugins check sample_plugin >"$TMP_ROOT/plugins_check.json")
require_contains "$TMP_ROOT/plugins_check.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json plugins list --source project >"$TMP_ROOT/plugins_list_project.json")
require_contains "$TMP_ROOT/plugins_list_project.json" '"source_filter"[[:space:]]*:[[:space:]]*"project"'
require_contains "$TMP_ROOT/plugins_list_project.json" '"id"[[:space:]]*:[[:space:]]*"sample_plugin"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json skills check writer >"$TMP_ROOT/skills_check.json")
require_contains "$TMP_ROOT/skills_check.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json skills list --source project >"$TMP_ROOT/skills_list_project.json")
require_contains "$TMP_ROOT/skills_list_project.json" '"source_filter"[[:space:]]*:[[:space:]]*"project"'
require_contains "$TMP_ROOT/skills_list_project.json" '"id"[[:space:]]*:[[:space:]]*"writer"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json plugins remove sample_plugin >"$TMP_ROOT/plugins_remove.json")
require_contains "$TMP_ROOT/plugins_remove.json" '"removed"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json skills remove writer >"$TMP_ROOT/skills_remove.json")
require_contains "$TMP_ROOT/skills_remove.json" '"removed"[[:space:]]*:[[:space:]]*true'

log "Step 13: status/health/doctor"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json status >"$TMP_ROOT/status.json")
require_contains "$TMP_ROOT/status.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json health >"$TMP_ROOT/health.json")
require_contains "$TMP_ROOT/health.json" '"type"[[:space:]]*:[[:space:]]*"health"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json doctor >"$TMP_ROOT/doctor.json")
require_contains "$TMP_ROOT/doctor.json" '"type"[[:space:]]*:[[:space:]]*"doctor"'

log "Step 14: compatibility command family (docs/dns/tui/qr/clawbot/completion/directory/dashboard/update/reset/uninstall)"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json docs >"$TMP_ROOT/docs_list.json")
require_contains "$TMP_ROOT/docs_list.json" '"topics"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json docs gateway >"$TMP_ROOT/docs_gateway.json")
require_contains "$TMP_ROOT/docs_gateway.json" '"topic"[[:space:]]*:[[:space:]]*"gateway"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json dns resolve localhost --port 443 >"$TMP_ROOT/dns_localhost.json")
require_contains "$TMP_ROOT/dns_localhost.json" '"addresses"'

(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="tui-smoke-ok" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json tui --prompt "hello tui" >"$TMP_ROOT/tui_prompt.json")
require_contains "$TMP_ROOT/tui_prompt.json" '"response"[[:space:]]*:[[:space:]]*"tui-smoke-ok"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json qr encode "smoke payload" --render ascii >"$TMP_ROOT/qr_ascii.json")
require_contains "$TMP_ROOT/qr_ascii.json" '"render"[[:space:]]*:[[:space:]]*"ascii"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json qr encode "smoke payload" --render png --output "$TMP_ROOT/qr/smoke.png" --module-size 6 >"$TMP_ROOT/qr_png.json")
require_contains "$TMP_ROOT/qr_png.json" '"render"[[:space:]]*:[[:space:]]*"png"'
if [[ ! -f "$TMP_ROOT/qr/smoke.png" ]]; then
  echo "error: expected qr png output at $TMP_ROOT/qr/smoke.png" >&2
  exit 1
fi

(cd "$SRC_DIR" && MOSAIC_MOCK_CHAT_RESPONSE="clawbot-send-ok" cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot send "hello clawbot send" >"$TMP_ROOT/clawbot_send.json")
require_contains "$TMP_ROOT/clawbot_send.json" '"response"[[:space:]]*:[[:space:]]*"clawbot-send-ok"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json clawbot status >"$TMP_ROOT/clawbot_status.json")
require_contains "$TMP_ROOT/clawbot_status.json" '"configured"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- completion shell zsh >"$TMP_ROOT/completion_zsh.sh")
require_contains "$TMP_ROOT/completion_zsh.sh" "compdef"

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- completion install zsh --dir "$TMP_ROOT/completions" >"$TMP_ROOT/completion_install.txt")
if [[ ! -f "$TMP_ROOT/completions/_mosaic" ]]; then
  echo "error: completion install did not create $TMP_ROOT/completions/_mosaic" >&2
  exit 1
fi

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json directory --ensure --check-writable >"$TMP_ROOT/directory.json")
require_contains "$TMP_ROOT/directory.json" '"root_dir"'
require_contains "$TMP_ROOT/directory.json" '"ensured"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/directory.json" '"checks"'
require_contains "$TMP_ROOT/directory.json" '"writable"[[:space:]]*:[[:space:]]*(true|false)'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json dashboard >"$TMP_ROOT/dashboard.json")
require_contains "$TMP_ROOT/dashboard.json" '"ok"[[:space:]]*:[[:space:]]*true'
require_contains "$TMP_ROOT/dashboard.json" '"dashboard"'
require_contains "$TMP_ROOT/dashboard.json" '"channels"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json update >"$TMP_ROOT/update_local.json")
require_contains "$TMP_ROOT/update_local.json" '"current_version"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json update --check --source mock://v9.9.9 >"$TMP_ROOT/update_check.json")
require_contains "$TMP_ROOT/update_check.json" '"latest_version"[[:space:]]*:[[:space:]]*"v9\.9\.9"'
require_contains "$TMP_ROOT/update_check.json" '"update_available"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --json update --check --source mock://0.0.0 >"$TMP_ROOT/update_check_old.json")
require_contains "$TMP_ROOT/update_check_old.json" '"latest_version"[[:space:]]*:[[:space:]]*"0\.0\.0"'
require_contains "$TMP_ROOT/update_check_old.json" '"update_available"[[:space:]]*:[[:space:]]*false'

MAINT_DIR="$TMP_ROOT/maint"
mkdir -p "$MAINT_DIR"
(cd "$MAINT_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state setup --base-url mock://mock-model --model mock-model >/dev/null)
(cd "$MAINT_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json reset >"$TMP_ROOT/reset_yes.json")
require_contains "$TMP_ROOT/reset_yes.json" '"ok"[[:space:]]*:[[:space:]]*true'
(cd "$MAINT_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json uninstall >"$TMP_ROOT/uninstall_yes.json")
require_contains "$TMP_ROOT/uninstall_yes.json" '"ok"[[:space:]]*:[[:space:]]*true'

log "Smoke test completed successfully"
echo "session_id=$SESSION_ID"
echo "src_channel_id=$SRC_CHANNEL_ID"
echo "telegram_channel_id=$TG_CHANNEL_ID"
echo "terminal_channel_id=$TERMINAL_CHANNEL_ID"
echo "pairing_request_id=$PAIRING_REQUEST_ID"
echo "pairing_reject_request_id=$PAIRING_REJECT_REQUEST_ID"
echo "tmp_dir=$TMP_ROOT"
echo "Use KEEP_TMP=1 to keep artifacts."
