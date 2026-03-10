# Mosaic CLI Feature Plan (CLI-first)

Generated: 2026-02-26

## Release Gate Snapshot (2026-03-05)

### Done (Code/Docs/CI)

1. CLI command surface and module parity baseline complete.
2. Full docs system extended:
   - module pages (`agents/channels/memory/plugins/skills/gateway/gateway-call/models-profiles/sessions/policy/regression`)
   - Azure end-to-end playbook (EN/CN)
3. Docs acceptance gate added in CI and Pages deploy:
   - `site/scripts/check_docs.sh`
   - `.github/workflows/ci.yml` (`docs-site-check`)
   - `.github/workflows/deploy.yml` (pre-deploy docs gate)
4. Azure operations automation added:
   - `cli/scripts/azure_ops_playbook.sh`
   - supports `--json-summary` and `--summary-out`
5. MCP beta freeze gate added and passing:
   - `cli/scripts/mcp_freeze_check.sh`
   - latest report: `cli/reports/mcp-freeze-latest.log`

### Remaining 0/1 Items (Release Operations)

| Item | State (0/1) |
| --- | --- |
| Live Azure run with real credentials | 0 |
| Tag and publish release workflow | 0 |

Engineering scope is complete; remaining items are release execution steps.

## Module Completion Plan Refresh (2026-03-05)

The command surface is complete, but depth/operational capability is still uneven across modules.  
This section tracks **functional depth completion** (not only command existence).

### Current Remaining Gaps (Depth View)

| Module | Depth | Main Missing Pieces |
| --- | --- | --- |
| `mcp` | 100% | no open blocking gap in current beta scope |
| `gateway` | 97% | deeper auto-remediation workflows around runtime/state reconciliation |
| `channels` | 95% | richer multi-channel recovery ergonomics and operator guardrails |
| `memory` | 100% | no open blocking gap in current beta scope |
| `knowledge` | 99% | source adapter depth (batch/http auth/mcp metadata) and long-horizon lifecycle automation polish |
| `nodes/devices/pairing` | 88% | broaden lifecycle diagnostics/recovery automation and richer operational telemetry exports |
| `hooks/cron/webhooks` | 94% | further enrich long-horizon replay analytics (trend scoring + backlog-age prioritization guidance) |
| `tts/voicecall` | 90% | deeper provider/runtime drill coverage beyond mock synth probe and broader external integration depth |
| `browser` | 92% | richer protocol-aware probe workflows (multi-target scheduling + report export) and retention policy automation tuning |
| `distribution` | 90% | clean-VM install verification matrix and stable external tap/bucket publish flow |

### Execution Plan (Complete In Batches)

1. Batch A (done in this iteration)
   - `mcp show <server_id>`
   - `mcp check --all` + default batch check (`mcp check`)
   - batch health summary envelope (`checked/healthy/unhealthy/results`)
   - tests/contracts/docs updated
2. Batch B (done)
   - gateway/channels diagnostics hardening:
     - gateway call/probe error taxonomy tightening
     - channels delivery replay insights and richer logs summary (`channels logs --summary`, `channels replay`)
3. Batch C (done)
   - memory/security operational hardening:
     - incremental memory indexing
     - namespace lifecycle controls (`status --all-namespaces`, `prune --max-namespaces/--max-age-hours`)
     - namespace document quota pruning (`prune --max-documents-per-namespace`) + prune reason breakdown
     - persistent cleanup policy (`memory policy get/set/apply` with interval guard)
     - security audit report dimensions and tuning (`--min-severity/--category/--top`)
4. Batch D
   - realtime + distribution stabilization:
     - tts/voicecall provider-depth checks (`tts diagnose` delivered; next add non-mock/provider adapters)
     - clean-VM install matrix (local installer smoke + release verification automation delivered)

### Acceptance Rule For Each Batch

1. module tests pass (`*_ops`, json contracts, command surface)
2. docs and runbook updated
3. `--json` envelope contracts stable
4. progress recorded in `WORKLOG.md` and `cli/docs/progress.md`

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
| `configure`, `config` | `configure` + alias `config` + `configure keys/get/set/unset/patch/preview/template` (`patch --target-profile`, `preview` dry-run, `template` JSON/TOML, grouped diff summaries by provider/agent/tools) | done | 100% |
| `models` | `models list/status/resolve/set/aliases/fallbacks` (`list` includes `--query/--limit`) | partial | 82% |
| `message` | `ask` + alias `message` + stdin prompt (`ask -`) + file/script input (`--prompt-file`, `--script`, including `--script -`) + batch session chaining in script mode | partial | 84% |
| `agent` | `chat` + alias `agent` + extended REPL commands (`/status`, `/agent`, `/session`, `/new`) + stdin prompt (`chat --prompt -`) + prompt/script files (`--prompt-file`, `--script`) | partial | 82% |
| `agents` | `agents list/add/update/show/remove/default/route` | partial | 80% |
| `sessions` | `session list/show/resume/clear` + alias `sessions` | partial | 80% |
| `status`, `health`, `doctor` | same commands | done | 90% |
| `gateway`, `daemon` | `gateway ...` + alias `daemon` + protocol health checks (`gateway_discover`/`gateway_discover_schema_profile`/`gateway_protocol_methods`/`gateway_call_status`/`gateway_call_health`/`gateway_call_nodes_run`/`gateway_call_nodes_invoke`) + method-level schema checks in health (`gateway_status_schema_profile`/`gateway_health_schema_profile`/`gateway_nodes_run_schema_profile`/`gateway_nodes_invoke_schema_profile`) + optional health auto-remediation (`gateway health --repair` + `gateway_auto_repair`, including service-target reconciliation before restart) + stricter discover/call response validation + persisted request telemetry (`.mosaic/data/gateway-events.jsonl`) + persisted gateway degradation history (`gateway.history`) surfaced in observability (`gateway.telemetry.*`, `gateway.recent_events`, `gateway.request_failures`/`gateway.failure_rate_regression` alerts) | partial | 97% |
| `mcp` | `mcp list/add/show/update/check/diagnose/repair/enable/disable/remove` + local registry + readiness checks (`check --all` batch summary) + deep batch protocol probes (`check --all --deep --timeout-ms --report-out`) + single-target protocol initialize/session probe (`diagnose --timeout-ms --report-out`) with framed-response conformance (`Content-Length` stdio parsing plus newline-JSON mock fallback) and post-initialize session readiness (`notifications/initialized`, `session_ready`) + explicit config mutation path (`update` with replace/clear semantics for `args/env/env_from/cwd` and enable-state toggles) + auto-remediation workflow (`repair [<server_id>|--all]` with optional `--clear-missing-cwd` + `--set-env-from KEY=ENV_NAME` and before/after deltas) + doctor/observability MCP telemetry integration + persisted MCP observability history (`mcp.history`) with delta/regression alerts and incident hints + secret-safe env indirection (`--env-from KEY=ENV_NAME`) with missing-env diagnostics + dedicated freeze gate (`scripts/mcp_freeze_check.sh`) + from-scratch smoke coverage for `session_ready` | done | 100% |
| `channels` | add/list/login/send/test/status/logs/replay/capabilities/resolve/remove/logout/export/import/rotate (`capabilities --target` includes runtime diagnostics; `replay` supports retry-focused planning with time/reason/status/attempt filters `--since-minutes/--reason/--http-status/--min-attempt`, batch planning via `--batch-size` + `batch_plan`, optional `--apply` full-payload replay with readiness preflight guard, legacy preview fallback handling, strict mode `--require-full-payload`, batch apply controls `--max-apply/--stop-on-error`, and report export `--report-out`) | partial | 95% |
| `logs` | `logs` (`--tail`, `--follow`, `--source`) | partial | 80% |
| `observability` | `observability report/export` (logs + system + doctor + policy + safety audit aggregate, supports `--audit-tail` + `--compare-window` + optional `--plugin-soak-report`, with plugin soak history persistence + retention + `current_vs_previous` deltas + gateway/channels telemetry slices + alert rollups + suppression controls + SLO view + persisted SLO history + unmet-streak/incident hints) | done | 100% |
| `system` | `system event/presence/list` (includes `--name` filter) | partial | 83% |
| `approvals`, `acp` | `approvals ...` + alias `acp` + `approvals check --command` + `allowlist list` | partial | 83% |
| `sandbox` | `sandbox get/set/check/list/explain` | partial | 83% |
| `safety` | `safety get/check/report` + merged sandbox/approvals decision surface + audit summary/diff (`--audit-tail`, `--compare-window`) | partial | 91% |
| `nodes`, `node`, `devices`, `pairing` | `nodes/devices/pairing` + alias `node` (includes `pairing reject` + `nodes diagnose [node-id] --stale-after-minutes --repair` for drift detection/remediation) | partial | 88% |
| `hooks`, `cron`, `webhooks` | same command families (`logs` support `--summary` + `--since-minutes`; `replay` supports failed-event planning + optional `--apply` with `--stop-on-error`, reason filters `--reason`, retryable-only filters, batch planning `--batch-size` + `batch_plan`, partial apply capping `--max-apply`, recovery diagnostics (`recovery_diagnostics`), report export `--report-out`, and webhook secret-env reuse) | partial | 94% |
| `tts` | `tts voices/speak/diagnose` (`diagnose` adds synth probe with timeout guard, optional output write check, and report export) | partial | 84% |
| `voicecall` | `voicecall start/status/send/history/stop` (supports channel-bound delivery path with `send --parse-mode/--token-env` overrides, reusing channels retry/telemetry and observability realtime/alert telemetry) | partial | 86% |
| `browser` | `browser start/stop/status/open/navigate/history/tabs/diagnose(showing probe + retention diagnostics)/show/focus/snapshot/screenshot/close/clear` | partial | 92% |
| `memory` | `memory index/search/status/clear/prune/policy` (`index` supports `--namespace`, `--incremental`, `--stale-after-hours`, `--retain-missing`, and reuse/reindex/remove counters; `status --all-namespaces`; `prune --max-namespaces/--max-age-hours/--max-documents-per-namespace` with reason breakdown fields; `policy get/set/apply` persists cleanup policy + interval guard; built-in `mosaic.memory.cleanup` / `memory.cleanup` event integration for cron/system/webhook runtime) | partial | 100% |
| `knowledge` | `knowledge ingest/search/ask/evaluate/datasets` (`ingest` supports `--source local_md|http|mcp` with markdown chunk staging/reuse/stale cleanup and unified `--report-out`; `search/ask` support `--min-score` filtering; `ask` supports `--references-only`; `evaluate` supports query batches + report export + baseline update/regression gating + history trend output; `datasets list/remove` supports lifecycle audit + dry-run/cleanup`) | partial | 99% |
| `security` | `security audit/baseline` (`audit` supports `--min-severity`, repeatable `--category`, and `--top` with dimensions summary; includes TLS/weak-hash/default-secret checks + risk score/level/recommendations) + runtime private-data guardrails in tools (`sensitive path blocking`, `sensitive command guard`, `secret output redaction`) + persistence guardrails across the log plane (`session/audit/system/channel/gateway/realtime/hooks/webhooks/cron/plugins/observability/knowledge-eval` pre-write redaction + private-key redline blocking) + state/config persistence guardrails (`config/models/policies/agents/mcp/channels/gateway/browser/voicecall/memory/security baseline` reject secret-like literal writes) | partial | 100% |
| `plugins`, `skills` | `plugins`: list (`--source`)/info/check/install/enable/disable/doctor/run/remove (`run` includes timeout/output-guard/resource-limits/sandbox/approval/event+metrics telemetry + unix CPU RLIMIT pre-enforcement + configurable cpu watchdog (`cpu_watchdog_ms`) including non-unix CPU-only fallback + supported-unix memory pre-enforcement (`RLIMIT_AS` on linux/android, `RLIMIT_DATA` on BSD) for safe thresholds + non-unix `max_rss_kb` guard + plugin soak long-horizon anomaly hints in observability); `skills`: list (`--source`)/info/check/install/remove | done | 100% |
| `directory` | `directory` (state path introspection + `--ensure` + `--check-writable`) | partial | 80% |
| `completion` | `completion shell/install` | partial | 80% |
| `dashboard` | `dashboard` (operational snapshot: config/sessions/agents/channels/gateway/policy/memory/presence) | partial | 80% |
| `update` | `update` (local version + optional remote source check + semantic version comparison) | partial | 80% |
| `reset` | `reset` (`--yes` destructive guard + state reinitialize) | partial | 80% |
| `uninstall` (top-level) | `uninstall` (`--yes` destructive guard + state removal) | partial | 80% |
| `dns` | `dns resolve <host> [--port]` | partial | 75% |
| `docs` | `docs [topic]` topic listing and URL routing | partial | 75% |
| `tui` | `tui` chat-first terminal UI (`mosaic-tui` + `ratatui/crossterm`) with sessions/messages/inspector panes, focus control (`--focus`), inspector toggle (`--no-inspector`), interactive keymap, and non-interactive `--prompt` JSON mode compatibility | partial | 90% |
| `qr` | `qr encode` + `qr pairing` with payload/ascii/png render | partial | 85% |
| `clawbot` | `clawbot ask/chat/send/status` (routes to existing runtime; supports `--prompt-file`/`--script`/`--text-file`, including stdin source `-`) | partial | 96% |
| `distribution` | cross-platform release packaging (`linux/mac/windows`) + installers (`install.sh`/`install.ps1`) + Homebrew/Scoop manifest generation + archive/assets verifiers + local installer smoke (`release_install_smoke.sh`) | partial | 90% |

## Totals (Current)

- Planned command entries observed: `50`
- Mosaic covered entries: `50`
- Command entry coverage: `100%`
- Weighted functional parity (estimated): `~100%`
- Beta freeze gate: `PASS` (`cli/scripts/beta_release_check.sh`, report: `cli/reports/beta-readiness-latest.log`)

## Beta Freeze (Scope Complete)

- Frozen scope: all command families listed in the parity matrix (including `mcp`, `tts`, `voicecall`).
- Completion rule: command surface + JSON/error contracts + smoke coverage + beta gate pass.
- Freeze result: `100%` for current CLI beta scope.
- Remaining work is optimization/backlog only (not release blocking): richer gateway/channels diagnostics, security rulepack expansion, optional post-beta MCP ergonomics.

## Module Gap Audit (Against Upstream `src`, 2026-03-01)

Upstream `src` modules observed:
`agents, approvals, browser, channels, cli, commands, config, cron, devices, error, gateway, health, hooks, logs, mcp, memory, models, nodes, observability, pairing, plugins, provider, safety, sandbox, security, sessions, skills, status, system, tools, tts, update, voicecall, webhooks`.

### High-Priority Gaps

No open high-priority gaps in current matrix.

### Not Implemented Yet (Major)

No unresolved missing major module gaps in current CLI matrix.

### Medium Gaps (Quality/Optimization)

| Area | Current | Optimization Target |
| --- | --- | --- |
| gateway | lifecycle/call/probe/discover + protocol checks in `gateway health --verbose` + discovery/status/health/nodes schema checks + optional auto-remediation (`gateway health --repair`, including service-target reconciliation before restart) + request telemetry stream (`gateway-events.jsonl`) + persisted long-window gateway degradation analytics (`gateway.history`) integrated into observability alerts | deeper runtime/state reconciliation workflows |
| channels | webhook/bot path complete for current kinds + target-level runtime capability diagnostics (`ready_for_send`, token/env + endpoint/target checks) + replay planning/execution (`channels replay`, `--since-minutes`, `--reason`, `--http-status`, `--min-attempt`, `--batch-size`, `--apply` full-payload + readiness preflight guard + legacy fallback + strict flag `--require-full-payload` + `--max-apply` + `--stop-on-error` + `--report-out`) | broader failure-recovery ergonomics and operator guardrails |
| mcp | local registry + `check` preflight + deep batch probes (`check --all --deep --report-out`) + `diagnose` stdio initialize/session probe (`--timeout-ms --report-out`) with framed-response parsing (`Content-Length` + newline-JSON fallback) and `session_ready` validation after `notifications/initialized` + explicit config mutation via `update` (replace/clear semantics for `args/env/env_from/cwd` + enable-state toggles) + `repair` auto-remediation pass (`enable disabled` + optional missing-cwd cleanup + env-source rewiring via `--set-env-from`) + doctor/observability MCP telemetry + persisted long-window MCP degradation analytics/alerts (`mcp.history`, ratio delta + incident hints) + secret-safe env indirection (`--env-from`) with source-env validation + dedicated freeze gate and from-scratch smoke proof | post-beta optimization only: optional richer runtime drills and broader remediation ergonomics |
| memory | index/search/status/clear/prune/policy + namespace lifecycle + incremental/refresh + tuned relevance scoring + document quota pruning (`--max-documents-per-namespace`) + persisted cleanup policy with interval guard + built-in auto-run event integration (`mosaic.memory.cleanup`) | optional daemon templates + richer cleanup telemetry summaries |
| knowledge | ingest/search/ask/evaluate/datasets over `local_md/http/mcp` with staged markdown chunks + unified ingest report export + retrieval references + score threshold filtering (`--min-score`) + references-only mode + query-batch evaluation export + baseline regression gating + persistent history trends + dataset lifecycle list/remove | source metadata depth and long-horizon lifecycle automation polish |
| security | audit/baseline + audit filter dimensions + risk score/level/recommendations + extra TLS/crypto/credential checks + runtime private-data guardrails in tool execution + log-plane persistence pre-write redaction/redline blocking + state/config persistence validation across canonical stores | deeper rulepacks and category-specific checks |
| distribution | release artifacts + installer scripts + generated manifests + release/archive/install smoke verification | run periodic install verification on clean VMs and publish stable brew tap/scoop bucket |

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
