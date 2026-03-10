# Mosaic CLI Release Readiness (CLI-first)

Generated at: `2026-03-10T03:47:22+08:00`

## 1) Local Gate Results

### Passed

1. `cd cli && cargo test --workspace`
   - result: full workspace test matrix passed, including `tui_ops` and `tui_interactive`
2. `bash site/scripts/check_docs.sh --report-dir reports --report-prefix local-docs-check-release`
   - result: docs syntax/link checks passed
3. `bash -n cli/scripts/release_notes_from_worklog.sh`
   - result: script syntax valid (macOS Bash 3 compatible)
4. `cd cli && ./scripts/release_notes_from_worklog.sh --version v0.2.0-beta.6 --max-entries 8 --out /tmp/mosaic-release-notes-draft.md`
   - result: release notes draft generated successfully
5. `make -n cli-release-notes v=v0.2.0-beta.6`
   - result: root Makefile release-notes target is wired and callable
6. `cd cli && ./scripts/release_prepare.sh --version v0.2.0-beta.6 --skip-check --notes-max-entries 20 --notes-out docs/release-notes-v0.2.0-beta.6.md`
   - result: one-command local release prepare flow completed (notes + build + package)
7. `cd cli && ./scripts/release_verify_archives.sh --version v0.2.0-beta.6 --assets-dir <release-assets-dir>`
   - result: archive verifier behavior validated on both passing and failing fixtures (binary/layout checks)
8. `cd cli && ./scripts/release_verify_assets.sh --version v0.2.0-beta.6 --assets-dir <release-assets-dir>`
   - result: verifier behavior validated on both failing (missing matrix) and passing (complete matrix) asset sets
9. `cd cli && ./scripts/release_publish_check.sh --version v0.2.0-beta.6 --release-json-file <fixture>`
   - result: published-release validator behavior validated on both failing and passing release asset payload fixtures
10. `cd cli && ./scripts/release_tooling_smoke.sh --version v0.2.0-beta.6`
    - result: consolidated smoke passed (archive verifier pass/fail + asset verifier pass/fail + release_prepare dry-run summary write)
11. `cd cli && ./scripts/release_install_smoke.sh --version v0.2.0-beta.6`
    - result: installer smoke passed (local assets path + `--release-only` guardrail + `install.ps1` parse check when pwsh exists)
12. `make cli-quality`
    - result: check + clippy + command-surface + full `mosaic-cli` test matrix passed after clippy stabilization
13. `make cli-regression`
    - result: full regression suite passed, including release tooling smoke + release install smoke + from-scratch smoke (`cli/reports/regression-20260309T194250Z.log`)

## 2) Release Scope Completion (0/1)

| Area | State (0/1) | Notes |
| --- | --- | --- |
| Core CLI command surface | 1 | help/contract/error tests in workspace gate are green |
| TUI runtime replacement | 1 | real `mosaic tui` path + interaction/non-interaction split + tests |
| Channels/gateway/policy/runtime modules | 1 | integrated in workspace tests and docs |
| Distribution scripts/workflow | 1 | packaging + manifest generation + installer workflow present |
| Release checklist and runbook | 1 | checklist rewritten for CLI-only release flow |
| Release notes generation path | 1 | `cli/scripts/release_notes_from_worklog.sh` + make target available |
| One-command local release prepare | 1 | `cli/scripts/release_prepare.sh` + `make cli-release-prepare` available |
| Release tooling smoke | 1 | `cli/scripts/release_tooling_smoke.sh` + `make cli-release-tooling-smoke` available |
| Release install smoke | 1 | `cli/scripts/release_install_smoke.sh` + `make cli-release-install-smoke` available |
| Release archive verifier | 1 | `cli/scripts/release_verify_archives.sh` + `make cli-release-verify-archives` available |
| Release asset verifier | 1 | `cli/scripts/release_verify_assets.sh` + `make cli-release-verify` available |
| Published release verifier | 1 | `cli/scripts/release_publish_check.sh` + `make cli-release-publish-check` available |

## 3) Remaining Operational Items (0/1)

These are publish operations, not code gaps.

| Item | State (0/1) | Owner |
| --- | --- | --- |
| Final release notes editing (`cli/docs/release-notes-<version>.md`) | 0 | release operator |
| Tag and run release workflow (`.github/workflows/cli-release.yml`) | 0 | release operator |
| Cross-platform install smoke from release assets | 0 | release operator |

## 4) Current Conclusion

Engineering readiness for this phase is **complete at code/docs/test level**.

Final release completion requires executing the remaining publish operations above.
