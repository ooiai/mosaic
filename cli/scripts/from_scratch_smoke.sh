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

log "Smoke test completed successfully"
echo "session_id=$SESSION_ID"
echo "src_channel_id=$SRC_CHANNEL_ID"
echo "telegram_channel_id=$TG_CHANNEL_ID"
echo "tmp_dir=$TMP_ROOT"
echo "Use KEEP_TMP=1 to keep artifacts."
