# Mosaic CLI (Rust)

Mosaic CLI is the first-phase Rust implementation of a local agent workflow inspired by OpenClaw.
This workspace ships a pure CLI with no frontend dependency.

## Scope (V1 + V2)

- Local agent core (`ask`, `chat`, `session`, `models`, `status`, `health`, `doctor`)
- Gateway control plane (`gateway install|start|restart|status|health|probe|discover|call|stop|uninstall`)
- Channels runtime (`channels add|update|list|status|test|send|logs|capabilities|resolve|export|import|rotate-token-env|remove|logout`)
- Nodes/device pairing runtime (`nodes list|status|run|invoke`, `devices list|approve|reject|rotate|revoke`, `pairing list|request|approve`)
- Hooks runtime (`hooks list|add|remove|enable|disable|run|logs`, auto-trigger on `system event`)
- Cron runtime (`cron list|add|remove|enable|disable|run|tick|logs`)
- Webhooks runtime (`webhooks list|add|remove|enable|disable|trigger|resolve|logs`)
- Browser runtime (`browser open|history|show|clear`)
- Ops runtime (`logs`, `system`, `approvals`, `sandbox`)
- Memory runtime (`memory index|search|status`)
- Security runtime (`security audit`)
- Agents runtime (`agents list|add|update|show|remove|default|route`)
- Plugins and skills runtime (`plugins list|info|check|install|remove`, `skills list|info|check|install|remove`)
- OpenAI-compatible provider
- Tooling: `read_file`, `write_file`, `search_text`, `run_cmd`
- Command aliases compatible with OpenClaw-style naming:
  - `mosaic onboard` -> `mosaic setup`
  - `mosaic message` -> `mosaic ask`
  - `mosaic agent` -> `mosaic chat`

## Workspace Layout

```
cli/
  crates/
    mosaic-cli
    mosaic-core
    mosaic-agent
    mosaic-agents
    mosaic-channels
    mosaic-gateway
    mosaic-ops
    mosaic-memory
    mosaic-security
    mosaic-plugins
    mosaic-tools
    mosaic-provider-openai
```

## Quick Start

```bash
cd cli
cargo test --workspace
```

### Install Binary (`mosaic`)

```bash
cd cli
cargo install --path crates/mosaic-cli --force
```

If `mosaic` is still not found, add Cargo bin to PATH (zsh):

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Setup (Project State)

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state setup \
  --base-url https://api.openai.com \
  --model gpt-4o-mini
```

### List Models

```bash
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state models list

cargo run -p mosaic-cli --bin mosaic -- --project-state models status
cargo run -p mosaic-cli --bin mosaic -- --project-state models set gpt-4o-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state models aliases set fast gpt-4o-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state models aliases list
cargo run -p mosaic-cli --bin mosaic -- --project-state models fallbacks add gpt-4.1-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state models fallbacks list
```

### Ask

```bash
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state ask "summarize this repo"
```

### Chat REPL

```bash
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state chat
```

### Gateway Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway install --host 127.0.0.1 --port 8787
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway start
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway restart --port 8788
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway status --deep
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway health --verbose
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway probe
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway discover
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway call status
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway stop
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway uninstall
```

### Nodes/Devices/Pairing Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes list
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes status local
# run/invoke go through gateway; start gateway first
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway start
# default approvals policy is confirm, so run needs --yes (or allowlist policy)
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes nodes run local --command "echo hello"
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes invoke local status --params '{"detail":true}'

cargo run -p mosaic-cli --bin mosaic -- --project-state devices list
cargo run -p mosaic-cli --bin mosaic -- --project-state devices approve dev-1 --name "Jerry Mac"
cargo run -p mosaic-cli --bin mosaic -- --project-state devices rotate dev-1
cargo run -p mosaic-cli --bin mosaic -- --project-state devices revoke dev-1 --reason "device replaced"

cargo run -p mosaic-cli --bin mosaic -- --project-state pairing request --device dev-1 --node local --reason "new laptop"
cargo run -p mosaic-cli --bin mosaic -- --project-state pairing list --status pending
cargo run -p mosaic-cli --bin mosaic -- --project-state pairing approve <request-id>
```

### Hooks Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state hooks add \
  --name deploy-notify \
  --event deploy \
  --command "echo deploy-hook-fired"

# default approvals mode is confirm, so non-interactive runs usually need --yes
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes hooks run <hook-id> --data '{"source":"manual"}'

# system event auto-triggers enabled hooks that match event name
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes system event deploy --data '{"version":"1.0.0"}'

cargo run -p mosaic-cli --bin mosaic -- --project-state hooks logs --tail 20
```

### Cron Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state cron add \
  --name deploy-cron \
  --event deploy \
  --every 60 \
  --data '{"source":"cron"}'

# execute due jobs (job.next_run_at <= now)
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes cron tick

# run one job immediately
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes cron run <job-id>

cargo run -p mosaic-cli --bin mosaic -- --project-state cron logs --tail 20
```

### Webhooks Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state webhooks add \
  --name deploy-webhook \
  --event deploy \
  --path /inbound/deploy \
  --method post

# resolve route to webhook and dispatch event -> hooks pipeline
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes webhooks resolve \
  --path /inbound/deploy \
  --method post \
  --data '{"release":"2026.02"}'

# secret-protected webhook
export MOSAIC_WEBHOOK_SECRET="replace-me"
cargo run -p mosaic-cli --bin mosaic -- --project-state webhooks add \
  --name deploy-secret \
  --event deploy \
  --path /inbound/secure \
  --method post \
  --secret-env MOSAIC_WEBHOOK_SECRET
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes webhooks resolve \
  --path /inbound/secure \
  --method post \
  --secret "$MOSAIC_WEBHOOK_SECRET"
```

### Browser Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser open --url mock://ok?title=Docs
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser history --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser show <visit-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser clear <visit-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser clear --all
```

### Channels Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state channels add \
  --name team-alerts \
  --kind slack_webhook \
  --endpoint https://hooks.slack.com/services/T000/B000/XXXXX

cargo run -p mosaic-cli --bin mosaic -- --project-state channels add \
  --name local-terminal \
  --kind terminal

cargo run -p mosaic-cli --bin mosaic -- --project-state channels add \
  --name tg-alerts \
  --kind telegram_bot \
  --chat-id=-1001234567890 \
  --default-parse-mode markdown_v2 \
  --default-title "Release Notice" \
  --default-block "service=mosaic" \
  --default-metadata '{"env":"staging"}'

cargo run -p mosaic-cli --bin mosaic -- --project-state channels update <channel-id> \
  --name tg-alerts-prod \
  --chat-id=-1009876543210 \
  --default-title "Prod Notice"

cargo run -p mosaic-cli --bin mosaic -- --project-state channels update <channel-id> \
  --clear-defaults

export MOSAIC_TELEGRAM_BOT_TOKEN="<bot-token>"
cargo run -p mosaic-cli --bin mosaic -- --project-state channels send <channel-id> \
  --text "deploy complete" \
  --parse-mode markdown_v2 \
  --title "Release Notice" \
  --block "build=42" \
  --idempotency-key release-42

cargo run -p mosaic-cli --bin mosaic -- --project-state channels test <channel-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state channels send <channel-id> --text "hello"
cargo run -p mosaic-cli --bin mosaic -- --project-state channels list
cargo run -p mosaic-cli --bin mosaic -- --project-state channels status
cargo run -p mosaic-cli --bin mosaic -- --project-state channels logs --channel <channel-id> --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state channels capabilities --channel slack_webhook
cargo run -p mosaic-cli --bin mosaic -- --project-state channels resolve --channel slack_webhook alert
cargo run -p mosaic-cli --bin mosaic -- --project-state channels export --out .mosaic/channels-backup.json
cargo run -p mosaic-cli --bin mosaic -- --project-state channels import --file .mosaic/channels-backup.json
cargo run -p mosaic-cli --bin mosaic -- --project-state channels import --file .mosaic/channels-backup.json --strict
cargo run -p mosaic-cli --bin mosaic -- --project-state channels import --file .mosaic/channels-backup.json --report-out .mosaic/import-report.json
cargo run -p mosaic-cli --bin mosaic -- --project-state channels import --file .mosaic/channels-backup.json --replace --dry-run
cargo run -p mosaic-cli --bin mosaic -- --project-state channels import --file .mosaic/channels-backup.json --replace
cargo run -p mosaic-cli --bin mosaic -- --project-state channels rotate-token-env --channel <channel-id> --to MOSAIC_TELEGRAM_BOT_TOKEN_V2 --dry-run
cargo run -p mosaic-cli --bin mosaic -- --project-state channels rotate-token-env --all --kind telegram_bot --to MOSAIC_TELEGRAM_BOT_TOKEN_V2
cargo run -p mosaic-cli --bin mosaic -- --project-state channels rotate-token-env --all --kind telegram_bot --from MOSAIC_TELEGRAM_BOT_TOKEN --to MOSAIC_TELEGRAM_BOT_TOKEN_V2
cargo run -p mosaic-cli --bin mosaic -- --project-state channels rotate-token-env --all --kind telegram_bot --from MOSAIC_TELEGRAM_BOT_TOKEN --to MOSAIC_TELEGRAM_BOT_TOKEN_V2 --report-out .mosaic/rotation-report.json
```

Detailed guide: `docs/channels-slack.md`
Discord webhook guide: `docs/channels-discord.md`
Terminal channel guide: `docs/channels-terminal.md`
Telegram channel guide: `docs/channels-telegram.md`
Gateway ops guide: `docs/gateway-ops.md`
Nodes/devices/pairing guide: `docs/nodes-devices-pairing.md`
Ops guide (logs/system): `docs/ops.md`
Hooks guide: `docs/hooks.md`
Cron guide: `docs/cron.md`
Webhooks guide: `docs/webhooks.md`
Browser guide: `docs/browser.md`
Approvals and sandbox guide: `docs/sandbox-approvals.md`
Memory guide: `docs/memory.md`
Security audit guide: `docs/security-audit.md`
Agents guide: `docs/agents.md`
Plugins and skills guide: `docs/plugins-skills.md`
Azure OpenAI provider guide: `docs/provider-azure-openai.md`
OpenClaw parity map: `docs/openclaw-parity.md`
Regression catalog (all docs + all test cases): `docs/regression-catalog.md`
Regression runbook: `docs/regression-runbook.md`
Work logs: `../WORKLOG.md` (release timeline) and `docs/progress.md` (concise per-iteration record)

Telegram default token env: `MOSAIC_TELEGRAM_BOT_TOKEN`.
Telegram min send interval env: `MOSAIC_CHANNELS_TELEGRAM_MIN_INTERVAL_MS` (default `800`).
Idempotency dedupe window env: `MOSAIC_CHANNELS_IDEMPOTENCY_WINDOW_SECONDS` (default `86400`).
Telegram 429 fallback retry env: `MOSAIC_CHANNELS_TELEGRAM_RETRY_AFTER_DEFAULT_SECONDS` (default `1`).

### Regression Scripts

```bash
cd cli
./scripts/update_regression_catalog.sh
./scripts/run_regression_suite.sh
./scripts/run_regression_suite.sh --worklog-summary "Nightly full regression"
SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh
./scripts/worklog_append.sh --summary "Summary of change" --tests "cargo test --workspace"
```

### Ops Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state logs --tail 100
cargo run -p mosaic-cli --bin mosaic -- --project-state system event deployment --data '{"env":"staging"}'
cargo run -p mosaic-cli --bin mosaic -- --project-state system presence
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals get
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals set allowlist
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals allowlist add "cargo test"
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox list
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox explain --profile restricted
```

### Memory Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state memory index --path .
cargo run -p mosaic-cli --bin mosaic -- --project-state memory search "rust cli"
cargo run -p mosaic-cli --bin mosaic -- --project-state memory status
```

### Security Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path .
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --deep
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --update-baseline
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --no-baseline
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --sarif
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --sarif-output scan.sarif
cargo run -p mosaic-cli --bin mosaic -- --project-state security baseline show
cargo run -p mosaic-cli --bin mosaic -- --project-state security baseline add --fingerprint "<fp>"
cargo run -p mosaic-cli --bin mosaic -- --project-state security baseline clear
```

### Agents Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state agents list
cargo run -p mosaic-cli --bin mosaic -- --project-state agents add --name Writer --id writer --set-default --route ask
cargo run -p mosaic-cli --bin mosaic -- --project-state agents update writer --name "Writer V2" --route chat
cargo run -p mosaic-cli --bin mosaic -- --project-state agents show writer
cargo run -p mosaic-cli --bin mosaic -- --project-state agents route list
cargo run -p mosaic-cli --bin mosaic -- --project-state ask --agent writer "hello"
```

### Plugins and Skills Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins list
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins info <plugin-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins check
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins install --path ./my-plugin
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins remove <plugin-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state skills list
cargo run -p mosaic-cli --bin mosaic -- --project-state skills info <skill-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state skills check
cargo run -p mosaic-cli --bin mosaic -- --project-state skills install --path ./writer
cargo run -p mosaic-cli --bin mosaic -- --project-state skills remove <skill-id>
```

## Optional Live Smoke Test

By default tests run with mock coverage only.  
To run live provider smoke tests:

```bash
cd cli
LIVE=1 OPENAI_API_KEY=... cargo test -p mosaic-provider-openai -- --nocapture
```

Optional env:

- `OPENAI_BASE_URL` (default: `https://api.openai.com`)
- `OPENAI_MODEL` (default: `gpt-4o-mini`)
