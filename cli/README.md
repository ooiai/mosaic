# Mosaic CLI (Rust)

Mosaic CLI is the first-phase Rust implementation of a local agent workflow inspired by OpenClaw.
This workspace ships a pure CLI with no frontend dependency.

## Scope (V1 + V2)

- Local agent core (`ask`, `chat`, `session`, `models`, `status`, `health`, `doctor`)
- Gateway control plane (`gateway run|status|health|probe|discover|call|stop`)
- Channels runtime (`channels add|update|list|status|test|send|logs|capabilities|resolve|export|import|remove|logout`)
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
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway run --host 127.0.0.1 --port 8787
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway status
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway health
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway probe
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway discover
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway call status
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway stop
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
cargo run -p mosaic-cli --bin mosaic -- --project-state channels import --file .mosaic/channels-backup.json --replace
```

Detailed guide: `docs/channels-slack.md`
Discord webhook guide: `docs/channels-discord.md`
Terminal channel guide: `docs/channels-terminal.md`
Telegram channel guide: `docs/channels-telegram.md`
Gateway ops guide: `docs/gateway-ops.md`
Approvals and sandbox guide: `docs/sandbox-approvals.md`
Memory guide: `docs/memory.md`
Security audit guide: `docs/security-audit.md`
Agents guide: `docs/agents.md`
Plugins and skills guide: `docs/plugins-skills.md`

Telegram default token env: `MOSAIC_TELEGRAM_BOT_TOKEN`.
Telegram min send interval env: `MOSAIC_CHANNELS_TELEGRAM_MIN_INTERVAL_MS` (default `800`).
Idempotency dedupe window env: `MOSAIC_CHANNELS_IDEMPOTENCY_WINDOW_SECONDS` (default `86400`).
Telegram 429 fallback retry env: `MOSAIC_CHANNELS_TELEGRAM_RETRY_AFTER_DEFAULT_SECONDS` (default `1`).

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
