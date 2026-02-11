# Security Audit (V3 bootstrap)

`mosaic security audit` performs local static checks for common security risks.

## Commands

```bash
mosaic --project-state security audit --path .
mosaic --project-state security audit --path . --deep
mosaic --project-state security audit --path . --update-baseline
mosaic --project-state security audit --path . --no-baseline
mosaic --project-state security audit --path . --sarif
mosaic --project-state security audit --path . --sarif-output scan.sarif

# Baseline management
mosaic --project-state security baseline show
mosaic --project-state security baseline add --fingerprint "<fp>"
mosaic --project-state security baseline add --category transport_security --match-path "vendor/*"
mosaic --project-state security baseline remove --fingerprint "<fp>"
mosaic --project-state security baseline clear
```

## Output

`--json` output shape:

- `report.summary`
  - `ok`
  - `findings`
  - `high`, `medium`, `low`
  - `ignored`
  - `scanned_files`, `skipped_files`
  - `generated_at`, `root`, `baseline_path`
- `report.findings[]`
  - `fingerprint`
  - `severity`, `category`, `title`, `detail`
  - `path`, `line`, `suggestion`
- `baseline`
  - `enabled`, `updated`, `added`, `path`
- `sarif_output` (when `--sarif-output` is used)

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

## Baseline

- Default baseline path:
  - project mode: `.mosaic/security/baseline.toml`
  - xdg mode: `<XDG config>/mosaic/security/baseline.toml`
- Use `--update-baseline` to add current findings fingerprints into the baseline.
- Use `--no-baseline` to ignore baseline filtering for one run.
- Use `security baseline show|add|remove|clear` for manual baseline management.
- Use `--sarif` to print SARIF v2.1.0 to stdout.
- Use `--sarif-output <path>` to persist SARIF v2.1.0 while keeping normal CLI output.
