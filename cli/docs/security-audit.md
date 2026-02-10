# Security Audit (V3 bootstrap)

`mosaic security audit` performs local static checks for common security risks.

## Commands

```bash
mosaic --project-state security audit --path .
mosaic --project-state security audit --path . --deep
```

## Output

`--json` output shape:

- `report.summary`
  - `ok`
  - `findings`
  - `high`, `medium`, `low`
  - `scanned_files`, `skipped_files`
  - `generated_at`, `root`
- `report.findings[]`
  - `severity`, `category`, `title`, `detail`
  - `path`, `line`, `suggestion`

## Current checks

- Private key marker detection (`BEGIN PRIVATE KEY`)
- Potential hardcoded secrets (`api_key`, `token`, `secret`, `password` literals)
- AWS access key style pattern (`AKIA...`)
- `curl ... | sh/bash` patterns
- Plain `http://` endpoint detection
- Wildcard CORS header (`Access-Control-Allow-Origin: *`)
- `eval()` usage detection

## Limits

- `--max-files` (default `800`)
- `--max-file-size` (default `262144` bytes)
- Skips common folders: `.git`, `target`, `node_modules`, `.pnpm-store`, `.mosaic`
