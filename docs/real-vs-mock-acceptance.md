# Real vs Mock Acceptance Matrix

This document is the source of truth for i2 acceptance.

Mosaic keeps mock paths for fast local iteration, but mock paths are not stage-completion evidence.

## Rule

- Mock paths are allowed for fast regression and docs smoke tests.
- Mock paths are not release-blocking acceptance.
- Release-blocking acceptance must use a real provider, a real protocol surface, and real saved artifacts.
- Manual operator acceptance may still be required for channels that depend on public callbacks or third-party bot infrastructure.

## Matrix

| Surface | Path | Classification | Command / Artifact | Notes |
| --- | --- | --- | --- | --- |
| provider smoke | `mock` provider | dev-only | `cargo test -p mosaic-provider --test integration_mock_provider` | Keeps fast local regression coverage |
| provider vendors | OpenAI / Azure / Anthropic / Ollama | release-blocking real | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | Requires secrets or local Ollama |
| provider payload shaping | local HTTP capture servers | local integration | `cargo test -p mosaic-provider --lib` | Validates protocol formatting, not upstream behavior |
| gateway protocol | HTTP + SSE through SDK | release-blocking real | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture` | Real server, real port, real client |
| MCP transport | stdio subprocess manager | release-blocking real | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture` | Real subprocess, not mocked transport |
| webchat ingress | `POST /ingress/webchat` | release-blocking real | `MOSAIC_REAL_TESTS=1 ./scripts/test-full-stack-example.sh openai-webchat` | Primary no-mock full-stack lane |
| telegram ingress | local webhook path with normalized update | real protocol, not final external acceptance | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-gateway --test real_telegram_ingress -- --nocapture` | Real HTTP handler and persisted session, but not a live bot webhook |
| telegram bot delivery | bot token + public webhook endpoint | manual real acceptance | operator runbook | Required before shipping Telegram as a production ingress |
| full-stack example | mock provider + Telegram payload | dev-only golden path | `./scripts/test-full-stack-example.sh mock` | Keeps docs/examples runnable without secrets |
| full-stack example | OpenAI + WebChat ingress | release-blocking real | `MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat` | Setup -> gateway -> ingress -> session -> inspect -> incident |

## Release Gate

The release-blocking real lane for i2 is:

```bash
MOSAIC_REAL_TESTS=1 make test-real
```

That lane must prove:

- at least one first-class provider succeeds without mock data
- Gateway HTTP and SSE work through the SDK
- MCP works over a real stdio transport
- the no-mock full-stack WebChat lane writes session, trace, audit, replay, and incident artifacts

## Manual Acceptance

The following are intentionally outside the default automated gate:

- live Telegram bot webhook validation
- externally routed public webhook infrastructure
- vendor accounts that are unavailable in the current environment

When one of these surfaces is a release target, document the operator runbook and record the artifact path used for acceptance.
