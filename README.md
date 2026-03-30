# Mosaic

Mosaic is a self-hosted AI assistant control plane.

It gives you one operator surface for sessions, a TUI, a local or HTTP Gateway, traces, capability execution, extensions, nodes, and multi-channel ingress. The goal of this README is simple: get a new operator from zero to a real conversation, then point them to the right Telegram, channel, and release docs without reading the source.

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
- [examples/providers/azure.yaml](./examples/providers/azure.yaml)
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

## Product Baseline

The current operator baseline is:

- WebChat and Telegram both enter through the Gateway
- Telegram supports `/mosaic` command discovery inside the chat
- Telegram attachments can route through a multimodal profile or a specialized processor policy
- one workspace can host multiple Telegram bots with isolated webhook paths, profiles, and capability scopes

If you want the shortest product path into a real channel, start with:

- [docs/telegram-step-by-step.md](./docs/telegram-step-by-step.md)
- [examples/full-stack/openai-telegram-single-bot.config.yaml](./examples/full-stack/openai-telegram-single-bot.config.yaml)
- [examples/extensions/telegram-e2e.yaml](./examples/extensions/telegram-e2e.yaml)

If Telegram is part of release scope, use:

- [docs/telegram-real-e2e.md](./docs/telegram-real-e2e.md)
- [examples/full-stack/openai-telegram-e2e.config.yaml](./examples/full-stack/openai-telegram-e2e.config.yaml)

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
make test-matrix
make test-golden
make smoke
make release-check
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

Open `.mosaic/config.yaml` and finish the real-provider-first template, or merge one of the provider examples from [`examples/providers/`](./examples/providers/README.md). If you want a local dev-only lane without credentials, re-run `mosaic setup init --dev-mock`.

For OpenAI:

```yaml
active_profile: openai
profiles:
  openai:
    type: openai
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

Then export the credential:

```bash
export OPENAI_API_KEY=your_api_key_here
```

For a production service install, continue with:

- [`.env.example`](./.env.example)
- [examples/deployment/production.config.yaml](./examples/deployment/production.config.yaml)
- [docs/deployment.md](./docs/deployment.md)
- [docs/security.md](./docs/security.md)

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

## Channel Command Catalog

Inside Telegram, Mosaic exposes a chat-native operator catalog through `/mosaic`.

Useful starting commands:

```text
/mosaic help
/mosaic help tools
/mosaic session status
/mosaic tool read_file .mosaic/config.yaml
/mosaic skill summarize_notes Shift handoff note goes here.
/mosaic workflow summarize_operator_note Workflow input goes here.
```

The catalog is dynamic. It only shows the tools, skills, and workflows that are visible in the current channel and allowed for the current bot policy.

## Telegram, Attachments, and Multi-Bot

Single-bot baseline:

- [examples/full-stack/openai-telegram-single-bot.config.yaml](./examples/full-stack/openai-telegram-single-bot.config.yaml)

Multi-bot routing:

- [examples/full-stack/openai-telegram-multi-bot.config.yaml](./examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](./examples/full-stack/openai-telegram-bot-split.config.yaml)

Image and document uploads:

- [examples/full-stack/openai-telegram-multimodal.config.yaml](./examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/channels/telegram-photo-update.json](./examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](./examples/channels/telegram-document-update.json)

These paths are explained in:

- [docs/channels.md](./docs/channels.md)
- [docs/configuration.md](./docs/configuration.md)
- [docs/telegram-step-by-step.md](./docs/telegram-step-by-step.md)
- [docs/telegram-real-e2e.md](./docs/telegram-real-e2e.md)

## Documentation

- [Getting Started](./docs/getting-started.md)
- [Configuration Reference](./docs/configuration.md)
- [Provider Guide](./docs/providers.md)
- [CLI Reference](./docs/cli.md)
- [TUI Guide](./docs/tui.md)
- [Gateway Guide](./docs/gateway.md)
- [Channel Setup](./docs/channels.md)
- [Telegram Step-by-Step Setup](./docs/telegram-step-by-step.md)
- [Full-Stack Guide](./docs/full-stack.md)
- [Telegram Real E2E Runbook](./docs/telegram-real-e2e.md)
- [Real vs Mock Acceptance](./docs/real-vs-mock-acceptance.md)
- [Residual Mock-First Audit](./docs/residual-mock-first-audit.md)
- [Provider and Runtime Policy Matrix](./docs/provider-runtime-policy-matrix.md)
- [Session, Inspect, and Incident Flow](./docs/session-inspect-incident.md)
- [Writer Ownership](./docs/writer-ownership.md)
- [Deployment Guide](./docs/deployment.md)
- [Operations Guide](./docs/operations.md)
- [Security Guide](./docs/security.md)
- [Testing Guide](./docs/testing.md)
- [Non-TUI Architecture Audit](./docs/non-tui-architecture-audit.md)
- [Release Guide](./docs/release.md)
- [Compatibility Guide](./docs/compatibility.md)
- [Upgrade Guide](./docs/upgrade.md)
- [Troubleshooting](./docs/troubleshooting.md)

## Crate Guide

Mosaic is a Cargo workspace. The `cli/` crate is the composition root, while the crates under `crates/` hold reusable system boundaries. Start with these READMEs when you need to understand ownership before changing code.

### Composition Root

- [cli/README.md](./cli/README.md) - operator-facing command entrypoint, bootstrap wiring, and top-level workflow composition

### Core Crates

- [crates/config/README.md](./crates/config/README.md) - config loading, validation, doctor output, and redaction
- [crates/provider/README.md](./crates/provider/README.md) - provider registry, capability metadata, retry, and vendor adapters
- [crates/tool-core/README.md](./crates/tool-core/README.md) - tool contracts, registry, built-in tools, and tool metadata
- [crates/skill-core/README.md](./crates/skill-core/README.md) - skill manifests, native skills, metadata, and registry behavior
- [crates/runtime/README.md](./crates/runtime/README.md) - agent run orchestration, model selection, tool loop, memory, and workflow dispatch
- [crates/gateway/README.md](./crates/gateway/README.md) - control-plane hub, ingress routing, run registry, HTTP and SSE surfaces
- [crates/session-core/README.md](./crates/session-core/README.md) - persistent sessions, transcripts, routes, and session metadata
- [crates/inspect/README.md](./crates/inspect/README.md) - trace loading, summary formatting, and operator inspection output

### Support Crates

- [crates/memory/README.md](./crates/memory/README.md) - session memory storage, summaries, compression, and search
- [crates/mcp-core/README.md](./crates/mcp-core/README.md) - MCP stdio server lifecycle and remote tool discovery
- [crates/sdk/README.md](./crates/sdk/README.md) - external HTTP and SSE client surface for operators and adapters
- [crates/control-protocol/README.md](./crates/control-protocol/README.md) - stable DTOs for gateway HTTP, SSE, sessions, and runs
- [crates/node-protocol/README.md](./crates/node-protocol/README.md) - local node registration, heartbeat, dispatch, and affinity contracts
- [crates/extension-core/README.md](./crates/extension-core/README.md) - extension manifests, validation, policy, and reload-safe registration
- [crates/scheduler-core/README.md](./crates/scheduler-core/README.md) - capability jobs, cron registration, and scheduler state
- [crates/channel-telegram/README.md](./crates/channel-telegram/README.md) - Telegram webhook adapter for multi-channel ingress
- [crates/workflow/README.md](./crates/workflow/README.md) - workflow manifests and step execution runner
- [crates/tui/README.md](./crates/tui/README.md) - operator console state machine, rendering, and gateway-backed interaction flow

## Examples

- [examples/README.md](./examples/README.md)
- [examples/TESTING.md](./examples/TESTING.md)
- [examples/providers/](./examples/providers/)
- [examples/channels/README.md](./examples/channels/README.md)
- [examples/channels/](./examples/channels/)
- [examples/channels/telegram-photo-update.json](./examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](./examples/channels/telegram-document-update.json)
- [examples/workflows/](./examples/workflows/)
- [examples/full-stack/openai-telegram-single-bot.config.yaml](./examples/full-stack/openai-telegram-single-bot.config.yaml)
- [examples/full-stack/openai-telegram-multi-bot.config.yaml](./examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [examples/full-stack/openai-telegram-multimodal.config.yaml](./examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](./examples/full-stack/openai-telegram-bot-split.config.yaml)
- [examples/extensions/](./examples/extensions/)
- [examples/gateway/](./examples/gateway/)
- [examples/full-stack/README.md](./examples/full-stack/README.md)
- [examples/full-stack/openai-telegram-e2e.config.yaml](./examples/full-stack/openai-telegram-e2e.config.yaml)
- [examples/full-stack/openai-webchat.config.yaml](./examples/full-stack/openai-webchat.config.yaml)
- [examples/full-stack/](./examples/full-stack/)
- [examples/deployment/](./examples/deployment/)

## Current Provider Support

Mosaic currently supports these provider modes in the runtime:

- `mock` via explicit `mosaic setup init --dev-mock`
- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`

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
make test-golden
make smoke
make release-check
```

## Architecture

Mosaic is still a control plane, not a thin chat wrapper. If you need the architectural rules and contributor boundaries, read [AGENTS.md](./AGENTS.md).
