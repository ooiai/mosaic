# Mosaic CLI Progress Log

Concise per-iteration delivery log for CLI work.

| UTC | Summary | Tests |
| --- | --- | --- |

| `2026-02-23T16:41:41Z` | Continue main.rs decomposition: extract devices/pairing handlers into dedicated module and add CLI progress log docs | cargo test -p mosaic-cli && cli/scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T164024Z.log) |
| `2026-02-23T16:44:36Z` | Continue main.rs decomposition: extract nodes handler into nodes_command.rs and remove stale policy imports | cargo test -p mosaic-cli |
| `2026-02-23T16:49:11Z` | Continue main.rs decomposition: extract gateway handler to gateway_command.rs; keep full regression green | cargo test -p mosaic-cli && cli/scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T164754Z.log) |
| `2026-02-23T16:55:03Z` | Continue main.rs decomposition: extract hooks/cron/webhooks into automation_commands.rs | cargo test -p mosaic-cli && cli/scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T165346Z.log) |
| `2026-02-23T16:59:30Z` | Continue main.rs decomposition: extract browser/memory/plugins/skills handlers into feature_commands.rs | cargo test -p mosaic-cli && cli/scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T165816Z.log) |
| `2026-02-23T17:07:59Z` | Continue main.rs decomposition: extract gateway lifecycle/runtime helpers into gateway_runtime.rs | cargo test -p mosaic-cli && cli/scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T170647Z.log) |
| `2026-02-23T17:23:10Z` | Refactor mosaic-cli entrypoint: extract runtime_context/core_commands/diagnostics_command modules and keep behavior unchanged | cargo check -p mosaic-cli; cargo test -p mosaic-cli; ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T172201Z.log) |
| `2026-02-23T17:27:45Z` | Stabilize quality gate: resolve clippy-deny warnings in core/channels/agents/plugins/security and keep regression green | cargo clippy -p mosaic-cli -- -D warnings; ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T172607Z.log) |
| `2026-02-23T17:28:36Z` | Add clippy gate to regression runbook and refresh regression catalog after quality hardening | ./scripts/update_regression_catalog.sh |
| `2026-02-23T17:29:14Z` | Enforce clippy quality gate in CI rust-cli-test job before full regression | ruby -e "require 'yaml'; YAML.load_file('.github/workflows/ci.yml')"; cargo clippy -p mosaic-cli -- -D warnings |
| `2026-02-23T23:09:29Z` | Continue main.rs decomposition: extract CLI schema/types into cli_schema.rs via include! while preserving module privacy semantics | cargo check -p mosaic-cli; cargo test -p mosaic-cli; cargo clippy -p mosaic-cli -- -D warnings; ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T230814Z.log) |
| `2026-02-23T23:12:07Z` | Add command_surface regression tests for root/channels/gateway help surfaces and keep full regression green | cargo test -p mosaic-cli --test command_surface; cargo test -p mosaic-cli; ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260223T231100Z.log) |
| `2026-02-26T08:23:21Z` | Expand command_surface coverage to hooks/cron/webhooks/agents and visible aliases; update runbook fast checks | cargo test -p mosaic-cli --test command_surface; cargo test -p mosaic-cli; cargo clippy -p mosaic-cli -- -D warnings; ./scripts/update_regression_catalog.sh |
| `2026-02-26T08:24:07Z` | Add root Makefile cli-quality target for fast local guardrail (check+clippy+command_surface+mosaic-cli tests) | make cli-quality |
| `2026-02-26T08:24:36Z` | Document new make cli-quality guardrail in EN/CN root README script list | make cli-quality |
| `2026-02-26T08:29:45Z` | Expand command_surface parity tests across nodes/devices/pairing/browser/memory/security/plugins/skills/logs/system/approvals/sandbox and keep cli-quality green | make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T08:31:17Z` | Full regression after command surface parity expansion | ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260226T083002Z.log) |
| `2026-02-26T08:33:11Z` | Add aliases_ops integration tests for onboard/message/agent alias runtime flows | cargo test -p mosaic-cli --test aliases_ops; make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T08:33:37Z` | Sync regression runbook fast-test section with new aliases_ops suite | ./scripts/update_regression_catalog.sh |
| `2026-02-26T08:39:09Z` | Expand help snapshots and error-code contract coverage | ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260226T083802Z.log) |
| `2026-02-26T08:39:36Z` | Add gateway/channels help snapshots and extend error-code contract tests for gateway_unavailable approval_required sandbox_denied | cargo test -p mosaic-cli --test help_snapshot --test error_codes; make cli-quality; ./scripts/run_regression_suite.sh |
| `2026-02-26T08:39:49Z` | Sync regression runbook fast-target list with error_codes suite | ./scripts/update_regression_catalog.sh |
| `2026-02-26T08:45:49Z` | Add json_contract schema snapshots for stable --json success/failure envelope regression | ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260226T084443Z.log) |
| `2026-02-26T08:51:36Z` | Add module-level json schema contracts for channels gateway security agents | ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260226T085027Z.log) |
| `2026-02-26T08:53:39Z` | Extend error code contracts for agents and security validation failures | ./scripts/run_regression_suite.sh (report: cli/reports/regression-20260226T085232Z.log) |
| `2026-02-26T08:54:16Z` | Document JSON contract test suites in cli README regression script section | docs-only |
| `2026-02-26T08:55:03Z` | Add make cli-json-contract target and document JSON contract gate in root README EN/CN and cli README | make cli-json-contract |
| `2026-02-26T08:55:41Z` | Add json-contracts guide documenting JSON envelope/schema suites and snapshot policy | ./scripts/update_regression_catalog.sh; make cli-json-contract |
| `2026-02-26T08:56:00Z` | Validate new cli-json-contract gate after JSON contract docs update | make cli-json-contract |
| `2026-02-26T09:12:25Z` | Add compatibility batch: aliases (`config/sessions/daemon/node/acp`), new `completion`, `directory`, `dashboard` commands, and create source-backed `planing.md` gap matrix | cargo test -p mosaic-cli; cargo test -p mosaic-cli --test help_snapshot --test command_surface --test compat_ops |
| `2026-02-26T09:12:25Z` | Refresh regression catalog after adding compat command tests | ./scripts/update_regression_catalog.sh |
| `2026-02-26T11:23:04Z` | Complete Phase B maintenance commands: add `update`, `reset`, `uninstall` with `--yes` safety gate, optional update source check, help snapshot + maintenance tests + error-code contracts | cargo test -p mosaic-cli; make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T11:28:11Z` | Start Phase C with runnable `docs` and `dns resolve` commands, plus command-surface/docs-dns/error-code tests and help snapshot refresh | cargo test -p mosaic-cli --test docs_dns_ops --test command_surface --test help_snapshot --test error_codes; make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T11:31:36Z` | Add `tui` compatibility shim command (routes to chat runtime) with command-surface and integration coverage | cargo test -p mosaic-cli --test tui_ops --test command_surface --test help_snapshot; make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T11:43:23Z` | Remove external legacy links from runtime/docs surfaces, switch docs index to local Mosaic docs, and add `qr` + `clawbot` commands with tests | cargo test -p mosaic-cli --test qr_clawbot_ops --test docs_dns_ops --test command_surface --test help_snapshot --test error_codes; make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T11:55:39Z` | Remove remaining legacy external references across root docs/security metadata, implement real QR render modes (`payload/ascii/png`) with PNG export, and expand `clawbot` with `send/status` plus validation and regression coverage | cargo test -p mosaic-cli --test qr_clawbot_ops --test command_surface --test error_codes; make cli-quality; ./scripts/update_regression_catalog.sh |
| `2026-02-26T12:01:37Z` | Extend `scripts/from_scratch_smoke.sh` to cover compatibility/maintenance commands (`docs/dns/tui/qr/clawbot/completion/directory/dashboard/update/reset/uninstall`), fix directory field assertion, and complete full smoke pass | SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh |
