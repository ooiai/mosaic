# Security Audit (V3 bootstrap)

`mosaic security audit` performs local static checks for common security risks.

Runtime data-protection guardrails for agent tools are documented in:

- `docs/sandbox-approvals.md` (`Private Data Guard` section)

That section also documents pre-persistence protection for:

- log/event/history JSONL files
- canonical state/config files (`config.toml`, `models.toml`, approvals/sandbox policy, agent routes, MCP server registry, channels/gateway/browser/voicecall state, memory cleanup policy/status, security baseline)

## Commands

```bash
mosaic --project-state security audit --path .
mosaic --project-state security audit --path . --deep
mosaic --project-state security audit --path . --update-baseline
mosaic --project-state security audit --path . --no-baseline
mosaic --project-state security audit --path . --min-severity medium
mosaic --project-state security audit --path . --category supply_chain --category cors --top 20
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
- `report.risk`
  - `score` (`0-100`)
  - `level` (`low|moderate|high|critical`)
  - `recommendations[]`
- `filters`
  - `min_severity`, `categories`, `top`, `filtered_out`
- `dimensions`
  - `categories` (count by category)
  - `severities.high|medium|low`
- `baseline`
  - `enabled`, `updated`, `added`, `path`
- `sarif_output` (when `--sarif-output` is used)

## Current checks

- Private key marker detection (`BEGIN PRIVATE KEY`)
- Potential hardcoded secrets (`api_key`, `token`, `secret`, `password` literals)
- Secret persistence in canonical state/config files (`state_persistence`)
- AWS access key style pattern (`AKIA...`)
- `curl ... | sh/bash` patterns
- Plain `http://` endpoint detection
- TLS verification disabled patterns (`InsecureSkipVerify=true`, `rejectUnauthorized=false`, etc.)
- Wildcard CORS header (`Access-Control-Allow-Origin: *`)
- Weak hash usage (`md5(...)`, `sha1(...)`)
- Default credential literals (`changeme`, `default`, `123456`, etc.)
- `eval()` usage detection

## Limits

- `--max-files` (default `800`)
- `--max-file-size` (default `262144` bytes)
- `--min-severity <low|medium|high>` to keep only findings at/above a severity threshold
- `--category <name>` (repeatable) to keep only selected categories
- `--top <n>` to cap returned findings after sorting by severity/path/line
- Generic source scan skips common folders: `.git`, `target`, `node_modules`, `.pnpm-store`, `.mosaic`
- Selected canonical state/config files inside `.mosaic` are still inspected for secret persistence risk
- Text output also prints risk score/level and recommended remediation actions.

## Baseline

- Default baseline path:
  - project mode: `.mosaic/security/baseline.toml`
  - xdg mode: `<XDG config>/mosaic/security/baseline.toml`
- Use `--update-baseline` to add current findings fingerprints into the baseline.
- Use `--no-baseline` to ignore baseline filtering for one run.
- Use `security baseline show|add|remove|clear` for manual baseline management.
- Use `--sarif` to print SARIF v2.1.0 to stdout.
- Use `--sarif-output <path>` to persist SARIF v2.1.0 while keeping normal CLI output.

## Persistence Guardrails

For state/config persistence, Mosaic now uses a stricter rule than log redaction:

- event/log/history writes: redact secret-like values, block private keys
- state/config writes: reject the write if secret-like literal values are about to be stored

This avoids silently mutating canonical state while still preventing accidental token/password persistence.
