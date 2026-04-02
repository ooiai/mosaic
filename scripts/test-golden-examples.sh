#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
WORKDIR=$(mktemp -d "${TMPDIR:-/tmp}/mosaic-golden-examples.XXXXXX")
cleanup() {
    rm -rf "$WORKDIR"
}
trap cleanup EXIT INT TERM

run_cli() {
    cargo run --manifest-path "$ROOT/Cargo.toml" -p mosaic-cli -- "$@"
}

sh "$ROOT/scripts/verify-test-matrix.sh" >/dev/null

ln -s "$ROOT/examples" "$WORKDIR/examples"
ln -s "$ROOT/scripts" "$WORKDIR/scripts"
cp "$ROOT/README.md" "$WORKDIR/README.md"

cd "$WORKDIR"

run_cli setup init --profile ollama-qwen3 >/dev/null
run_cli setup validate >/dev/null
run_cli setup doctor >/dev/null
run_cli config show >/dev/null

if [ "${MOSAIC_REAL_TESTS:-0}" != "1" ] || [ -z "${OPENAI_API_KEY:-}" ]; then
    printf 'golden examples ok\nworkspace=%s\nmode=config-only\n' "$WORKDIR"
    exit 0
fi

run_cli run "$ROOT/examples/time-now-agent.yaml" --session golden-time >/dev/null
run_cli run "$ROOT/examples/workflows/research-brief.yaml" --workflow research_brief --session golden-workflow >/dev/null
run_cli run "$ROOT/examples/mcp-filesystem.yaml" --session golden-mcp >/dev/null
sh "$ROOT/scripts/test-full-stack-example.sh" openai-webchat >/dev/null

TRACE_PATH=$(find "$WORKDIR/.mosaic/runs" -maxdepth 1 -name '*.json' | sort | tail -n 1)
if [ -z "$TRACE_PATH" ]; then
    echo "golden example verification did not produce a trace" >&2
    exit 1
fi

run_cli inspect "$TRACE_PATH" >/dev/null

printf 'golden examples ok\nworkspace=%s\ntrace=%s\n' "$WORKDIR" "$TRACE_PATH"
