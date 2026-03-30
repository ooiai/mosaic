# Real vs Mock Acceptance Matrix

This document is the release-facing summary of the k5 testing model.

Mosaic keeps mock paths for speed, but mock paths are never product evidence by themselves.

## Rules

- Mock is allowed for fast regression and docs smoke loops.
- Mock is not valid release-blocking acceptance evidence.
- `protocol-real` proves a transport or contract boundary.
- `product-real` proves a real operator story with real artifacts.
- `release-blocking acceptance` is the subset of real lanes that must pass before shipping the scoped release target.

## Classification Matrix

| Surface | Path | Classification | Command / Artifact | Notes |
| --- | --- | --- | --- | --- |
| provider smoke | `mock` provider | dev-only | `cargo test -p mosaic-provider --test integration_mock_provider` | fast safety net only |
| provider vendors | OpenAI / Azure / Anthropic / Ollama | protocol-real and release-blocking for provider compatibility | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | OpenAI is the default release provider lane |
| gateway protocol | real HTTP + SSE through SDK | protocol-real and release-blocking | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture` | real server, real port, real SSE |
| MCP transport | stdio subprocess manager | protocol-real and release-blocking | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture` | real subprocess transport |
| webchat full stack | OpenAI + WebChat ingress + saved artifacts | product-real and automated release-blocking acceptance | `MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat` | primary automated no-mock lane |
| telegram webhook contract | local real Telegram HTTP handler | protocol-real | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-gateway --test real_telegram_ingress -- --nocapture` | real handler, not a live bot |
| Telegram-first acceptance | live bot token + public HTTPS webhook + OpenAI + CLI operator flow | product-real and operator-manual release-blocking acceptance | [telegram-real-e2e.md](./telegram-real-e2e.md) | required when Telegram is in release scope |
| mock full stack | mock provider + Telegram sample payload | dev-only | `./scripts/test-full-stack-example.sh mock` | docs and example smoke only |

## Release Roles

### Automated release-blocking lanes

- OpenAI provider-real lane
- `make test-matrix`
- `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture`
- `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture`
- `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture`
- `MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat`

### Operator-manual release-blocking sign-off

- Telegram-first release-blocking acceptance lane in [telegram-real-e2e.md](./telegram-real-e2e.md)

Use this lane when:

- Telegram is a release target
- outbound bot delivery must be proven against the real Telegram Bot API
- `/mosaic` command catalog discovery must be proven in a live channel
- image and document uploads must be proven in the live Telegram path when attachments are in scope
- multi-bot webhook routing or per-bot webhook management CLI is in scope
- the operator needs final evidence that `session`, `inspect`, `audit`, `replay`, and `incident` all match the live Telegram conversation

### Compatibility addendum lanes

- Azure real vendor path
- Anthropic real vendor path
- Ollama local real-model path

These remain real tests, but they are compatibility proof rather than the main operator story.

## What Still Uses Mock

Mock is intentionally retained for:

- fast unit and local integration safety nets
- docs smoke paths that must run without secrets
- the explicit dev-only mock full-stack example

Mock is intentionally not used for:

- the primary OpenAI provider lane
- the OpenAI + WebChat full-stack release lane
- the Telegram-first acceptance lane

## Telegram Release Scope

When Telegram is in release scope, release sign-off is not complete until the Telegram-first runbook is executed and the operator records the saved artifact paths.

That sign-off should capture:

- the session id
- the run trace path
- the incident bundle path
- the webhook info output
- the CLI commands used for verification

For the detailed operator procedure, use [telegram-real-e2e.md](./telegram-real-e2e.md).
