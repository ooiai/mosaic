# Browser (CLI)

`browser` provides lightweight URL fetch and visit history for CLI workflows.

## Commands

```bash
mosaic --project-state browser start
mosaic --project-state browser stop
mosaic --project-state browser status
mosaic --project-state browser open --url <http|https|mock-url> [--timeout-ms <ms>]
mosaic --project-state browser navigate --url <http|https|mock-url> [--timeout-ms <ms>]
mosaic --project-state browser history [--tail <n>]
mosaic --project-state browser tabs [--tail <n>]
mosaic --project-state browser diagnose [--stale-after-minutes <minutes>] [--probe-url <url> ...] [--probe-timeout-ms <ms>] [--artifact-max-age-hours <hours>] [--repair]
mosaic --project-state browser show <visit-id>
mosaic --project-state browser focus <visit-id>
mosaic --project-state browser snapshot [visit-id]
mosaic --project-state browser screenshot [visit-id] [--out <path>]
mosaic --project-state browser close <visit-id>
mosaic --project-state browser close --all
mosaic --project-state browser clear <visit-id>
mosaic --project-state browser clear --all
```

## Behavior

- `start/stop/status` manage and inspect local browser runtime state (`running`, `active_visit_id`).
- `open` fetches URL content and records a visit entry.
- `navigate` is an explicit navigation command with the same fetch behavior as `open`.
- `tabs` returns the same visit list as `history` plus `active_visit_id`.
- `diagnose` checks runtime/history drift (`duplicate_visit_id`, invalid urls, stale visits, active pointer mismatch), classifies network failures (`network_failure_classes`), validates screenshot artifact integrity, and can run active probe drills with `--probe-url`.
- `diagnose --artifact-max-age-hours <hours>` evaluates screenshot retention against visit timestamps and reports stale artifacts.
- `diagnose --repair` applies safe repairs and persists changes, including cleanup of orphan/corrupt/stale screenshot artifacts.
- `focus` switches `active_visit_id` to a specific visit entry.
- `snapshot` resolves a visit (explicit id > active tab > latest visit) and returns concise page state.
- `screenshot` writes a deterministic text artifact for automation/testing workflows.
- `close` is a tab-oriented alias for visit removal (`close <visit-id>` or `close --all`).
- Supported schemes: `http`, `https`, `mock`.
- `mock://ok` is useful for offline/local testing.
- Non-2xx responses are recorded as `visit.ok=false` with `http_status`.

## Stored Data

- Visit history file: `.mosaic/data/browser-history.json`
- Runtime state file: `.mosaic/data/browser-state.json`
- Screenshot artifacts (default): `.mosaic/data/browser-screenshots/<visit-id>.txt`
- Entry fields include: `id`, `ts`, `url`, `ok`, `http_status`, `title`, `content_type`, `content_length`, `preview`, `error`.

## Example

```bash
mosaic --project-state --json browser start
mosaic --project-state --json browser open --url mock://ok?title=Docs
mosaic --project-state --json browser diagnose --stale-after-minutes 30 --probe-url mock://ok --probe-url mock://404
mosaic --project-state --json browser diagnose --stale-after-minutes 30 --artifact-max-age-hours 168 --repair
mosaic --project-state --json browser snapshot
mosaic --project-state --json browser screenshot
mosaic --project-state --json browser stop
```
