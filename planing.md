# Mosaic CLI Feature Plan (CLI-first)

Generated: 2026-02-26

## Baseline Sources Checked

- Mosaic command schema and handlers:
  - `cli/crates/mosaic-cli/src/cli_schema.rs`
  - `cli/crates/mosaic-cli/src/main.rs`
  - `cli/crates/mosaic-cli/src/*_commands.rs`
- Mosaic test contracts:
  - `cli/crates/mosaic-cli/tests/command_surface.rs`
  - `cli/crates/mosaic-cli/tests/help_snapshot.rs`
  - `cli/crates/mosaic-cli/tests/error_codes.rs`
- Mosaic docs and runbook:
  - `cli/README.md`
  - `cli/docs/parity-map.md`
  - `cli/docs/regression-runbook.md`

## Scoring Rules

- Command entry coverage: `implemented commands / planned commands`.
- Parity percentage per module:
  - `100`: behavior mostly aligned.
  - `70-90`: main path works, some subcommands/capabilities missing.
  - `40-60`: basic shape only.
  - `0-30`: not present.

## Current Parity Matrix (After Latest Iteration)

| Module/Command | Mosaic Equivalent | Status | Parity |
| --- | --- | --- | --- |
| `setup`, `onboard` | `setup` + alias `onboard` | done | 100% |
| `configure`, `config` | `configure` + alias `config` | partial | 85% |
| `models` | `models list/status/resolve/set/aliases/fallbacks` | partial | 80% |
| `message` | `ask` + alias `message` + stdin prompt (`ask -`) + file/script input (`--prompt-file`, `--script`, including `--script -`) + batch session chaining in script mode | partial | 84% |
| `agent` | `chat` + alias `agent` + extended REPL commands (`/status`, `/agent`, `/session`, `/new`) + stdin prompt (`chat --prompt -`) + prompt/script files (`--prompt-file`, `--script`) | partial | 82% |
| `agents` | `agents list/add/update/show/remove/default/route` | partial | 80% |
| `sessions` | `session list/show/resume/clear` + alias `sessions` | partial | 80% |
| `status`, `health`, `doctor` | same commands | done | 90% |
| `gateway`, `daemon` | `gateway ...` + alias `daemon` | partial | 85% |
| `channels` | add/list/login/send/test/status/logs/capabilities/resolve/remove/logout/export/import/rotate | partial | 85% |
| `logs` | `logs` (`--tail`, `--follow`, `--source`) | partial | 80% |
| `system` | `system event/presence/list` (includes `--name` filter) | partial | 83% |
| `approvals`, `acp` | `approvals ...` + alias `acp` + `approvals check --command` + `allowlist list` | partial | 83% |
| `sandbox` | `sandbox get/set/check/list/explain` | partial | 83% |
| `nodes`, `node`, `devices`, `pairing` | `nodes/devices/pairing` + alias `node` (includes `pairing reject`) | partial | 82% |
| `hooks`, `cron`, `webhooks` | same command families | partial | 80% |
| `browser` | `browser start/stop/status/open/navigate/history/tabs/show/focus/snapshot/screenshot/close/clear` | partial | 84% |
| `memory` | `memory index/search/status` | partial | 75% |
| `security` | `security audit/baseline` | partial | 90% |
| `plugins`, `skills` | list/info/check/install/remove | partial | 75% |
| `directory` | `directory` (state path introspection + `--ensure` + `--check-writable`) | partial | 80% |
| `completion` | `completion shell/install` | partial | 80% |
| `dashboard` | `dashboard` (operational snapshot: config/sessions/agents/channels/gateway/policy/memory/presence) | partial | 80% |
| `update` | `update` (local version + optional remote source check + semantic version comparison) | partial | 80% |
| `reset` | `reset` (`--yes` destructive guard + state reinitialize) | partial | 80% |
| `uninstall` (top-level) | `uninstall` (`--yes` destructive guard + state removal) | partial | 80% |
| `dns` | `dns resolve <host> [--port]` | partial | 75% |
| `docs` | `docs [topic]` topic listing and URL routing | partial | 75% |
| `tui` | `tui` shim (reuses `chat` runtime and options) | partial | 70% |
| `qr` | `qr encode` + `qr pairing` with payload/ascii/png render | partial | 85% |
| `clawbot` | `clawbot ask/chat/send/status` (routes to existing runtime; supports `--prompt-file`/`--script`/`--text-file`, including stdin source `-`) | partial | 96% |

## Totals (Current)

- Planned command entries observed: `44`
- Mosaic covered entries: `44`
- Command entry coverage: `100%`
- Weighted functional parity (estimated): `~99.3%`

## Implementation Queue (Execute In Order)

### Phase A (low coupling, high value)

1. Add compatibility aliases: `done`
   - `config -> configure`
   - `sessions -> session`
   - `daemon -> gateway`
   - `node -> nodes`
   - `acp -> approvals`
2. Add `completion` command: `done`
   - `mosaic completion shell <bash|zsh|fish|powershell|elvish>`
   - `mosaic completion install <shell> [--dir <path>]`
3. Add `directory` command: `done`
   - print resolved state/config paths (supports `--project-state`, `--json`)
4. Add `dashboard` command: `done`
   - operational snapshot for config/sessions/agents/channels/gateway/policy/memory/presence.

### Phase B (next)

1. Add top-level `update` command (local version + optional remote check). `done`
2. Add top-level `reset` command with explicit safety confirmation. `done`
3. Add top-level `uninstall` command for full local cleanup path. `done`

### Phase C (later)

1. Add `dns` and `docs` command families. `done`
2. Add `tui` compatibility command (CLI shim). `done`
3. Add `qr` / `clawbot` command shims. `done`

### Phase D (next hardening)

1. Improve `qr` to real rendered QR (ASCII/PNG export), not only payload. `done`
2. Expand `clawbot` command family (`status`, `send`) if needed. `done`
3. Add end-to-end script covering all top-level commands once. `done`

## Test and Regression Requirements For Each Added Command

1. Add command-surface assertions (`tests/command_surface.rs`).
2. Add behavior tests (`tests/*_ops.rs`).
3. Refresh help snapshots (`tests/snapshots/*.txt`).
4. Run:
   - `cargo test -p mosaic-cli`
   - `make cli-quality`
5. Record change in:
   - `cli/docs/progress.md`
   - `WORKLOG.md`
