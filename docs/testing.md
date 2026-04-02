# Testing Guide

Mosaic testing is not one bucket of `cargo test`. It is a product-proof system with fixed layers, fixed release roles, and an explicit mapping from each key crate to at least one real or quasi-real acceptance lane.

This document is the source of truth for the k5 channel product baseline.

Capability taxonomy for test interpretation is defined in [capabilities.md](./capabilities.md). When a test or runbook claims that a lane proves "tool", "MCP", "node", "skill", or "workflow", it should use the same `route_kind`, `capability_source_kind`, `execution_target`, and `failure_origin` vocabulary described there.

See also:

- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [providers.md](./providers.md)
- [release.md](./release.md)

## Fixed Layers

Mosaic uses five test layers.

| Layer | Meaning | Typical command | What is allowed |
| --- | --- | --- | --- |
| `unit` | isolated crate logic and small contracts | `make test-unit` | mocks and pure in-process fixtures |
| `local integration` | public crate API, file IO, local HTTP, subprocesses, local persistence | `make test-integration` | local processes, local ports, local filesystem |
| `protocol-real` | a real transport or protocol surface, but not necessarily the final external product environment | targeted `cargo test ... --test real_*` | real HTTP, SSE, stdio, webhook handlers, SDK clients |
| `product-real` | a real provider, real channel or ingress shape, real persistence, real operator flow, real artifacts | `MOSAIC_REAL_TESTS=1 make test-real` or the Telegram runbook | no mock-only evidence |
| `release-blocking acceptance` | the lanes that must be proven before shipping the scoped release target | `make test-matrix`, `MOSAIC_REAL_TESTS=1 make test-real`, plus Telegram manual sign-off when Telegram is in scope | real evidence only |

Rules:

- Mock is acceptable in `unit` and selected `local integration` tests.
- Mock is not acceptable as `product-real` or `release-blocking acceptance` evidence.
- A `protocol-real` lane proves a contract boundary. It does not automatically prove the whole product story.
- A `product-real` lane should write real session, trace, audit, replay, or incident artifacts whenever the surface supports them.

## Release Roles

Mosaic uses three release roles for real testing.

### 1. Automated release-blocking acceptance

These lanes must pass in automation before a release is cut:

| Lane | Command | Why it blocks release |
| --- | --- | --- |
| test matrix consistency | `make test-matrix` | proves docs, scripts, examples, and release instructions still describe the same lanes |
| OpenAI provider-real lane | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | proves at least one first-class external provider works without mock data |
| Gateway protocol-real lane | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture` | proves real HTTP server and SSE client paths |
| MCP protocol-real lane | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture` | proves real subprocess transport |
| OpenAI + WebChat product-real lane | `MOSAIC_REAL_TESTS=1 ./scripts/test-full-stack-example.sh openai-webchat` | proves setup -> gateway -> ingress -> session -> inspect -> incident without mock |

### 2. Operator-manual release-blocking acceptance

These lanes are release-blocking when the feature is in scope, but they require operator-managed infrastructure:

| Lane | Command / Runbook | Scope |
| --- | --- | --- |
| Telegram-first release-blocking acceptance lane | [telegram-real-e2e.md](./telegram-real-e2e.md) | required when Telegram is a release target |
| live public webhook routing | operator-managed HTTPS reverse proxy or tunnel | required for Telegram bot acceptance |

### 3. Compatibility addendum lanes

These are real lanes, but they are compatibility proof rather than the default operator story:

| Vendor | Command | Role |
| --- | --- | --- |
| Azure | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | vendor compatibility addendum |
| Anthropic | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | vendor compatibility addendum |
| Ollama | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | local real-model addendum |

## k5 Channel Product Scenarios

The current channel product baseline adds these operator-visible proofs:

| Scenario | Where it is proven | Classification |
| --- | --- | --- |
| channel command catalog discovery | `/mosaic help` and `/mosaic help tools` inside [telegram-real-e2e.md](./telegram-real-e2e.md) | `product-real` |
| Telegram image upload | Telegram photo lane in [telegram-real-e2e.md](./telegram-real-e2e.md) using [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json) | `product-real` |
| Telegram document upload | Telegram document lane in [telegram-real-e2e.md](./telegram-real-e2e.md) using [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json) | `product-real` |
| specialized processor routing | [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml) and the document lane in [telegram-real-e2e.md](./telegram-real-e2e.md) | `product-real` |
| dual-bot Gateway routing | [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml) plus the bot A / bot B isolation section in [telegram-real-e2e.md](./telegram-real-e2e.md) | `product-real` |
| per-bot webhook management CLI | `mosaic adapter telegram webhook set --bot ...`, `info --bot ...`, and `test-send --bot ...` in [docs/cli.md](./cli.md) and [telegram-real-e2e.md](./telegram-real-e2e.md) | `product-real` |

## Crate-by-Crate Product Proof Matrix

Every key crate must map to a primary real proof lane or an explicit supplemental lane.

| Crate | Primary proof lane | Supplemental lane | Highest classification |
| --- | --- | --- | --- |
| `mosaic-config` | `mosaic setup init`, `mosaic setup validate`, `mosaic setup doctor`, and `mosaic config show` inside `./scripts/test-full-stack-example.sh openai-webchat` | Telegram-first workspace bootstrap, bot registry examples, and attachment config examples | `release-blocking acceptance` |
| `mosaic-provider` | `cargo test -p mosaic-provider --test real_vendors -- --nocapture` | OpenAI + WebChat full-stack, Telegram image upload, Telegram document upload | `release-blocking acceptance` |
| `mosaic-runtime` | OpenAI + WebChat full-stack lane writes real session and trace artifacts | Telegram-first lane proves tool, skill, workflow, attachment, and route planning in a real channel session | `product-real` |
| `mosaic-tool-core` | Telegram-first lane proves real `time_now` and `read_file` invocations | local integration tests for builtin exec/webhook coverage | `product-real` |
| `mosaic-skill-core` | Telegram-first lane proves `/mosaic skill summarize_notes` in a real Telegram session | attachment-aware manifest in [examples/extensions/telegram-e2e.yaml](../examples/extensions/telegram-e2e.yaml) | `product-real` |
| `mosaic-workflow` | Telegram-first lane proves `/mosaic workflow summarize_operator_note` in a real Telegram session | attachment-aware workflow manifest in [examples/extensions/telegram-e2e.yaml](../examples/extensions/telegram-e2e.yaml) | `product-real` |
| `mosaic-extension-core` | Telegram-first lane loads `examples/extensions/telegram-e2e.yaml` and exposes capabilities through Gateway and CLI | extension registry integration tests | `product-real` |
| `mosaic-gateway` | OpenAI + WebChat full-stack lane proves real HTTP, session, audit, replay, and incident flow | Telegram-first lane proves live Telegram webhook ingress, attachment normalization, multi-bot routing, and outbound reply | `release-blocking acceptance` |
| `mosaic-session-core` | OpenAI + WebChat full-stack lane proves persisted sessions and routing metadata | Telegram-first lane proves persisted Telegram session continuity, bot metadata, and thread continuity | `product-real` |
| `mosaic-inspect` | OpenAI + WebChat full-stack lane proves saved trace and incident export | Telegram-first lane proves operator-facing `inspect --verbose` against live Telegram artifacts, attachment route, and bot identity | `product-real` |
| `mosaic-control-protocol` | `cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture` proves the real control-plane DTO path | CLI attach flows and remote Gateway operator commands | `protocol-real` |
| `mosaic-sdk` | `cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture` | remote operator attach commands in CLI and TUI | `protocol-real` |
| `mosaic-channel-telegram` | `cargo test -p mosaic-gateway --test real_telegram_ingress -- --nocapture` proves the real webhook contract path | Telegram-first lane proves live inbound normalize plus outbound reply, image uploads, and document uploads | `product-real` |
| `mosaic-mcp-core` | `cargo test -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture` | `examples/mcp-filesystem.yaml` golden path | `protocol-real` |
| `mosaic-memory` | OpenAI + WebChat full-stack and Telegram-first lanes persist real session memory and references | file store integration tests | `product-real` |
| `mosaic-node-protocol` | local integration file-bus tests | node operator flows in CLI | `local integration` |
| `mosaic-scheduler-core` | local integration cron store tests | CLI cron flows | `local integration` |
| `mosaic-cli` | `make test-golden`, `make test-matrix`, and the Telegram-first runbook | command regression tests in `cli/src/main.rs` | `release-blocking acceptance` |
| `mosaic-tui` | local operator-console tests | remote attach and session refresh tests | `local integration` |

Interpretation:

- Telegram-first is the primary product proof for `tool-core`, `skill-core`, `workflow`, `extension-core`, `channel-telegram`, and most operator-facing CLI behavior.
- OpenAI + WebChat is the primary automated no-mock release lane for `config`, `gateway`, `runtime`, `session-core`, and `inspect`.
- Provider vendor breadth is carried by `real_vendors.rs`, not by forcing every vendor through the Telegram lane.
- `node-protocol`, `scheduler-core`, and `tui` are not currently proven by the Telegram-first release lane; their highest proof class remains local integration until a broader real product lane is added.

## Telegram-First Release Lane

The Telegram-first release-blocking acceptance lane proves:

- real OpenAI provider
- real Telegram inbound message
- real Telegram outbound reply
- `/mosaic help` category discovery
- real builtin tool path
- real manifest skill path
- real workflow path
- real image upload handling
- real document upload handling
- real session persistence
- real inspect, audit, replay, and incident artifacts
- real CLI operator flow for setup, webhook management, and diagnosis
- real dual-bot routing when the multi-bot config is in scope

Use [telegram-real-e2e.md](./telegram-real-e2e.md) as the operator runbook. That runbook is the product-level proof for Telegram. Do not replace it with mock payloads when Telegram is in scope.

## Commands

Fast local safety net:

```bash
make test-unit
make test-integration
```

Docs, examples, and matrix consistency:

```bash
make test-matrix
make test-golden
./scripts/test-full-stack-example.sh mock
```

Automated real lanes:

```bash
MOSAIC_REAL_TESTS=1 make test-real
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

Telegram-first operator sign-off:

```bash
See docs/telegram-real-e2e.md
```

## Environment and Secrets

Real lanes are enabled by `MOSAIC_REAL_TESTS=1`, then narrowed by the secrets and services that are present.

| Variable | Purpose |
| --- | --- |
| `OPENAI_API_KEY` | OpenAI provider-real lane and the OpenAI + WebChat full-stack lane |
| `AZURE_OPENAI_API_KEY` | Azure compatibility addendum |
| `ANTHROPIC_API_KEY` | Anthropic compatibility addendum |
| `MOSAIC_TEST_OLLAMA_BASE_URL` / `MOSAIC_TEST_OLLAMA_MODEL` | Ollama local real-model addendum |
| `MOSAIC_WEBCHAT_SHARED_SECRET` | OpenAI + WebChat full-stack ingress |
| `MOSAIC_TELEGRAM_BOT_TOKEN` | Telegram-first manual acceptance and CLI webhook management |
| `MOSAIC_TELEGRAM_SECRET_TOKEN` | Telegram webhook auth |
| `MOSAIC_TELEGRAM_OPS_BOT_TOKEN` / `MOSAIC_TELEGRAM_MEDIA_BOT_TOKEN` | multi-bot Telegram operator lanes |
| `MOSAIC_TELEGRAM_OPS_SECRET_TOKEN` / `MOSAIC_TELEGRAM_MEDIA_SECRET_TOKEN` | multi-bot webhook auth |
| `MOSAIC_PUBLIC_WEBHOOK_BASE_URL` | Telegram-first live webhook registration |
| `MOSAIC_OPERATOR_TOKEN` | optional remote Gateway operator auth |

## Flaky Test Policy

- A real lane may skip when its required secret, daemon, or public endpoint is absent.
- A local integration lane must not depend on public internet access.
- If a real upstream API is unstable, quarantine only the affected real lane instead of silently replacing it with mock coverage.
- If timing is the issue, prefer readiness polling and explicit health checks over blind retry loops.
- Do not claim release acceptance based on a skipped Telegram-first lane when Telegram is in release scope.

## Updating the Matrix

When you add a new crate, protocol surface, or acceptance story:

1. decide its highest proof class
2. update the crate row in this document
3. update [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md) if release roles changed
4. update [release.md](./release.md) if release-blocking or operator-manual sign-off changed
5. update `scripts/verify-test-matrix.sh` so repo automation enforces the same story

Do not add a real test without recording where it fits in the product proof system. Do not claim release acceptance based only on mock data.
