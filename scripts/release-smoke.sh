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
run_cli setup init --dev-mock
run_cli setup validate >/dev/null
run_cli setup doctor >/dev/null
run_cli config show >/dev/null
run_cli config sources >/dev/null
run_cli model list >/dev/null
run_cli gateway status >/dev/null
run_cli run "$ROOT/examples/time-now-agent.yaml" --session release-smoke >/dev/null
TRACE_PATH=$(find "$WORKDIR/.mosaic/runs" -maxdepth 1 -name '*.json' | head -n 1)
if [ -z "$TRACE_PATH" ]; then
    echo "release smoke did not produce a trace" >&2
    exit 1
fi
RUN_ID=$(basename "$TRACE_PATH" .json)
run_cli session list >/dev/null
run_cli session show release-smoke >/dev/null
run_cli inspect "$TRACE_PATH" >/dev/null
run_cli gateway audit --limit 5 >/dev/null
run_cli gateway replay --limit 5 >/dev/null
run_cli gateway incident "$RUN_ID" >/dev/null
cp -R "$WORKDIR/.mosaic" "$WORKDIR/.mosaic.backup"
rm -rf "$WORKDIR/.mosaic"
cp -R "$WORKDIR/.mosaic.backup" "$WORKDIR/.mosaic"
run_cli session show release-smoke >/dev/null
run_cli gateway status >/dev/null
run_cli gateway audit --limit 5 >/dev/null
run_cli inspect "$TRACE_PATH" >/dev/null
printf 'release smoke ok\nworkspace=%s\ntrace=%s\nrun_id=%s\n' "$WORKDIR" "$TRACE_PATH" "$RUN_ID"
