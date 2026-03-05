# Mosaic CLI Release Readiness (CLI-first)

Generated at: `2026-03-05T07:47:30Z`

## 1) Local Gate Results

### Passed

1. `make docs-check`
2. `cargo test -p mosaic-cli --test command_surface`  
   - result: `39 passed; 0 failed`
3. `./scripts/tutorial_regression.sh --profile tutorial-smoke`  
   - result: completed, report generated
4. `node --check site/assets/docs.js`
5. `site/scripts/check_docs.sh --report-dir reports --report-prefix local-docs-check`

### Key Reports

1. `cli/reports/tutorial-regression-latest.log`
2. `reports/local-docs-check-summary.json`
3. `reports/local-docs-check-syntax.log`
4. `reports/local-docs-check-links.log`

## 2) Release Scope Completion (0/1)

| Area | State (0/1) | Notes |
| --- | --- | --- |
| Core CLI command surface | 1 | `command_surface` test passed |
| Config/profile/models chain | 1 | setup/configure/models docs + commands stable |
| Session lifecycle | 1 | `session list/show/resume/clear` documented and wired |
| Channels runtime | 1 | add/test/send/logs/capabilities/resolve documented |
| Gateway lifecycle + call API | 1 | install/start/probe/call/health documented |
| Approvals/sandbox/safety | 1 | policy tutorial and command checks available |
| Memory/security/plugins/skills/agents | 1 | module docs and command help coverage complete |
| Docs site navigation/search | 1 | new modules indexed in `docs.js` and linked |
| Docs static gate in CI | 1 | `docs-site-check` job + artifacts |
| Docs gate in pages deploy | 1 | pre-deploy check + artifacts |
| Azure one-command ops script | 1 | `azure_ops_playbook.sh` added |
| Azure script machine summary | 1 | `--json-summary` + `--summary-out` |
| Cross-platform packaging scripts | 1 | existing release scripts/workflows present |

## 3) Remaining Items (0/1)

These are release operations, not code gaps.

| Item | State (0/1) | Owner |
| --- | --- | --- |
| Live Azure playbook run with real credentials | 0 | user runtime env |
| Release tag + publish workflow execution | 0 | release operator |

## 4) Completion Time

Engineering completion for this phase is **done** at `2026-03-05` (local code/documentation/CI gate level).

Final external completion = after the two operational items above are executed.
