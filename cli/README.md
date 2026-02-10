# Mosaic CLI (Rust)

Mosaic CLI is the first-phase Rust implementation of a local agent workflow inspired by OpenClaw.
This workspace ships a pure CLI with no frontend dependency.

## Scope (V1)

- Local agent core (`ask`, `chat`, `session`, `models`, `status`, `health`, `doctor`)
- Gateway skeleton (`gateway run|status|health`)
- Channels skeleton (`channels add|list|login`)
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
cargo run -p mosaic-cli --bin mosaic -- --project-state gateway stop
```

### Channels Runtime

```bash
cargo run -p mosaic-cli --bin mosaic -- --project-state channels add --name demo --kind mock
cargo run -p mosaic-cli --bin mosaic -- --project-state channels list
cargo run -p mosaic-cli --bin mosaic -- --project-state channels send <channel-id> --text "hello"
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
