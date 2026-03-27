#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
MODE=${1:-mock}
WORKDIR=$(mktemp -d "${TMPDIR:-/tmp}/mosaic-full-stack.XXXXXX")
PORT=${MOSAIC_FULL_STACK_PORT:-}
BASE_URL=""
GATEWAY_PID=""

cleanup() {
    if [ -n "$GATEWAY_PID" ]; then
        kill "$GATEWAY_PID" 2>/dev/null || true
        wait "$GATEWAY_PID" 2>/dev/null || true
    fi
    rm -rf "$WORKDIR"
}
trap cleanup EXIT INT TERM

run_cli() {
    cargo run --manifest-path "$ROOT/Cargo.toml" -p mosaic-cli -- "$@"
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        exit 1
    fi
}

wait_for_gateway() {
    attempts=0
    while [ "$attempts" -lt 80 ]; do
        if curl -sf "$BASE_URL/health" >/dev/null 2>&1; then
            return 0
        fi
        attempts=$((attempts + 1))
        sleep 0.25
    done
    return 1
}

require_cmd curl
require_cmd python3

if [ "$MODE" = "openai" ]; then
    if [ "${MOSAIC_REAL_TESTS:-0}" != "1" ]; then
        echo "full-stack openai skipped: set MOSAIC_REAL_TESTS=1 to enable" >&2
        exit 0
    fi
    if [ -z "${OPENAI_API_KEY:-}" ]; then
        echo "full-stack openai skipped: OPENAI_API_KEY is not set" >&2
        exit 0
    fi
fi

if [ -z "$PORT" ]; then
    PORT=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')
fi
BASE_URL="http://127.0.0.1:${PORT}"

cd "$WORKDIR"

run_cli setup init >/dev/null

case "$MODE" in
    mock)
        cp "$ROOT/examples/full-stack/mock-telegram.config.yaml" ".mosaic/config.yaml"
        export MOSAIC_TELEGRAM_SECRET_TOKEN="${MOSAIC_TELEGRAM_SECRET_TOKEN:-full-stack-secret}"
        ;;
    openai)
        cp "$ROOT/examples/full-stack/openai-telegram.config.yaml" ".mosaic/config.yaml"
        if [ -n "${MOSAIC_TEST_OPENAI_BASE_URL:-}" ]; then
            tmp=".mosaic/config.yaml.tmp"
            sed "s|https://api.openai.com/v1|$MOSAIC_TEST_OPENAI_BASE_URL|" \
                ".mosaic/config.yaml" >"$tmp"
            mv "$tmp" ".mosaic/config.yaml"
        fi
        if [ -n "${MOSAIC_TEST_OPENAI_MODEL:-}" ]; then
            tmp=".mosaic/config.yaml.tmp"
            sed "s|gpt-5.4-mini|$MOSAIC_TEST_OPENAI_MODEL|" \
                ".mosaic/config.yaml" >"$tmp"
            mv "$tmp" ".mosaic/config.yaml"
        fi
        export MOSAIC_TELEGRAM_SECRET_TOKEN="${MOSAIC_TEST_TELEGRAM_SECRET:-${MOSAIC_TELEGRAM_SECRET_TOKEN:-full-stack-secret}}"
        ;;
    *)
        echo "usage: $0 [mock|openai]" >&2
        exit 1
        ;;
esac

run_cli setup validate >/dev/null
run_cli setup doctor >/dev/null
run_cli model list >/dev/null

run_cli gateway serve --http "127.0.0.1:${PORT}" >"$WORKDIR/gateway.stdout" 2>"$WORKDIR/gateway.stderr" &
GATEWAY_PID=$!

if ! wait_for_gateway; then
    echo "gateway did not become ready" >&2
    sed -n '1,120p' "$WORKDIR/gateway.stdout" >&2 || true
    sed -n '1,120p' "$WORKDIR/gateway.stderr" >&2 || true
    exit 1
fi

curl -sf -X POST "$BASE_URL/ingress/telegram" \
    -H 'content-type: application/json' \
    -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
    --data @"$ROOT/examples/channels/telegram-update.json" \
    >"$WORKDIR/telegram-response.json"

run_cli gateway --attach "$BASE_URL" status >"$WORKDIR/gateway-status.txt"
run_cli gateway --attach "$BASE_URL" audit --limit 10 >"$WORKDIR/gateway-audit.txt"
run_cli gateway --attach "$BASE_URL" replay --limit 10 >"$WORKDIR/gateway-replay.txt"

SESSION_ID="telegram--100123-99"
run_cli session show "$SESSION_ID" >"$WORKDIR/session.txt"
grep -q 'channel: Some("telegram")' "$WORKDIR/session.txt"
grep -q 'thread_id: Some("99")' "$WORKDIR/session.txt"
grep -q 'session_route: gateway.channel/telegram/' "$WORKDIR/session.txt"

TRACE_PATH=$(find "$WORKDIR/.mosaic/runs" -maxdepth 1 -name '*.json' | sort | tail -n 1)
if [ -z "$TRACE_PATH" ]; then
    echo "full-stack example did not produce a trace" >&2
    exit 1
fi

run_cli inspect "$TRACE_PATH" >"$WORKDIR/inspect.txt"
RUN_ID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["run_id"])' "$TRACE_PATH")
run_cli gateway --attach "$BASE_URL" incident "$RUN_ID" >"$WORKDIR/incident.txt"

if [ ! -f "$WORKDIR/.mosaic/audit/incidents/${RUN_ID}.json" ]; then
    echo "incident bundle was not written for run ${RUN_ID}" >&2
    exit 1
fi

printf 'full-stack example ok\nmode=%s\nworkspace=%s\nsession=%s\ntrace=%s\nrun_id=%s\n' \
    "$MODE" "$WORKDIR" "$SESSION_ID" "$TRACE_PATH" "$RUN_ID"
