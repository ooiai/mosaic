# Mosaic

<p align="center">
  <strong>A self-hosted AI assistant control plane.</strong>
</p>

<p align="center">
  Long-running, multi-channel, stateful, routable, executable, extensible, and governable.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-orange?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/Cargo-Workspace-blue" alt="Cargo Workspace">
  <img src="https://img.shields.io/badge/Interface-Terminal%20Control%20Plane-0f766e" alt="Terminal Control Plane">
  <img src="https://img.shields.io/badge/License-MIT-black" alt="MIT License">
</p>

---

## Table of Contents

- [What Mosaic Is](#what-mosaic-is)
- [What Mosaic Is Not](#what-mosaic-is-not)
- [Architecture at a Glance](#architecture-at-a-glance)
- [Repository Layout](#repository-layout)
- [Quick Start](#quick-start)
- [Current Operator Surface](#current-operator-surface)
- [Engineering Rules](#engineering-rules)
- [Contributing](#contributing)

## What Mosaic Is

Mosaic is not designed as a single chat frontend. It is a self-hosted control plane for always-on AI agents that can:

- accept input from external channels such as WhatsApp, Telegram, Slack, Discord, and WebChat
- coordinate sessions, routing, permissions, memory, and event streams through a central Gateway
- orchestrate primary agents, sub-agents, model scheduling, and context compression
- expose operator surfaces such as terminal, web, and desktop control interfaces
- execute tools and device-node actions across browser, exec, canvas, pdf, image, cron, webhook, and file capabilities

In short: Mosaic is closer to an Agent OS than to a chatbot app.

## What Mosaic Is Not

Mosaic should not be treated as:

- a thin "chat UI + LLM API" wrapper
- a single-request/single-response assistant
- a product whose architecture is centered on one channel or one model
- a place where business orchestration is hidden inside adapters or generic infrastructure

## Architecture at a Glance

```mermaid
flowchart LR
    A[External Channels<br/>WhatsApp / Telegram / Slack / Discord / WebChat]
    B[Gateway / Control Plane<br/>Ingress / Auth / Routing / Sessions / Events]
    C[Agent Runtime<br/>Primary Agent / Sub-Agents / Model Scheduling / Compression]
    D[Capabilities<br/>Exec / Browser / Canvas / PDF / Image / Cron / Webhook / Files]
    E[Device Nodes<br/>macOS / iOS / Android / Headless]
    F[Operator Surfaces<br/>CLI / Web / Desktop]
    G[Config and Extensions<br/>mosaic.json / skills / plugins / workspace]

    A --> B
    F --> B
    G --> B
    B --> C
    C --> D
    C --> E
    D --> B
    E --> B
```

### Core Layers

| Layer                       | Responsibility                                                   | Meaning                                      |
| --------------------------- | ---------------------------------------------------------------- | -------------------------------------------- |
| Interaction Entry           | Normalize channel payloads and preserve context                  | Put the assistant where users already are    |
| Control Plane               | Ingress, auth, session mapping, routing, event broadcast         | The Gateway is the system hub                |
| Agent Runtime               | Agent orchestration, model strategy, compression, planning       | Runtime behavior is more than one model call |
| Capability Execution        | Run tools with permission boundaries and side-effect control     | Capabilities define both power and risk      |
| Device Node                 | Expose device-local actions through reconnectable node protocols | Extend agents into real devices              |
| Configuration and Extension | Control policies, plugins, skills, workspaces, hot reload        | Enable long-term product evolution           |

<details>
<summary><strong>Architecture principles</strong></summary>

- Channels are entrypoints, not the center of the system.
- The Gateway coordinates; it should not become a dumping ground.
- The runtime owns orchestration and collaboration, not just prompt execution.
- Tools and device nodes must preserve explicit permission boundaries.
- Configuration should describe behavior, not replace architecture.

</details>

## Repository Layout

Mosaic is organized as a Cargo workspace.

```text
mosaic/
|-- Cargo.toml
|-- Makefile
|-- README.md
|-- cli/
|   |-- Cargo.toml
|   `-- src/main.rs
`-- crates/
    `-- tui/
        |-- Cargo.toml
        `-- src/
```

### Workspace Rules

| Path           | Role                  | Rule                                                |
| -------------- | --------------------- | --------------------------------------------------- |
| `cli/`         | Composition root      | Start first-pass feature work here                  |
| `crates/`      | Reusable module layer | Move logic here once it is clearly shared or stable |
| workspace root | Consistency boundary  | Own shared dependencies, linting, and build policy  |

## Quick Start

Use the root `Makefile` as the standard entrypoint for local CLI workflows.

```bash
mosaic setup init
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
mosaic tui
mosaic session list
mosaic inspect .mosaic/runs/<run-id>.json
```

### Standard Commands

| Command        | Purpose                            |
| -------------- | ---------------------------------- |
| `make install` | Install the CLI binary from `cli/` |
| `make build`   | Build the CLI crate                |
| `make clean`   | Clean workspace build artifacts    |
| `make check`   | Run workspace checks               |

### Release And Test Commands

- `make test-unit`
- `make test-integration`
- `make test-matrix`
- `make test-golden`
- `MOSAIC_REAL_TESTS=1 make test-real`
- `make release-check`
- `make package`

The installed binary name is `mosaic`.

## Operator Docs

- [`docs/getting-started.md`](./docs/getting-started.md)
- [`docs/cli.md`](./docs/cli.md)
- [`docs/channels.md`](./docs/channels.md)
- [`docs/skills.md`](./docs/skills.md)
- [`docs/sandbox.md`](./docs/sandbox.md)
- [`docs/capabilities.md`](./docs/capabilities.md)
- [`docs/telegram-step-by-step.md`](./docs/telegram-step-by-step.md)
- [`docs/full-stack.md`](./docs/full-stack.md)
- [`docs/telegram-real-e2e.md`](./docs/telegram-real-e2e.md)
- [`docs/real-vs-mock-acceptance.md`](./docs/real-vs-mock-acceptance.md)
- [`docs/residual-mock-first-audit.md`](./docs/residual-mock-first-audit.md)
- [`docs/provider-runtime-policy-matrix.md`](./docs/provider-runtime-policy-matrix.md)
- [`docs/writer-ownership.md`](./docs/writer-ownership.md)
- [`docs/deployment.md`](./docs/deployment.md)
- [`docs/security.md`](./docs/security.md)
- [`docs/session-inspect-incident.md`](./docs/session-inspect-incident.md)
- [`docs/testing.md`](./docs/testing.md)
- [`docs/non-tui-architecture-audit.md`](./docs/non-tui-architecture-audit.md)
- [`docs/release.md`](./docs/release.md)

## Examples

- [`examples/README.md`](./examples/README.md)
- [`examples/TESTING.md`](./examples/TESTING.md)
- [`examples/skills/README.md`](./examples/skills/README.md)
- [`examples/capabilities/README.md`](./examples/capabilities/README.md)
- [`examples/sandbox/README.md`](./examples/sandbox/README.md)
- [`examples/composition/README.md`](./examples/composition/README.md)
- [`examples/channels/README.md`](./examples/channels/README.md)
- [`examples/channels/telegram-photo-update.json`](./examples/channels/telegram-photo-update.json)
- [`examples/channels/telegram-document-update.json`](./examples/channels/telegram-document-update.json)
- [`examples/skills/native-skill.yaml`](./examples/skills/native-skill.yaml)
- [`examples/skills/manifest-skill.yaml`](./examples/skills/manifest-skill.yaml)
- [`examples/skills/operator-note/SKILL.md`](./examples/skills/operator-note/SKILL.md)
- [`examples/capabilities/builtin-tool.yaml`](./examples/capabilities/builtin-tool.yaml)
- [`examples/capabilities/node-routed-tool.yaml`](./examples/capabilities/node-routed-tool.yaml)
- [`examples/capabilities/workflow.yaml`](./examples/capabilities/workflow.yaml)
- [`examples/sandbox/python-markdown-skill-pack.yaml`](./examples/sandbox/python-markdown-skill-pack.yaml)
- [`examples/sandbox/node-manifest-skill.yaml`](./examples/sandbox/node-manifest-skill.yaml)
- [`examples/composition/openai-capability-composition.config.yaml`](./examples/composition/openai-capability-composition.config.yaml)
- [`examples/full-stack/README.md`](./examples/full-stack/README.md)
- [`examples/full-stack/openai-telegram-single-bot.config.yaml`](./examples/full-stack/openai-telegram-single-bot.config.yaml)
- [`examples/full-stack/openai-telegram-e2e.config.yaml`](./examples/full-stack/openai-telegram-e2e.config.yaml)
- [`examples/full-stack/openai-telegram-multi-bot.config.yaml`](./examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [`examples/full-stack/openai-telegram-multimodal.config.yaml`](./examples/full-stack/openai-telegram-multimodal.config.yaml)
- [`examples/full-stack/openai-telegram-bot-split.config.yaml`](./examples/full-stack/openai-telegram-bot-split.config.yaml)
- [`examples/full-stack/openai-webchat.config.yaml`](./examples/full-stack/openai-webchat.config.yaml)
- [`examples/deployment/production.config.yaml`](./examples/deployment/production.config.yaml)

## First Real Flow

The operator-first path is:

1. `mosaic setup init`
2. `mosaic setup validate`
3. `mosaic setup doctor`
4. `mosaic config show`
5. `mosaic model list`
6. `mosaic tui`
7. Use `/mosaic help` inside a real channel or the interactive console.
8. Inspect the resulting run with `mosaic inspect .mosaic/runs/<run-id>.json`

For the Telegram-first real acceptance lane, start with:

- [`docs/telegram-step-by-step.md`](./docs/telegram-step-by-step.md)
- [`docs/telegram-real-e2e.md`](./docs/telegram-real-e2e.md)
- [`docs/full-stack.md`](./docs/full-stack.md)

For release evidence, use:

- `make release-check`
- `make test-matrix`
- `make test-golden`

## Skill System

Mosaic now has three skill sources:

- native Rust skills
- manifest-backed skills
- markdown skill packs centered on `SKILL.md`

Start here:

- [`docs/skills.md`](./docs/skills.md)
- [`examples/skills/README.md`](./examples/skills/README.md)

## Sandbox Model

Mosaic sandboxing is both:

- policy and execution guardrails
- workspace-local execution environments under `.mosaic/sandbox/`

Start here:

- [`docs/sandbox.md`](./docs/sandbox.md)
- [`examples/sandbox/README.md`](./examples/sandbox/README.md)

## Capability Taxonomy

Mosaic distinguishes:

- builtin tool
- MCP tool
- node-routed capability
- skill
- workflow

The shared operator vocabulary lives in:

- [`docs/capabilities.md`](./docs/capabilities.md)
- [`examples/capabilities/README.md`](./examples/capabilities/README.md)

## Crate Guide

- [`./cli/README.md`](./cli/README.md)
- [`./crates/channel-telegram/README.md`](./crates/channel-telegram/README.md)
- [`./crates/config/README.md`](./crates/config/README.md)
- [`./crates/control-protocol/README.md`](./crates/control-protocol/README.md)
- [`./crates/extension-core/README.md`](./crates/extension-core/README.md)
- [`./crates/gateway/README.md`](./crates/gateway/README.md)
- [`./crates/inspect/README.md`](./crates/inspect/README.md)
- [`./crates/mcp-core/README.md`](./crates/mcp-core/README.md)
- [`./crates/memory/README.md`](./crates/memory/README.md)
- [`./crates/node-protocol/README.md`](./crates/node-protocol/README.md)
- [`./crates/provider/README.md`](./crates/provider/README.md)
- [`./crates/runtime/README.md`](./crates/runtime/README.md)
- [`./crates/sandbox-core/README.md`](./crates/sandbox-core/README.md)
- [`./crates/scheduler-core/README.md`](./crates/scheduler-core/README.md)
- [`./crates/sdk/README.md`](./crates/sdk/README.md)
- [`./crates/session-core/README.md`](./crates/session-core/README.md)
- [`./crates/skill-core/README.md`](./crates/skill-core/README.md)
- [`./crates/tool-core/README.md`](./crates/tool-core/README.md)
- [`./crates/tui/README.md`](./crates/tui/README.md)
- [`./crates/workflow/README.md`](./crates/workflow/README.md)

## Current Operator Surface

The repository currently includes the first terminal control-plane slice.

<details open>
<summary><strong>TUI capabilities</strong></summary>

- left session list
- center task and conversation timeline
- top status bar with workspace, session, model, runtime, and gateway state
- bottom composer for operator instructions
- right observability panel for logs and activity
- keyboard-first navigation
- local mock control commands for stage-2 interaction flows

</details>

<details>
<summary><strong>Keyboard model</strong></summary>

| Key                     | Action                       |
| ----------------------- | ---------------------------- |
| `Tab` / `Shift+Tab`     | Cycle focus between panes    |
| `j` / `k` or arrow keys | Move within the focused pane |
| `i`                     | Jump to composer             |
| `Enter`                 | Submit composer input        |
| `Ctrl+L`                | Toggle observability panel   |
| `Esc`                   | Return focus to sessions     |
| `q` / `Ctrl+C`          | Quit                         |

</details>

<details>
<summary><strong>Local mock commands</strong></summary>

| Command                 | Effect                                        |
| ----------------------- | --------------------------------------------- | ---------- | --------------------------------- |
| `/help`                 | Show local control commands in the timeline   |
| `/logs`                 | Toggle the observability panel                |
| `/gateway connect`      | Mark gateway as connected in the local TUI    |
| `/gateway disconnect`   | Mark gateway as disconnected in the local TUI |
| `/runtime <status>`     | Update the runtime status label               |
| `/session state <active | waiting                                       | degraded>` | Update the selected session state |
| `/session model <name>` | Update the selected session model             |

</details>

## Engineering Rules

The repository is guided by a few non-negotiable boundaries:

- treat Mosaic as a control plane, not a chat app
- start new one-step feature work in `cli/`
- extract shared, stable logic into `crates/`
- keep Gateway semantics inside Mosaic crates, not generic infrastructure
- preserve compatibility unless a change is intentionally breaking
- keep authorization, auditability, observability, and interruption paths explicit

## Contributing

Before modifying the repository:

- read [`AGENTS.md`](./AGENTS.md) for architecture boundaries and repository rules
- read [`Constraints.md`](./Constraints.md) for the minimal-change constraint
- prefer the smallest safe change that preserves existing behavior

If repeated logic appears, extract one semantic implementation instead of creating multiple near-duplicates.
