# Getting Started

This guide takes a new operator from a fresh checkout to a real Mosaic conversation and a saved trace.

## Prerequisites

- Rust toolchain installed
- Cargo available in `PATH`
- one provider credential or local model endpoint

## 1. Install or run the CLI

Install the binary:

```bash
cargo install --path cli
mosaic --help
```

Or run it from the workspace during development:

```bash
cargo run -p mosaic-cli -- --help
```

## 2. Initialize the workspace

From the repository root:

```bash
mosaic setup init
```

This writes `.mosaic/config.yaml` and creates the default runtime directories.

If the file already exists and you want to regenerate it:

```bash
mosaic setup init --force
```

## 3. Choose a provider

Open `.mosaic/config.yaml` and configure a real profile.

### OpenAI example

Copy from [examples/providers/openai.yaml](../examples/providers/openai.yaml):

```yaml
active_profile: openai
profiles:
  openai:
    type: openai
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

Export the key:

```bash
export OPENAI_API_KEY=your_api_key_here
```

### Ollama example

Copy from [examples/providers/ollama.yaml](../examples/providers/ollama.yaml):

```yaml
active_profile: ollama
profiles:
  ollama:
    type: ollama
    model: qwen3:14b
    base_url: http://127.0.0.1:11434
```

### Anthropic example

Copy from [examples/providers/anthropic.yaml](../examples/providers/anthropic.yaml):

```yaml
active_profile: anthropic
profiles:
  anthropic:
    type: anthropic
    model: claude-sonnet-4-5
    base_url: https://api.anthropic.com/v1
    api_key_env: ANTHROPIC_API_KEY
```

Export the key:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

### Production template and secrets

If you plan to keep Mosaic running as a long-lived service, continue with:

- [`.env.example`](../.env.example)
- [examples/deployment/production.config.yaml](../examples/deployment/production.config.yaml)
- [docs/deployment.md](./deployment.md)
- [docs/security.md](./security.md)

## 4. Validate and diagnose

```bash
mosaic setup validate
mosaic setup doctor
mosaic model list
```

What success looks like:

- `validation: ok`
- `doctor: ok` or only expected warnings
- the active profile appears in `mosaic model list`

## 5. Start the TUI

```bash
mosaic tui
```

Inside the TUI:

- type a message in the composer
- press `Enter`
- watch the timeline update
- press `F1` for shortcuts and slash commands

## 6. Verify session state

After one turn:

```bash
mosaic session list
mosaic session show <session-id>
```

If you did not set a session name in the TUI, use `mosaic session list` first and then inspect the ID you want.

## 7. Inspect the saved trace

Every completed run writes a trace under `.mosaic/runs/`.

```bash
mosaic inspect .mosaic/runs/<run-id>.json
```

Look for:

- `effective_profile`
- `ingress`
- `governance`
- `tool_calls`
- `capability_invocations`

## 8. Run a simple file-based smoke test

```bash
mosaic run examples/time-now-agent.yaml --session quickstart
```

Then inspect the output and follow the printed next steps.

## Suggested first-day path

```bash
mosaic setup init
mosaic setup validate
mosaic setup doctor
mosaic model list
mosaic tui
mosaic session list
mosaic inspect .mosaic/runs/<run-id>.json
```

For a production handoff after the first day, continue with:

- [docs/deployment.md](./deployment.md)
- [docs/operations.md](./operations.md)
- [docs/upgrade.md](./upgrade.md)
