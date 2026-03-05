# Plugins and Skills (V3 Minimal Framework)

This module provides a local, CLI-first runtime for discovering and validating plugin and skill packages.

## Commands

```bash
# Plugins
mosaic --project-state plugins list
mosaic --project-state plugins list --source project
mosaic --project-state plugins info <plugin-id>
mosaic --project-state plugins check [plugin-id]
mosaic --project-state plugins install --path ./my-plugin [--force]
mosaic --project-state plugins enable <plugin-id>
mosaic --project-state plugins disable <plugin-id>
mosaic --project-state plugins doctor
mosaic --project-state --yes plugins run <plugin-id> --hook run --arg smoke [--timeout-ms 10000]
mosaic --project-state plugins remove <plugin-id>

# Skills
mosaic --project-state skills list
mosaic --project-state skills list --source project
mosaic --project-state skills info <skill-id>
mosaic --project-state skills check [skill-id]
mosaic --project-state skills install --path ./writer [--force]
mosaic --project-state skills remove <skill-id>

# Bind installed skills to an agent
mosaic --project-state agents update writer --skill writer
mosaic --project-state agents update writer --clear-skills
```

## Skills End-to-End (Recommended Flow)

### 1) Create a local skill package

```bash
mkdir -p ./writer
cat > ./writer/SKILL.md <<'EOF'
# Writer
Produce concise, structured answers.

## Rules
- Start with the direct answer.
- Keep examples short.
EOF
```

Minimum requirement is `SKILL.md` (non-empty and with at least one markdown heading).

### 2) Install and verify

```bash
mosaic --project-state skills install --path ./writer
mosaic --project-state skills list --source project
mosaic --project-state skills info writer
mosaic --project-state skills check writer
```

### 3) Bind skill to an agent

```bash
# create agent with skill
mosaic --project-state agents add --name Writer --id writer --skill writer --set-default --route ask

# or update existing agent
mosaic --project-state agents update writer --skill writer
mosaic --project-state agents show writer
```

`agents show` should include `skills: writer`.

### 4) Run and confirm behavior

```bash
mosaic --project-state ask --agent writer "summarize README"
```

For mock regression, you can capture the model request envelope:

```bash
MOSAIC_MOCK_CHAT_RESPONSE=ok \
MOSAIC_MOCK_CHAT_CAPTURE_PATH=./mock-chat-request.json \
mosaic --project-state ask --agent writer "hello"
```

Then inspect `mock-chat-request.json` and verify the system prompt contains:

- `BEGIN AGENT SKILL: writer`
- your `SKILL.md` content

### 5) Update, replace, remove

```bash
# update local skill content, then replace install
mosaic --project-state skills install --path ./writer --force

# clear skill binding from agent
mosaic --project-state agents update writer --clear-skills

# remove skill package
mosaic --project-state skills remove writer
```

## Skills Troubleshooting

- `skill '<id>' not found` when adding/updating agent:
  - run `mosaic --project-state skills list` first
  - ensure `--project-state` usage is consistent
- `skills check` fails:
  - verify `SKILL.md` exists and is non-empty
  - ensure first heading exists (`# ...`)
- agent runs but skill seems ignored:
  - run `mosaic --project-state agents show <id>` and check `skills`
  - verify route/default/`--agent` actually selects that agent
- skill removed after binding:
  - runtime will fail until you reinstall skill or run `agents update <id> --clear-skills`

## Skills Regression Checklist

```bash
# install/list/info/check
mosaic --project-state skills install --path ./writer --force
mosaic --project-state --json skills list
mosaic --project-state --json skills info writer
mosaic --project-state --json skills check writer

# bind/unbind
mosaic --project-state --json agents update writer --skill writer
mosaic --project-state --json agents show writer
mosaic --project-state --json agents update writer --clear-skills

# remove
mosaic --project-state --json skills remove writer
```

## Discovery Roots

The CLI discovers extensions from these roots in priority order:

1. Project state root (`.mosaic/plugins`, `.mosaic/skills` when `--project-state` is used)
2. `$CODEX_HOME/plugins` and `$CODEX_HOME/skills` (if `CODEX_HOME` is set)
3. `~/.codex/plugins` and `~/.codex/skills`

If duplicate IDs exist, earlier roots override later roots.

`list --source` supports:

- `all` (default)
- `project`
- `codex-home`
- `user-home`

## Expected File Shape

- Plugin directory:
  - `<plugin-id>/plugin.toml` (recommended)
- Skill directory:
  - `<skill-id>/SKILL.md` (required for discovery)

Minimal `plugin.toml` example:

```toml
[plugin]
id = "demo"
name = "Demo Plugin"
version = "0.1.0"
description = "Example plugin package."

[runtime]
run = "hooks/run.sh"
doctor = "hooks/doctor.sh"
timeout_ms = 15000
sandbox_profile = "standard" # restricted|standard|elevated
cpu_watchdog_ms = 3000      # optional wall-time watchdog budget override
max_output_bytes = 262144   # optional stdout/stderr cap per stream
max_cpu_ms = 2500           # optional per-hook CPU budget
max_rss_kb = 131072         # optional per-hook memory ceiling
```

## JSON Contracts

All commands support `--json`. Successful command envelope:

```json
{ "ok": true, "...": "..." }
```

`check` returns a report with per-extension checks and summary:

- `report.ok`
- `report.checked`
- `report.failed`
- `report.results[]`

`doctor` returns plugin runtime health summary:

- `doctor.plugins_total`
- `doctor.enabled_plugins`
- `doctor.disabled_plugins`
- `doctor.disabled_plugin_ids[]`
- `doctor.stale_disabled_ids[]`
- `doctor.runtime_missing_run_hooks[]`
- `doctor.runtime_missing_doctor_hooks[]`
- `doctor.runtime_runnable_plugins`

Missing target IDs return validation error (`exit_code=7`).

`plugins run` executes one hook (`run` or `doctor`) from:

1. manifest runtime path (`[runtime].run` / `[runtime].doctor`), or
2. fallback files under plugin package:
   - `hooks/run`
   - `hooks/run.sh`
   - `hooks/run.py`
   - `hooks/doctor`
   - `hooks/doctor.sh`
   - `hooks/doctor.py`

`plugins run` execution policy and telemetry:

- timeout resolution order: `--timeout-ms` > `[runtime].timeout_ms` > default `15000ms`
- output cap resolution order: `[runtime].max_output_bytes` > default `262144` (per stream)
- sandbox profile resolution order: `[runtime].sandbox_profile` > global `sandbox` policy profile
- approval resolution follows global `approvals` policy; in default `confirm` mode pass `--yes` for non-interactive execution
- optional per-hook resource ceilings: `[runtime].max_cpu_ms` and `[runtime].max_rss_kb`
  - unix runtime applies proactive `RLIMIT_CPU` when `max_cpu_ms` is configured
  - supported unix targets also apply proactive memory rlimits when `max_rss_kb` is configured at safe thresholds (>=16 MiB): `RLIMIT_AS` on linux/android, `RLIMIT_DATA` on BSD targets (`freebsd/dragonfly/netbsd/openbsd`)
  - other unix targets fall back to post-run memory checks only
  - `max_rss_kb` post-run enforcement requires unix-like resource metrics; non-unix targets reject `max_rss_kb` at validation time
- cpu watchdog fallback: runtime uses `[runtime].cpu_watchdog_ms` when configured; otherwise if `max_cpu_ms` is much lower than `timeout_ms`, runtime derives a stricter wall-time budget and terminates stalled hooks with a `resource watchdog exceeded` error
  - non-unix targets still support `max_cpu_ms` through the same watchdog path even when OS rlimits/metrics are unavailable
- restricted profile performs preflight checks on shell hook lines and blocks network/system commands
- each run appends event JSONL at `.mosaic/data/plugin-events/<plugin_id>.jsonl`
- JSON success payload includes:
  - `timeout_ms`
  - `output_limit_bytes`
  - `timed_out`
  - `sandbox_profile`
  - `resource_limits`
  - `resource_metrics` (`cpu_user_ms`, `cpu_system_ms`, `cpu_total_ms`, `max_rss_kb`)
  - `resource_rlimits_applied`
  - `stdout_bytes` / `stderr_bytes`
  - `stdout_truncated` / `stderr_truncated`
  - `event_log_path`
  - `command.rendered`

## Soak Script

For longer runtime stability checks around plugin resource policies:

```bash
cd cli
ITERATIONS=200 ./scripts/plugin_resource_soak.sh
```

Optional overrides:

- `MOSAIC_BIN=/path/to/mosaic` (skip auto-build path detection)
- `CPU_TIMEOUT_MS=5000`
- `KEEP_TMP=1` (preserve temporary workspace and event logs)

## Install/Remove Behavior

- `install` currently writes to project scope only (`.mosaic/plugins` and `.mosaic/skills`).
- `install` requires:
  - plugin source contains `plugin.toml`
  - skill source contains `SKILL.md`
- If target ID already exists, use `--force` to replace.
- `remove` only deletes project-scope entries and is a no-op for user/global sources.

## Plugin Enable/Disable State

- Plugin enable/disable state is persisted in `.mosaic/data/plugins-state.json`.
- Default behavior is enabled unless plugin ID is listed under `disabled_plugins`.
- `plugins install` auto-enables the installed plugin ID.
