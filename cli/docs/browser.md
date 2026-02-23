# Browser (CLI)

`browser` provides lightweight URL fetch and visit history for CLI workflows.

## Commands

```bash
mosaic --project-state browser open --url <http|https|mock-url> [--timeout-ms <ms>]
mosaic --project-state browser history [--tail <n>]
mosaic --project-state browser show <visit-id>
mosaic --project-state browser clear <visit-id>
mosaic --project-state browser clear --all
```

## Behavior

- `open` fetches URL content and records a visit entry.
- Supported schemes: `http`, `https`, `mock`.
- `mock://ok` is useful for offline/local testing.
- Non-2xx responses are recorded as `visit.ok=false` with `http_status`.

## Stored Data

- Visit history file: `.mosaic/data/browser-history.json`
- Entry fields include: `id`, `ts`, `url`, `ok`, `http_status`, `title`, `content_type`, `content_length`, `preview`, `error`.

## Example

```bash
mosaic --project-state --json browser open --url mock://ok?title=Docs
mosaic --project-state --json browser history --tail 5
```
