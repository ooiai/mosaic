#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CLI_DIR=$(cd "$SCRIPT_DIR/.." && pwd)

ITERATIONS=${ITERATIONS:-50}
CPU_TIMEOUT_MS=${CPU_TIMEOUT_MS:-5000}
KEEP_TMP=${KEEP_TMP:-0}
MOSAIC_BIN=${MOSAIC_BIN:-"$CLI_DIR/target/debug/mosaic"}

if ! [[ "$ITERATIONS" =~ ^[0-9]+$ ]] || [[ "$ITERATIONS" -lt 1 ]]; then
  echo "ITERATIONS must be a positive integer (got: $ITERATIONS)" >&2
  exit 1
fi

if ! [[ "$CPU_TIMEOUT_MS" =~ ^[0-9]+$ ]] || [[ "$CPU_TIMEOUT_MS" -lt 1 ]]; then
  echo "CPU_TIMEOUT_MS must be a positive integer (got: $CPU_TIMEOUT_MS)" >&2
  exit 1
fi

if [[ ! -x "$MOSAIC_BIN" ]]; then
  echo "[soak] building mosaic binary..."
  (cd "$CLI_DIR" && cargo build -p mosaic-cli --bin mosaic >/dev/null)
fi

if [[ ! -x "$MOSAIC_BIN" ]]; then
  echo "unable to locate executable mosaic binary at $MOSAIC_BIN" >&2
  exit 1
fi

TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/mosaic-plugin-soak-XXXXXX")
WORK_DIR="$TMP_DIR/workspace"

cleanup() {
  if [[ "$KEEP_TMP" == "1" ]]; then
    echo "[soak] keeping tmp dir: $TMP_DIR"
  else
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT

mkdir -p "$WORK_DIR/.mosaic/plugins/ok/hooks"
mkdir -p "$WORK_DIR/.mosaic/plugins/cpuwatch/hooks"
mkdir -p "$WORK_DIR/.mosaic/plugins/rss/hooks"

cat > "$WORK_DIR/.mosaic/plugins/ok/plugin.toml" <<'TOML'
[plugin]
id = "ok"
name = "OK"
version = "0.1.0"

[runtime]
run = "hooks/run.sh"
TOML

cat > "$WORK_DIR/.mosaic/plugins/cpuwatch/plugin.toml" <<'TOML'
[plugin]
id = "cpuwatch"
name = "CPU Watch"
version = "0.1.0"

[runtime]
run = "hooks/run.sh"
max_cpu_ms = 100
TOML

cat > "$WORK_DIR/.mosaic/plugins/rss/plugin.toml" <<'TOML'
[plugin]
id = "rss"
name = "RSS"
version = "0.1.0"

[runtime]
run = "hooks/run.sh"
max_rss_kb = 1
TOML

cat > "$WORK_DIR/.mosaic/plugins/ok/hooks/run.sh" <<'SH'
#!/bin/sh
echo ok
SH

cat > "$WORK_DIR/.mosaic/plugins/cpuwatch/hooks/run.sh" <<'SH'
#!/bin/sh
sleep 1
echo cpuwatch
SH

cat > "$WORK_DIR/.mosaic/plugins/rss/hooks/run.sh" <<'SH'
#!/bin/sh
sleep 0.1
echo rss
SH

run_mosaic() {
  (
    cd "$WORK_DIR"
    "$MOSAIC_BIN" --project-state --yes --json "$@"
  )
}

assert_ok_true() {
  local payload="$1"
  python3 - "$payload" <<'PY'
import json
import sys
payload = json.loads(sys.argv[1])
if payload.get("ok") is not True:
    raise SystemExit(f"expected ok=true payload, got: {payload}")
PY
}

assert_cpu_failure() {
  local payload="$1"
  python3 - "$payload" <<'PY'
import json
import sys
payload = json.loads(sys.argv[1])
err = payload.get("error") or {}
code = err.get("code")
msg = (err.get("message") or "").lower()
if code != "tool":
    raise SystemExit(f"expected tool error for cpuwatch, got: {payload}")
if "watchdog" not in msg and "resource limit exceeded" not in msg:
    raise SystemExit(f"expected watchdog/resource-limit failure message, got: {payload}")
PY
}

assert_rss_failure() {
  local payload="$1"
  python3 - "$payload" <<'PY'
import json
import sys
payload = json.loads(sys.argv[1])
err = payload.get("error") or {}
code = err.get("code")
if code not in {"tool", "validation"}:
    raise SystemExit(f"expected tool|validation error for rss plugin, got: {payload}")
PY
}

ok_runs=0
cpu_failures=0
rss_failures=0

for i in $(seq 1 "$ITERATIONS"); do
  ok_payload=$(run_mosaic plugins run ok)
  assert_ok_true "$ok_payload"
  ok_runs=$((ok_runs + 1))

  if cpu_payload=$(run_mosaic plugins run cpuwatch --timeout-ms "$CPU_TIMEOUT_MS"); then
    echo "[soak] iteration $i: expected cpuwatch failure but command succeeded" >&2
    exit 1
  fi
  assert_cpu_failure "$cpu_payload"
  cpu_failures=$((cpu_failures + 1))

  if rss_payload=$(run_mosaic plugins run rss); then
    echo "[soak] iteration $i: expected rss failure but command succeeded" >&2
    exit 1
  fi
  assert_rss_failure "$rss_payload"
  rss_failures=$((rss_failures + 1))

done

ok_event_lines=$(wc -l < "$WORK_DIR/.mosaic/data/plugin-events/ok.jsonl" 2>/dev/null || echo 0)
cpu_event_lines=$(wc -l < "$WORK_DIR/.mosaic/data/plugin-events/cpuwatch.jsonl" 2>/dev/null || echo 0)
rss_event_lines=$(wc -l < "$WORK_DIR/.mosaic/data/plugin-events/rss.jsonl" 2>/dev/null || echo 0)

echo "[soak] completed"
echo "iterations=$ITERATIONS"
echo "ok_runs=$ok_runs cpu_failures=$cpu_failures rss_failures=$rss_failures"
echo "event_lines.ok=$ok_event_lines"
echo "event_lines.cpuwatch=$cpu_event_lines"
echo "event_lines.rss=$rss_event_lines"
echo "workspace=$WORK_DIR"
