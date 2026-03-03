# Mosaic CLI (Rust)

Mosaic CLI is the first-phase Rust implementation of a local agent workflow.
This workspace ships a pure CLI with no frontend dependency.

## Scope (V1 + V2)

- Local agent core (`ask`, `chat`, `session`, `models`, `dashboard`, `status`, `health`, `doctor`)
- Gateway control plane (`gateway install|start|restart|status|health|probe|discover|call|stop|uninstall`)
- MCP runtime (`mcp list|add|check|enable|disable|remove`)
- Channels runtime (`channels add|update|list|status|test|send|logs|capabilities|resolve|export|import|rotate-token-env|remove|logout`)
- Nodes/device pairing runtime (`nodes list|status|run|invoke`, `devices list|approve|reject|rotate|revoke`, `pairing list|request|approve|reject`)
- Hooks runtime (`hooks list|add|remove|enable|disable|run|logs`, auto-trigger on `system event`)
- Cron runtime (`cron list|add|remove|enable|disable|run|tick|logs`)
- Webhooks runtime (`webhooks list|add|remove|enable|disable|trigger|resolve|logs`)
- Realtime compatibility runtime (`tts voices|speak`, `voicecall start|status|send|history|stop`)
- Browser runtime (`browser start|stop|status|open|navigate|history|tabs|show|focus|snapshot|screenshot|close|clear`)
- Ops runtime (`logs`, `observability`, `system`, `approvals`, `sandbox`, `safety`)
- CLI compatibility/runtime helpers (`completion shell|install`, `directory`)
- Maintenance runtime (`update`, `reset`, `uninstall`)
- Discovery/runtime helpers (`docs`, `dns resolve`)
- UX compatibility shim (`tui` -> chat runtime)
- Compatibility helpers (`qr encode|pairing` with payload/ascii/png render, `clawbot ask|chat|send|status`)
- Memory runtime (`memory index|search|status|clear`)
- Security runtime (`security audit`)
- Agents runtime (`agents list|add|update|show|remove|default|route`)
- Plugins and skills runtime (`plugins list|info|check|install|enable|disable|doctor|run|remove`, `skills list|info|check|install|remove`)
- OpenAI-compatible provider
- Tooling: `read_file`, `write_file`, `search_text`, `run_cmd`
- Command aliases for legacy naming compatibility:
  - `mosaic onboard` -> `mosaic setup`
  - `mosaic config` -> `mosaic configure`
  - `mosaic message` -> `mosaic ask`
  - `mosaic agent` -> `mosaic chat`
  - `mosaic sessions` -> `mosaic session`
  - `mosaic daemon` -> `mosaic gateway`
  - `mosaic node` -> `mosaic nodes`
  - `mosaic acp` -> `mosaic approvals`

## Workspace Layout

```
cli/
  crates/
    mosaic-cli
    mosaic-core
    mosaic-agent
    mosaic-agents
    mosaic-mcp
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

From GitHub release assets:

```bash
# macOS (Homebrew formula from release asset)
brew install https://github.com/ooiai/mosaic/releases/latest/download/mosaic.rb

# Linux/macOS installer script
curl -fsSL https://github.com/ooiai/mosaic/releases/latest/download/install.sh | bash

# Windows (PowerShell)
# irm https://github.com/ooiai/mosaic/releases/latest/download/install.ps1 | iex
```

From local source:

```bash
cd cli
cargo install --path crates/mosaic-cli --force
```

If `mosaic` is still not found, add Cargo bin to PATH (zsh):

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Completion / Directory / Dashboard

```bash
cargo run -p mosaic-cli --bin mosaic -- completion shell zsh
cargo run -p mosaic-cli --bin mosaic -- completion install zsh

cargo run -p mosaic-cli --bin mosaic -- --project-state directory
cargo run -p mosaic-cli --bin mosaic -- --project-state directory --ensure --check-writable
cargo run -p mosaic-cli --bin mosaic -- --project-state dashboard
cargo run -p mosaic-cli --bin mosaic -- --project-state --json dashboard
```

`dashboard` now provides an operational snapshot (config, sessions, agents, channels, gateway, policy, memory, presence) and keeps compatibility keys like `configured/profile/latest_session`.

### Maintenance Commands

```bash
# show current version
cargo run -p mosaic-cli --bin mosaic -- update

# optional remote check (supports JSON with latest/version/tag_name or plain text)
cargo run -p mosaic-cli --bin mosaic -- update --check --source mock://v0.2.0

# semantic compare: older/same versions report update_available=false
cargo run -p mosaic-cli --bin mosaic -- --json update --check --source mock://0.0.0

# destructive operations require --yes
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes reset
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes uninstall
```

### Docs / DNS

```bash
cargo run -p mosaic-cli --bin mosaic -- docs
cargo run -p mosaic-cli --bin mosaic -- docs gateway

cargo run -p mosaic-cli --bin mosaic -- --json dns resolve localhost --port 443
```

### TUI Shim

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state tui --prompt "hello"
```

### QR / Clawbot

```bash
cargo run -p mosaic-cli --bin mosaic -- --json qr encode "hello world"
cargo run -p mosaic-cli --bin mosaic -- --json qr encode "hello world" --render ascii
cargo run -p mosaic-cli --bin mosaic -- --json qr encode "hello world" --render png --output .mosaic/qr/hello.png --module-size 6
cargo run -p mosaic-cli --bin mosaic -- --json qr pairing --device dev-1 --node local --ttl-seconds 300

cargo run -p mosaic-cli --bin mosaic -- --project-state clawbot ask "hello"
cargo run -p mosaic-cli --bin mosaic -- --project-state clawbot ask --prompt-file prompts/clawbot-ask.txt
cargo run -p mosaic-cli --bin mosaic -- --project-state --json clawbot ask --script prompts/clawbot-ask-script.txt
cargo run -p mosaic-cli --bin mosaic -- --project-state --json clawbot chat --script prompts/clawbot-chat-script.txt
cargo run -p mosaic-cli --bin mosaic -- --project-state clawbot send "ship it"
cargo run -p mosaic-cli --bin mosaic -- --project-state clawbot send --text-file prompts/clawbot-send.txt
printf "ship it stdin\n" | cargo run -p mosaic-cli --bin mosaic -- --project-state clawbot send --text-file -
cargo run -p mosaic-cli --bin mosaic -- --project-state --json clawbot status
```

### Setup (Project State)

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state setup \
  --base-url https://api.openai.com \
  --model gpt-4o-mini
```

### Configure (Profile Keys)

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state configure --show
cargo run -p mosaic-cli --bin mosaic -- --project-state configure keys
cargo run -p mosaic-cli --bin mosaic -- --project-state configure get provider.base_url
cargo run -p mosaic-cli --bin mosaic -- --project-state configure set tools.enabled false
cargo run -p mosaic-cli --bin mosaic -- --project-state configure unset tools.enabled
cargo run -p mosaic-cli --bin mosaic -- --project-state configure patch --set provider.model=gpt-4.1-mini --dry-run
cargo run -p mosaic-cli --bin mosaic -- --project-state configure patch --set provider.model=gpt-4.1-mini --set agent.max_turns=12
cargo run -p mosaic-cli --bin mosaic -- --project-state configure patch --file config-patch.json
cargo run -p mosaic-cli --bin mosaic -- --project-state configure preview --target-profile migration --set provider.model=gpt-4.1-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state configure patch --target-profile migration --set provider.model=gpt-4.1-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state --json configure template --target-profile migration --format json
cargo run -p mosaic-cli --bin mosaic -- --project-state --json configure template --target-profile migration --format toml --defaults
```

`configure patch/preview --json` includes per-key `updates`, grouped `groups` summaries (`provider/agent/tools`), and `target_profile` metadata for profile-aware migration previews.

### List Models

```bash
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state models list

OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state --json models list --query gpt --limit 5

cargo run -p mosaic-cli --bin mosaic -- --project-state models status
cargo run -p mosaic-cli --bin mosaic -- --project-state models resolve
cargo run -p mosaic-cli --bin mosaic -- --project-state models resolve fast
cargo run -p mosaic-cli --bin mosaic -- --project-state models set gpt-4o-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state models aliases set fast gpt-4o-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state models aliases list
cargo run -p mosaic-cli --bin mosaic -- --project-state models fallbacks add gpt-4.1-mini
cargo run -p mosaic-cli --bin mosaic -- --project-state models fallbacks list
```

`models list --json` now includes `query`, `limit`, `total_models`, `matched_models`, and `returned_models`.

### Ask

```bash
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state ask "summarize this repo"

# read prompt from stdin
printf "summarize this repo\n" | OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state ask -

# read prompt from file
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state ask --prompt-file prompts/ask.txt

# run line-based ask script (one non-empty line = one ask turn)
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state --json ask --script prompts/ask-script.txt

# run ask script from stdin
printf "first ask\nsecond ask\n" | OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state --json ask --script -
```

### Chat REPL

```bash
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state chat

# single non-interactive chat from file
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state --json chat --prompt-file prompts/chat.txt

# run line-based chat script (one non-empty line = one turn)
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state --json chat --script prompts/chat-script.txt
```

REPL commands:

- `/help`: show command help
- `/status`: show active profile, agent, and session
- `/agent`: show active agent
- `/session`: show current session id
- `/new`: reset chat and start a new session
- `/exit`: quit chat

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

### TTS / Voicecall Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state --json tts voices
cargo run -p mosaic-cli --bin mosaic -- --project-state --json tts speak --text "hello" --voice alloy --format wav --out .mosaic/tts/hello.wav

cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall start --target ops-room
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall send --text "deployment started"
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall history --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall stop
```

### MCP Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp list
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp add --name local-mcp --command /usr/bin/env --arg bash
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp check <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp disable <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp enable <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp remove <server-id>
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
cargo run -p mosaic-cli --bin mosaic -- --project-state pairing reject <request-id> --reason "policy denied"
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
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser start
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser navigate --url mock://ok?title=Docs
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser status
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser tabs --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser focus <visit-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser snapshot
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser screenshot
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser history --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser show <visit-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser close <visit-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser close --all
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser clear <visit-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser clear --all
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser stop
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
Distribution guide (brew/linux/windows): `docs/distribution.md`
Coverage map: `docs/parity-map.md`
JSON contracts guide: `docs/json-contracts.md`
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
cargo test -p mosaic-cli --test error_codes
cargo test -p mosaic-cli --test json_contract
cargo test -p mosaic-cli --test json_contract_modules
SKIP_WORKSPACE_TESTS=1 ./scripts/from_scratch_smoke.sh
ITERATIONS=200 ./scripts/plugin_resource_soak.sh
./scripts/worklog_append.sh --summary "Summary of change" --tests "cargo test --workspace"

# package one platform release asset
./scripts/package_release_asset.sh --version v0.2.0-beta.2 --target aarch64-apple-darwin

# generate brew/scoop manifests from collected assets
./scripts/update_distribution_manifests.sh --version v0.2.0-beta.2 --assets-dir ./dist/v0.2.0-beta.2 --output-dir ./dist/v0.2.0-beta.2
```

### Ops Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state logs --tail 100
cargo run -p mosaic-cli --bin mosaic -- --project-state --json logs --tail 100 --source system
cargo run -p mosaic-cli --bin mosaic -- --project-state --json logs --tail 100 --source plugin:sample_plugin
cargo run -p mosaic-cli --bin mosaic -- --project-state --json observability report --tail 100 --event-tail 50 --audit-tail 100 --compare-window 100
cargo run -p mosaic-cli --bin mosaic -- --project-state --json observability report --tail 100 --event-tail 50 --audit-tail 100 --compare-window 100 --plugin-soak-report ./reports/plugin-soak-latest.log --no-doctor
cargo run -p mosaic-cli --bin mosaic -- --project-state --json observability export --out .mosaic/reports/observability.json --tail 100 --event-tail 50 --audit-tail 100 --compare-window 100 --no-doctor
cargo run -p mosaic-cli --bin mosaic -- --project-state system event deployment --data '{"env":"staging"}'
cargo run -p mosaic-cli --bin mosaic -- --project-state system presence
cargo run -p mosaic-cli --bin mosaic -- --project-state system list --tail 50
cargo run -p mosaic-cli --bin mosaic -- --project-state system list --tail 50 --name deployment
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals get
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals check --command "cargo test --workspace"
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals set allowlist
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals allowlist add "cargo test"
cargo run -p mosaic-cli --bin mosaic -- --project-state approvals allowlist list
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox get
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox set restricted
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox check --command "curl https://example.com"
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox list
cargo run -p mosaic-cli --bin mosaic -- --project-state sandbox explain --profile restricted
cargo run -p mosaic-cli --bin mosaic -- --project-state safety get
cargo run -p mosaic-cli --bin mosaic -- --project-state safety check --command "cargo test --workspace"
cargo run -p mosaic-cli --bin mosaic -- --project-state safety report --command "curl https://example.com" --audit-tail 100 --compare-window 100
```

`observability report/export` appends plugin soak samples to `.mosaic/data/reports/plugin-soak-history.jsonl` (or XDG equivalent) when `plugin_soak.available=true`, supports retention (`MOSAIC_OBS_PLUGIN_SOAK_HISTORY_MAX_SAMPLES`), exposes history/delta summary fields, includes gateway + channels telemetry slices, and adds alert suppression/SLO controls (`MOSAIC_OBS_ALERT_*`, `MOSAIC_OBS_SLO_*`).

### Memory Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state memory index --path .
cargo run -p mosaic-cli --bin mosaic -- --project-state memory search "rust cli"
cargo run -p mosaic-cli --bin mosaic -- --project-state memory status
cargo run -p mosaic-cli --bin mosaic -- --project-state memory clear
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
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins list --source project
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins info <plugin-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins check
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins install --path ./my-plugin
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins enable <plugin-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins disable <plugin-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins doctor
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes plugins run <plugin-id> --hook run --arg smoke --timeout-ms 10000
cargo run -p mosaic-cli --bin mosaic -- --project-state plugins remove <plugin-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state skills list
cargo run -p mosaic-cli --bin mosaic -- --project-state skills list --source project
cargo run -p mosaic-cli --bin mosaic -- --project-state skills info <skill-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state skills check
cargo run -p mosaic-cli --bin mosaic -- --project-state skills install --path ./writer
cargo run -p mosaic-cli --bin mosaic -- --project-state skills remove <skill-id>
```

Plugin runtime manifests can optionally enforce per-hook output and resource budgets with `[runtime].max_output_bytes`, `[runtime].max_cpu_ms`, and `[runtime].max_rss_kb`; `plugins run --json` emits `output_limit_bytes`, truncation flags, and matching `resource_limits`/`resource_metrics`.

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
