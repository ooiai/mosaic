#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
WORKDIR=$(mktemp -d "${TMPDIR:-/tmp}/mosaic-release-smoke.XXXXXX")
cleanup() {
    rm -rf "$WORKDIR"
}
trap cleanup EXIT INT TERM

run_cli() {
    cargo run --manifest-path "$ROOT/Cargo.toml" -p mosaic-cli -- "$@"
}

cd "$WORKDIR"
run_cli setup init --profile ollama-qwen3
run_cli setup validate >/dev/null
run_cli setup doctor >/dev/null
run_cli config show >/dev/null
run_cli config sources >/dev/null
run_cli model list >/dev/null
run_cli gateway status >/dev/null
if [ "${MOSAIC_REAL_TESTS:-0}" != "1" ] || [ -z "${OPENAI_API_KEY:-}" ]; then
    printf 'release smoke ok\nworkspace=%s\nmode=config-only\n' "$WORKDIR"
    exit 0
fi

sh "$ROOT/scripts/test-full-stack-example.sh" openai-webchat >/dev/null
printf 'release smoke ok\nworkspace=%s\nmode=openai-webchat\n' "$WORKDIR"
