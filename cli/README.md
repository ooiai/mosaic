# Mosaic CLI (Rust)

Mosaic CLI is the first-phase Rust implementation of a local agent workflow.
This workspace ships a pure CLI with no frontend dependency.

## Scope (V1 + V2)

- Local agent core (`ask`, `chat`, `session`, `models`, `dashboard`, `status`, `health`, `doctor`)
- Gateway control plane (`gateway install|start|restart|status|health|probe|discover|diagnose|call|stop|uninstall`)
- MCP runtime (`mcp list|add|show|update|check|diagnose|repair|enable|disable|remove`, supports `check --all`)
- Channels runtime (`channels add|update|list|status|test|send|logs|replay|capabilities|resolve|export|import|rotate-token-env|remove|logout`)
- Nodes/device pairing runtime (`nodes list|status|diagnose|run|invoke`, `devices list|approve|reject|rotate|revoke`, `pairing list|request|approve|reject`)
- Hooks runtime (`hooks list|add|remove|enable|disable|run|logs|replay`, auto-trigger on `system event`)
- Cron runtime (`cron list|add|remove|enable|disable|run|tick|logs|replay`)
- Webhooks runtime (`webhooks list|add|remove|enable|disable|trigger|resolve|logs|replay`)
- Realtime compatibility runtime (`tts voices|speak|diagnose`, `voicecall start|status|send|history|stop`)
- Browser runtime (`browser start|stop|status|open|navigate|history|tabs|diagnose|show|focus|snapshot|screenshot|close|clear`)
- Ops runtime (`logs`, `observability`, `system`, `approvals`, `sandbox`, `safety`)
- CLI compatibility/runtime helpers (`completion shell|install`, `directory`)
- Maintenance runtime (`update`, `reset`, `uninstall`)
- Discovery/runtime helpers (`docs`, `dns resolve`)
- Chat-first terminal UI runtime (`tui`)
- Compatibility helpers (`qr encode|pairing` with payload/ascii/png render, `clawbot ask|chat|send|status`)
- Memory runtime (`memory index|search|status|clear`)
- Knowledge runtime (`knowledge ingest|search|ask|evaluate|datasets list|datasets remove`)
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
    mosaic-tui
    mosaic-tools
    mosaic-provider-openai
```

## Quick Start

```bash
cd cli
cargo test --workspace
```

### Regression Scripts

```bash
# full regression catalog + workspace tests + from-scratch smoke
./scripts/run_regression_suite.sh

# MCP-only beta freeze gate
./scripts/mcp_freeze_check.sh

# beginner-friendly end-to-end tutorial regression (offline by default)
./scripts/tutorial_regression.sh

# optional live provider smoke
LIVE=1 BASE_URL=https://api.openai.com MODEL=gpt-4o-mini ./scripts/tutorial_regression.sh

# azure one-command operations playbook (profile + channels + gateway + policy + optional regression)
AZURE_OPENAI_BASE_URL=https://<resource>.openai.azure.com/openai/v1 \
AZURE_OPENAI_API_KEY=<key> \
./scripts/azure_ops_playbook.sh --profile az-openai --run-regression

# emit machine-readable summary json
AZURE_OPENAI_BASE_URL=https://<resource>.openai.azure.com/openai/v1 \
AZURE_OPENAI_API_KEY=<key> \
./scripts/azure_ops_playbook.sh --profile az-openai --json-summary --summary-out reports/azure-playbook-summary.json
```

### Install Binary (`mosaic`)

From source (no release dependency):

```bash
# macOS (Homebrew tap + formula in repo)
brew tap ooiai/mosaic https://github.com/ooiai/mosaic
brew install mosaic

# Linux/macOS installer script
curl -fsSL https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.sh | bash -s -- --from-source

# Windows (PowerShell)
irm https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.ps1 -OutFile install.ps1
powershell -ExecutionPolicy Bypass -File .\install.ps1 -FromSource
```

From local workspace source:

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

### TUI

```bash
# interactive fullscreen TUI (default when no subcommand is provided)
cargo run -p mosaic-cli --bin mosaic -- --project-state

# explicit TUI entry still works
cargo run -p mosaic-cli --bin mosaic -- --project-state tui

# customize initial focus and inspector visibility
cargo run -p mosaic-cli --bin mosaic -- --project-state tui --focus sessions --no-inspector

# non-interactive one-shot (JSON contract compatible with previous tui path)
cargo run -p mosaic-cli --bin mosaic -- --project-state --json tui --prompt "hello"
```

TUI shortcuts:

- `Enter`: send
- `Ctrl+J`: newline
- `Tab`: cycle focus
- `Ctrl+N`: new session
- `Ctrl+R`: refresh sessions
- `Ctrl+I`: toggle inspector
- `Ctrl+A`: open the agent picker
- `Ctrl+S`: open the session picker
- selecting a session from the left pane: reloads that session, shows its bound profile/agent in the list, and rebinds the active runtime to that session's bound agent
- `/agent <id>` in the input box: switch active agent; if the current TUI chat already has a session, Mosaic starts a new session before switching
- `/help`, `/agents`, `/agent`, `/session`, `/clear`, and `/status` are first-class local TUI slash commands
- `/models`, `/skills`, `/docs`, `/logs`, and `/doctor` now run as local TUI discovery/inspection commands and render results directly into the transcript
- `/agent ...` and `/session ...` now autocomplete against configured agents and recent sessions before falling back to generic command rows
- the command assistant labels suggestions by source (`local`, `agent`, `session`, `shell`) and shows follow-up hints for the selected row
- `/memory`, `/knowledge`, and `/plugins` are still surfaced as shell-first command hints in the command sheet
- running turns now show animated waiting copy plus a bottom activity rail for recent tool/runtime events
- while a turn is in flight, the conversation pane also shows an active `Mosaic` turn placeholder so progress remains visible inside the transcript
- `/agents` in the input box: open the same agent picker without leaving the keyboard flow
- `/session <id>` in the input box: resume a session directly by id
- `/new` in the input box: start a fresh session without leaving the current runtime
- `/status` in the input box: print the active runtime summary into the status line
- `?`: help overlay
- `q` / `Ctrl+C`: quit

The bottom status line now always includes the active detail plus `profile / agent / session / policy`, so runtime context stays visible while you move between sessions and agents. `Ctrl+A` opens an in-TUI agent picker for direct keyboard selection when you do not want to type `/agent <id>`, and `Ctrl+S` does the same for session resume when the left pane is not the active focus.
`agents list` now also marks default/route bindings directly in its output, and the TUI agent picker mirrors that metadata so you can see which agents back `ask/chat` routes before switching.

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

# resume an existing session; if --agent is omitted, Mosaic reuses the last agent bound to that session
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state --json ask --session <session-id> "continue this thread"
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

# resume an existing chat session with its last bound agent
OPENAI_API_KEY=... cargo run -p mosaic-cli --bin mosaic -- \
  --project-state chat --session <session-id>
```

REPL commands:

- `/help`: show command help
- `/status`: show active profile, agent, and session
- `/agent`: show active agent
- `/agent <id>`: switch active agent; if the current chat already has a session, Mosaic starts a new session before switching
- `/session`: show current session id
- `/new`: reset chat and start a new session
- `/exit`: quit chat

`session list --json` now includes per-session `runtime` summaries, and `session show --json` includes a `runtime` object with the last persisted `profile_name` and `agent_id`.

### Gateway Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway install --host 127.0.0.1 --port 8787
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway start
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway restart --port 8788
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway status --deep
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway health --verbose
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway health --verbose --repair
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway probe
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway discover
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway diagnose --method status
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway call status
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway stop
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway uninstall
```

`gateway health --verbose` now validates discovery shape plus `status`, `health`, `nodes.run`, and `nodes.invoke` schema profiles, and `--repair` can reconcile missing service metadata before falling back to restart behavior.

### TTS / Voicecall Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state --json tts voices
cargo run -p mosaic-cli --bin mosaic -- --project-state --json tts speak --text "hello" --voice alloy --format wav --out .mosaic/tts/hello.wav
cargo run -p mosaic-cli --bin mosaic -- --project-state --json tts diagnose --voice alloy --format txt --text "probe" --timeout-ms 2000 --report-out .mosaic/reports/tts-diagnose.json

# local-only voicecall
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall start --target ops-room
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall send --text "deployment started"
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall history --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall stop

# channel-routed voicecall (reuses channels transport/retry/telemetry)
cargo run -p mosaic-cli --bin mosaic -- --project-state --json channels add --name voice-terminal --kind terminal
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall start --target ops-room --channel-id <channel-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall send --text "deployment started"
# for channel kinds that require auth/format overrides:
cargo run -p mosaic-cli --bin mosaic -- --project-state --json voicecall send --text "markdown alert" --parse-mode markdown --token-env MOSAIC_TELEGRAM_BOT_TOKEN
```

### MCP Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp list
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp add --name local-mcp --command /usr/bin/env --arg bash --env MCP_MODE=local --env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp show <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp update <server-id> --env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY --clear-cwd
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp update <server-id> --clear-args --disable
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp check <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp check --all
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp check --all --deep --timeout-ms 2000
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp check --all --deep --timeout-ms 2000 --report-out .mosaic/reports/mcp-check-deep.json
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp diagnose <server-id> --timeout-ms 2000
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp diagnose <server-id> --timeout-ms 2000 --report-out .mosaic/reports/mcp-diagnose.json
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp repair <server-id> --timeout-ms 2000
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp repair <server-id> --timeout-ms 2000 --set-env-from OPENAI_API_KEY=AZURE_OPENAI_API_KEY
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp repair --all --timeout-ms 2000 --clear-missing-cwd --report-out .mosaic/reports/mcp-repair.json
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp disable <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp enable <server-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state mcp remove <server-id>
```

`--env` persists non-sensitive runtime pairs. `--env-from KEY=ENV_NAME` persists only the env variable reference and resolves the real value from Mosaic's process environment during MCP checks and launches.

Use `mcp update` for deliberate config mutations and `mcp repair` for diagnose-driven remediation. `update` replaces the full `args`/`env`/`env_from` collection you pass and supports `--clear-args`, `--clear-env`, `--clear-env-from`, and `--clear-cwd`.

`mcp diagnose` now accepts standard `Content-Length` framed MCP stdio responses and the lightweight newline-JSON mocks used in local regression tests. Deep probe health now means `initialize` succeeded and Mosaic could send `notifications/initialized` on the same session.

### Nodes/Devices/Pairing Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes list
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes status local
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes diagnose local --stale-after-minutes 30
cargo run -p mosaic-cli --bin mosaic -- --project-state nodes diagnose local --stale-after-minutes 30 --repair
cargo run -p mosaic-cli --bin mosaic -- --project-state --json nodes diagnose local --stale-after-minutes 30 --repair --report-out .mosaic/reports/nodes-diagnose.json
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

`nodes diagnose --report-out` writes the full remediation report to disk, and node/device/pairing lifecycle actions append normalized telemetry to `.mosaic/data/nodes-events.jsonl`, which is aggregated into `observability report/export`.

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
cargo run -p mosaic-cli --bin mosaic -- --project-state hooks logs --tail 20 --summary --since-minutes 60
cargo run -p mosaic-cli --bin mosaic -- --project-state hooks replay --tail 50 --limit 10 --batch-size 5 --reason tool --retryable-only --report-out hooks-replay-plan.json
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes hooks replay --tail 50 --limit 10 --batch-size 5 --apply --max-apply 3 --stop-on-error --report-out hooks-replay-apply.json
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
cargo run -p mosaic-cli --bin mosaic -- --project-state cron logs --tail 20 --summary --since-minutes 60
cargo run -p mosaic-cli --bin mosaic -- --project-state cron replay --tail 50 --limit 10 --batch-size 5 --reason hook_failures --retryable-only --report-out cron-replay-plan.json
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes cron replay --tail 50 --limit 10 --batch-size 5 --apply --max-apply 3 --stop-on-error --report-out cron-replay-apply.json
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

cargo run -p mosaic-cli --bin mosaic -- --project-state webhooks logs --tail 20 --summary --since-minutes 60
cargo run -p mosaic-cli --bin mosaic -- --project-state webhooks replay --tail 50 --limit 10 --batch-size 5 --reason auth --report-out webhooks-replay-plan.json
cargo run -p mosaic-cli --bin mosaic -- --project-state --yes webhooks replay --tail 50 --limit 10 --batch-size 5 --apply --max-apply 3 --report-out webhooks-replay-apply.json
```

### Browser Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser start
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser navigate --url mock://ok?title=Docs
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser status
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser tabs --tail 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser diagnose --stale-after-minutes 30 --probe-url mock://ok --probe-url mock://404
cargo run -p mosaic-cli --bin mosaic -- --project-state --json browser diagnose --stale-after-minutes 30 --artifact-max-age-hours 168 --repair
# diagnose includes network failure class breakdown + optional active probes
# --artifact-max-age-hours adds screenshot retention checks
# --repair also cleans orphan/corrupt/stale screenshot artifacts
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
cargo run -p mosaic-cli --bin mosaic -- --project-state channels logs --channel <channel-id> --tail 20 --summary
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 50 --limit 5
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 200 --since-minutes 30 --limit 20
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 50 --limit 5 --include-non-retryable
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 50 --limit 5 --reason rate_limited --reason upstream_5xx
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 100 --limit 20 --batch-size 5 --http-status 500 --min-attempt 2
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 50 --limit 3 --apply
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 50 --limit 3 --apply --require-full-payload
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 50 --limit 3 --apply --stop-on-error
cargo run -p mosaic-cli --bin mosaic -- --project-state channels replay <channel-id> --tail 100 --limit 10 --apply --max-apply 3 --report-out .mosaic/replay-report.json
# replay --apply uses stored full payload when available, with legacy text_preview fallback warnings
# replay --apply now runs channel readiness preflight and blocks early when token/target config is not ready
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
Knowledge guide: `docs/knowledge.md`
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
Sensitive-file override env (admin only): `MOSAIC_ALLOW_SENSITIVE_FILES=1`.
Sensitive-command override env (admin only): `MOSAIC_ALLOW_SENSITIVE_COMMANDS=1`.
Secret-redaction disable env (admin only): `MOSAIC_DISABLE_SECRET_REDACTION=1`.

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
./scripts/package_release_asset.sh --version <version> --target aarch64-apple-darwin

# generate brew/scoop manifests from collected assets
./scripts/update_distribution_manifests.sh --version <version> --assets-dir ./dist/<version> --output-dir ./dist/<version>

# generate release notes draft from worklog
./scripts/release_notes_from_worklog.sh --version <version> --out docs/release-notes-<version>.md

# one-command local release prepare
./scripts/release_prepare.sh --version <version>

# release tooling smoke (verifier pass/fail + dry-run summary)
./scripts/release_tooling_smoke.sh --version <version>

# release installer smoke (install.sh local assets + release-only guardrail)
./scripts/release_install_smoke.sh --version <version>

# verify archive internal contents
./scripts/release_verify_archives.sh --version <version> --assets-dir <dir>

# verify release assets directory
./scripts/release_verify_assets.sh --version <version> --assets-dir <dir>

# verify published GitHub release assets by tag
./scripts/release_publish_check.sh --version <version> --repo ooiai/mosaic --json
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
It also includes node/device/pairing runtime telemetry from `.mosaic/data/nodes-events.jsonl` under `nodes.*`, with summary counters for online/offline nodes, approved devices, pending pairings, and failed lifecycle events.

### Memory Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state memory index --path .
cargo run -p mosaic-cli --bin mosaic -- --project-state memory index --path . --incremental
cargo run -p mosaic-cli --bin mosaic -- --project-state memory index --path . --namespace ops --incremental --stale-after-hours 24 --retain-missing
cargo run -p mosaic-cli --bin mosaic -- --project-state memory search "rust cli"
cargo run -p mosaic-cli --bin mosaic -- --project-state memory search "gateway retry" --namespace ops
cargo run -p mosaic-cli --bin mosaic -- --project-state memory status
cargo run -p mosaic-cli --bin mosaic -- --project-state memory status --all-namespaces
cargo run -p mosaic-cli --bin mosaic -- --project-state memory clear
cargo run -p mosaic-cli --bin mosaic -- --project-state memory prune --max-namespaces 5 --dry-run
cargo run -p mosaic-cli --bin mosaic -- --project-state memory prune --max-documents-per-namespace 1000 --dry-run
cargo run -p mosaic-cli --bin mosaic -- --project-state memory prune --max-namespaces 5 --max-age-hours 168
cargo run -p mosaic-cli --bin mosaic -- --project-state memory policy get
cargo run -p mosaic-cli --bin mosaic -- --project-state memory policy set --enabled true --max-documents-per-namespace 1000 --min-interval-minutes 60
cargo run -p mosaic-cli --bin mosaic -- --project-state memory policy apply
```

### Knowledge Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source local_md --path docs --namespace knowledge
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source local_md --path docs --namespace knowledge --report-out .mosaic/reports/knowledge-local-ingest.json
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source local_md --path docs --namespace knowledge --incremental --stale-after-hours 24
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source http --url https://example.com/guide.md --namespace knowledge
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source http --url https://example.com/guide.md --header-env "Authorization=MOSAIC_DOC_TOKEN" --http-retries 3 --http-retry-backoff-ms 200 --namespace knowledge
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source http --url-file .mosaic/http-knowledge-urls.txt --continue-on-error --report-out .mosaic/reports/knowledge-http-ingest.json --namespace knowledge
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ingest --source mcp --mcp-server local-mcp --namespace knowledge
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge search "gateway retry policy" --namespace knowledge --limit 20 --min-score 4
cargo run -p mosaic-cli --bin mosaic -- --project-state knowledge ask "How does retry policy work?" --namespace knowledge --top-k 8 --min-score 6
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge ask "How does retry policy work?" --namespace knowledge --top-k 8 --min-score 6 --references-only
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge evaluate --query "gateway retry policy" --query "sandbox default profile" --namespace knowledge --top-k 8 --min-score 6 --report-out .mosaic/reports/knowledge-eval.json
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge evaluate --query "gateway retry policy" --namespace knowledge --history-window 20
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge evaluate --query "gateway retry policy" --namespace knowledge --update-baseline
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge evaluate --query "gateway retry policy" --namespace knowledge --max-coverage-drop 0.05 --max-avg-top-score-drop 1.0 --fail-on-regression
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge datasets list
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge datasets list --namespace knowledge
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge datasets remove knowledge --dry-run
cargo run -p mosaic-cli --bin mosaic -- --project-state --json knowledge datasets remove knowledge
```

### Security Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path .
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --deep
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --update-baseline
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --no-baseline
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --min-severity medium
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --category supply_chain --category cors --top 20
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --sarif
cargo run -p mosaic-cli --bin mosaic -- --project-state security audit --path . --sarif-output scan.sarif
cargo run -p mosaic-cli --bin mosaic -- --project-state security baseline show
cargo run -p mosaic-cli --bin mosaic -- --project-state security baseline add --fingerprint "<fp>"
cargo run -p mosaic-cli --bin mosaic -- --project-state security baseline clear
```

### Agents Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state agents list
cargo run -p mosaic-cli --bin mosaic -- --project-state agents add --name Writer --id writer --skill writer --set-default --route ask
cargo run -p mosaic-cli --bin mosaic -- --project-state agents update writer --name "Writer V2" --skill reviewer --route chat
cargo run -p mosaic-cli --bin mosaic -- --project-state agents update writer --clear-skills
cargo run -p mosaic-cli --bin mosaic -- --project-state agents show writer
cargo run -p mosaic-cli --bin mosaic -- --project-state agents route list
cargo run -p mosaic-cli --bin mosaic -- --project-state ask --agent writer "hello"
cargo run -p mosaic-cli --bin mosaic -- --project-state --json session show <session-id>
cargo run -p mosaic-cli --bin mosaic -- --project-state chat
# inside the REPL:
/agent writer
cargo run -p mosaic-cli --bin mosaic -- --project-state tui
# inside the TUI input:
/agent writer
```

If you resume a session with `ask/chat/tui --session <id>` and do not pass `--agent`, Mosaic now reuses the session's last bound agent before considering route/default-agent fallback. Inside `chat` and interactive `tui`, `/agent <id>` switches the active agent, `/agents` shows the available inventory inline or opens the picker, and if the current conversation already has a session Mosaic resets to a new session before applying the new binding. Interactive `tui` also supports `/session <id>`, `/new`, `/status`, and `Ctrl+S` for direct session control from the input flow.

For CLI-side diagnosis, `mosaic --project-state agents current [--agent <id>] [--session <id>] [--route ask|chat|tui]` now explains which layer wins: explicit agent, session runtime, route binding, default agent, or plain profile fallback.

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
