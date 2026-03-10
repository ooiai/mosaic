#!/usr/bin/env bash
set -euo pipefail

CLI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$CLI_DIR"

STAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
REPORT_DIR="$CLI_DIR/reports"
REPORT_PATH="$REPORT_DIR/mcp-freeze-${STAMP}.log"
LATEST_PATH="$REPORT_DIR/mcp-freeze-latest.log"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/mosaic-mcp-freeze-XXXXXX")"
WORK_DIR="$TMP_ROOT/work"
mkdir -p "$REPORT_DIR" "$WORK_DIR"

cleanup() {
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

log() {
  printf "\n[%s] %s\n" "$(date +%H:%M:%S)" "$*"
}

run() {
  log "$*"
  "$@"
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

run_capture() {
  local output="$1"
  shift
  log "$* > $output"
  "$@" >"$output"
}

{
  echo "Mosaic MCP freeze check"
  echo "UTC: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  run cargo fmt --all --check
  run cargo test -p mosaic-mcp
  run cargo test -p mosaic-cli --test mcp_ops --test command_surface --test error_codes --test json_contract_modules
  run bash ../site/scripts/check_docs.sh --report-dir /tmp --report-prefix local-docs-check-mcp-freeze

  cat >"$TMP_ROOT/mock-mcp.sh" <<'EOF'
#!/bin/sh
body='{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}'
len=$(printf %s "$body" | wc -c | tr -d ' ')
printf 'Content-Length: %s\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n%s' "$len" "$body"
sleep 1
EOF
  chmod +x "$TMP_ROOT/mock-mcp.sh"

  run_capture "$TMP_ROOT/mcp_add.json" \
    cargo run --manifest-path "$CLI_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- \
    --project-state --json mcp add --name freeze-mcp --command "$TMP_ROOT/mock-mcp.sh" --cwd "$WORK_DIR" --env MCP_MODE=freeze
  require_contains "$TMP_ROOT/mcp_add.json" '"ok"[[:space:]]*:[[:space:]]*true'

  MCP_SERVER_ID="$(sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP_ROOT/mcp_add.json" | head -n1)"
  if [[ -z "$MCP_SERVER_ID" ]]; then
    echo "error: failed to parse mcp server id from add output" >&2
    cat "$TMP_ROOT/mcp_add.json" >&2
    exit 1
  fi

  run_capture "$TMP_ROOT/mcp_diagnose.json" \
    cargo run --manifest-path "$CLI_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- \
    --project-state --json mcp diagnose "$MCP_SERVER_ID" --timeout-ms 1000
  require_contains "$TMP_ROOT/mcp_diagnose.json" '"handshake_ok"[[:space:]]*:[[:space:]]*true'
  require_contains "$TMP_ROOT/mcp_diagnose.json" '"initialized_notification_sent"[[:space:]]*:[[:space:]]*true'
  require_contains "$TMP_ROOT/mcp_diagnose.json" '"session_ready"[[:space:]]*:[[:space:]]*true'
  require_contains "$TMP_ROOT/mcp_diagnose.json" '"healthy"[[:space:]]*:[[:space:]]*true'

  run_capture "$TMP_ROOT/mcp_check_deep.json" \
    cargo run --manifest-path "$CLI_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- \
    --project-state --json mcp check --all --deep --timeout-ms 1000
  require_contains "$TMP_ROOT/mcp_check_deep.json" '"protocol_ok"[[:space:]]*:[[:space:]]*1'
  require_contains "$TMP_ROOT/mcp_check_deep.json" '"healthy"[[:space:]]*:[[:space:]]*1'

  run_capture "$TMP_ROOT/mcp_disable.json" \
    cargo run --manifest-path "$CLI_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- \
    --project-state --json mcp disable "$MCP_SERVER_ID"
  require_contains "$TMP_ROOT/mcp_disable.json" '"enabled"[[:space:]]*:[[:space:]]*false'

  run_capture "$TMP_ROOT/mcp_repair.json" \
    cargo run --manifest-path "$CLI_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- \
    --project-state --json mcp repair "$MCP_SERVER_ID" --timeout-ms 1000
  require_contains "$TMP_ROOT/mcp_repair.json" '"enabled_server"'
  require_contains "$TMP_ROOT/mcp_repair.json" '"protocol_session_ready"[[:space:]]*:[[:space:]]*true'

  run_capture "$TMP_ROOT/mcp_remove.json" \
    cargo run --manifest-path "$CLI_DIR/Cargo.toml" -p mosaic-cli --bin mosaic -- \
    --project-state --json mcp remove "$MCP_SERVER_ID"
  require_contains "$TMP_ROOT/mcp_remove.json" '"removed"[[:space:]]*:[[:space:]]*true'

  log "mcp freeze checks passed"
} 2>&1 | tee "$REPORT_PATH"

cp "$REPORT_PATH" "$LATEST_PATH"

echo "report: $REPORT_PATH"
echo "latest: $LATEST_PATH"
