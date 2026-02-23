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
require_contains "$TMP_ROOT/nodes_status.json" '"total"[[:space:]]*:[[:space:]]*1'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json devices revoke dev-smoke --reason "smoke cleanup" >"$TMP_ROOT/devices_revoke.json")
require_contains "$TMP_ROOT/devices_revoke.json" '"status"[[:space:]]*:[[:space:]]*"revoked"'

log "Step 6: hooks/webhooks/cron/system/logs"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json hooks add --name deploy-hook --event deploy --command "echo hook-smoke-ok" >"$TMP_ROOT/hooks_add.json")
require_contains "$TMP_ROOT/hooks_add.json" '"ok"[[:space:]]*:[[:space:]]*true'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --yes --json system event deploy --data '{"release":"smoke"}' >"$TMP_ROOT/system_event_deploy.json")
require_contains "$TMP_ROOT/system_event_deploy.json" '"hooks"'
require_contains "$TMP_ROOT/system_event_deploy.json" '"triggered"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

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

log "Step 7: browser/runtime presence"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser open --url "mock://ok?title=Smoke+Docs" >"$TMP_ROOT/browser_open.json")
require_contains "$TMP_ROOT/browser_open.json" '"http_status"[[:space:]]*:[[:space:]]*200'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json browser history --tail 10 >"$TMP_ROOT/browser_history.json")
require_contains "$TMP_ROOT/browser_history.json" '"visits"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json system presence >"$TMP_ROOT/system_presence.json")
require_contains "$TMP_ROOT/system_presence.json" '"presence"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json logs --tail 100 >"$TMP_ROOT/logs_after_browser.json")
require_contains "$TMP_ROOT/logs_after_browser.json" '"source"[[:space:]]*:[[:space:]]*"browser"'

log "Step 8: approvals/sandbox policies"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals get >"$TMP_ROOT/approvals_get.json")
require_contains "$TMP_ROOT/approvals_get.json" '"mode"[[:space:]]*:[[:space:]]*"confirm"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals set deny >"$TMP_ROOT/approvals_set_deny.json")
require_contains "$TMP_ROOT/approvals_set_deny.json" '"mode"[[:space:]]*:[[:space:]]*"deny"'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json approvals allowlist add "echo" >"$TMP_ROOT/approvals_allowlist_add.json")
require_contains "$TMP_ROOT/approvals_allowlist_add.json" '"allowlist"'

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

log "Step 10: memory index/search/status"
printf "Rust memory smoke test document\n" >"$SRC_DIR/memory-smoke.txt"
(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory index --path . --max-files 200 >"$TMP_ROOT/memory_index.json")
require_contains "$TMP_ROOT/memory_index.json" '"indexed_documents"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory search rust --limit 5 >"$TMP_ROOT/memory_search.json")
require_contains "$TMP_ROOT/memory_search.json" '"total_hits"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json memory status >"$TMP_ROOT/memory_status.json")
require_contains "$TMP_ROOT/memory_status.json" '"indexed_documents"[[:space:]]*:[[:space:]]*[1-9][0-9]*'

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

(cd "$SRC_DIR" && cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- --project-state --json skills check writer >"$TMP_ROOT/skills_check.json")
require_contains "$TMP_ROOT/skills_check.json" '"ok"[[:space:]]*:[[:space:]]*true'

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

log "Smoke test completed successfully"
echo "session_id=$SESSION_ID"
echo "src_channel_id=$SRC_CHANNEL_ID"
echo "telegram_channel_id=$TG_CHANNEL_ID"
echo "terminal_channel_id=$TERMINAL_CHANNEL_ID"
echo "pairing_request_id=$PAIRING_REQUEST_ID"
echo "tmp_dir=$TMP_ROOT"
echo "Use KEEP_TMP=1 to keep artifacts."
