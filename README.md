# Mosaic

Mosaic is a self-hosted AI assistant control plane.

It gives you one operator surface for sessions, a TUI, a local or HTTP Gateway, traces, capability execution, extensions, nodes, and multi-channel ingress. The goal of this README is simple: get a new operator from zero to a real conversation without reading the source.

## 3 Minute Quick Start

1. Build or install the CLI.

```bash
cargo install --path cli
# or, during development
cargo run -p mosaic-cli -- --help
```

2. Initialize the workspace config.

```bash
mosaic setup init
```

3. Configure a real provider in `.mosaic/config.yaml`.

Use one of these examples:

- [examples/providers/openai.yaml](./examples/providers/openai.yaml)
- [examples/providers/ollama.yaml](./examples/providers/ollama.yaml)
- [examples/providers/anthropic.yaml](./examples/providers/anthropic.yaml)

4. Validate the config and run doctor.

```bash
mosaic setup validate
mosaic setup doctor
```

5. Start the TUI and send a message.

```bash
mosaic tui
```

6. Inspect the session and the saved trace.

```bash
mosaic session list
mosaic session show <session-id>
mosaic inspect .mosaic/runs/<run-id>.json
```

## Install

### Option A: Install the binary

```bash
cargo install --path cli
mosaic --help
```

### Option B: Run from the workspace

```bash
cargo run -p mosaic-cli -- --help
```

### Common developer entrypoints

```bash
make build
make check
```

## First Run Path

### 1. Initialize the workspace

```bash
mosaic setup init
```

This creates:

- `.mosaic/config.yaml`
- `.mosaic/sessions/`
- `.mosaic/runs/`
- `.mosaic/audit/`
- `.mosaic/extensions/`

### 2. Configure a provider

Open `.mosaic/config.yaml` and replace the default `mock` profile with a real profile, or merge one of the provider examples from [`examples/providers/`](./examples/providers/README.md).

For OpenAI:

```yaml
active_profile: openai
profiles:
  openai:
    type: openai-compatible
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

Then export the credential:

```bash
export OPENAI_API_KEY=your_api_key_here
```

### 3. Validate and diagnose

```bash
mosaic setup validate
mosaic setup doctor
mosaic model list
```

### 4. Start the operator surface

```bash
mosaic tui
```

Inside the TUI, type a message and press `Enter`.

### 5. Inspect state and traces

```bash
mosaic session list
mosaic session show default
mosaic inspect .mosaic/runs/<run-id>.json
```

If you want a quick non-TUI smoke run:

```bash
mosaic run examples/time-now-agent.yaml --session quickstart
```

## Documentation

- [Getting Started](./docs/getting-started.md)
- [Configuration Reference](./docs/configuration.md)
- [Provider Guide](./docs/providers.md)
- [CLI Reference](./docs/cli.md)
- [TUI Guide](./docs/tui.md)
- [Gateway Guide](./docs/gateway.md)
- [Troubleshooting](./docs/troubleshooting.md)

## Examples

- [examples/README.md](./examples/README.md)
- [examples/providers/](./examples/providers/)
- [examples/workflows/](./examples/workflows/)
- [examples/extensions/](./examples/extensions/)
- [examples/gateway/](./examples/gateway/)

## Current Provider Support

Mosaic currently supports these provider modes in the runtime:

- `mock`
- `openai-compatible`

That means:

- OpenAI works directly with `type: openai-compatible`
- Ollama works through its OpenAI-compatible endpoint
- Anthropic currently needs an OpenAI-compatible bridge or proxy if you want to use it from Mosaic today

Details and examples are in [docs/providers.md](./docs/providers.md).

## Useful First Commands

```bash
mosaic setup init
mosaic setup validate
mosaic setup doctor
mosaic model list
mosaic tui
mosaic session list
mosaic gateway status
mosaic inspect .mosaic/runs/<run-id>.json
```

## Architecture

Mosaic is still a control plane, not a thin chat wrapper. If you need the architectural rules and contributor boundaries, read [AGENTS.md](./AGENTS.md).
