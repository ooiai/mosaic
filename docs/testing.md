# Testing Guide

Mosaic now treats testing as a layered system instead of a single `cargo test` bucket.

The goal of this guide is to make three things explicit:

- which tests are fast and always-on
- which tests are true integration tests
- which tests are gated behind real services, secrets, or local daemons

## Layers

Mosaic uses four test layers.

| Layer | Purpose | Default command | External dependencies |
| --- | --- | --- | --- |
| unit | validate isolated logic and small contracts | `make test-unit` | none |
| local integration | exercise crate public APIs, file IO, local HTTP, subprocesses, and registry wiring | `make test-integration` | local process only |
| real integration | run true provider, gateway, ingress, or daemon paths when explicitly enabled | `MOSAIC_REAL_TESTS=1 make test-real` | secrets, services, or local daemons |
| golden example verification | prove docs and examples still work from setup to inspect | `make test-golden` | local process only |

## Per-Crate Matrix

The matrix below is the source of truth for h3.

| Crate | Unit | Local Integration | Real Integration | Golden / Docs |
| --- | --- | --- | --- | --- |
| `mosaic-config` | inline unit tests | `crates/config/tests/integration_load_validate.rs` | no | `scripts/test-golden-examples.sh` |
| `mosaic-provider` | inline unit tests | `crates/provider/tests/integration_mock_provider.rs` | `crates/provider/tests/real_vendors.rs` | provider examples and setup docs |
| `mosaic-tool-core` | inline unit tests | `crates/tool-core/tests/integration_builtin_tools.rs` | optional via `exec` / `webhook` local services in `make test-real` | example runs |
| `mosaic-skill-core` | inline unit tests | `crates/skill-core/tests/integration_registry.rs` | no | workflow and skill examples |
| `mosaic-workflow` | inline unit tests | `crates/workflow/tests/integration_runner.rs` | no | `examples/workflows/research-brief.yaml` |
| `mosaic-session-core` | inline unit tests | `crates/session-core/tests/integration_file_store.rs` | no | session and inspect flows |
| `mosaic-memory` | inline unit tests | `crates/memory/tests/integration_file_store.rs` | no | runtime session flows |
| `mosaic-inspect` | inline unit tests | `crates/inspect/tests/integration_trace_roundtrip.rs` | no | inspect docs and golden scripts |
| `mosaic-control-protocol` | inline unit tests | `crates/control-protocol/tests/integration_roundtrip.rs` | no | gateway and SDK flows |
| `mosaic-sdk` | inline unit tests | `crates/sdk/tests/integration_client_transport.rs` | `crates/sdk/tests/real_gateway_http.rs` | gateway docs |
| `mosaic-gateway` | inline unit tests | `crates/gateway/tests/integration_local_gateway.rs` | `crates/gateway/tests/real_telegram_ingress.rs` | webchat and telegram examples |
| `mosaic-runtime` | inline unit tests | `crates/runtime/tests/integration_runtime_flow.rs` | optional via real providers and node routing | example runs |
| `mosaic-mcp-core` | inline unit tests | `crates/mcp-core/tests/integration_manager.rs` | `crates/mcp-core/tests/real_stdio_mcp.rs` | `examples/mcp-filesystem.yaml` |
| `mosaic-node-protocol` | inline unit tests | `crates/node-protocol/tests/integration_file_bus.rs` | no | node docs and CLI flows |
| `mosaic-extension-core` | inline unit tests | `crates/extension-core/tests/integration_extension_set.rs` | no | extension examples |
| `mosaic-scheduler-core` | inline unit tests | `crates/scheduler-core/tests/integration_file_store.rs` | no | cron flows |
| `mosaic-channel-telegram` | inline unit tests | `crates/channel-telegram/tests/integration_payloads.rs` | gateway-level real webhook path | telegram ingress docs |
| `mosaic-tui` | inline unit tests | `crates/tui/tests/integration_event_buffer.rs` | no | `mosaic tui` docs |
| `mosaic-cli` | inline unit tests | command-path regression tests in `cli/src/main.rs` | no | `scripts/test-golden-examples.sh` |

## Commands

Fast local loop:

```bash
make test-unit
make test-integration
```

Golden docs and examples:

```bash
make test-golden
```

Real integration lane:

```bash
MOSAIC_REAL_TESTS=1 make test-real
```

Pull-request style lane:

```bash
make ci-fast
```

Operator lane with real secrets and services:

```bash
MOSAIC_REAL_TESTS=1 make ci-real
```

## Environment Gates

All real tests are disabled unless `MOSAIC_REAL_TESTS=1`.

Real tests then opt into additional vendor or channel checks based on the secrets that are present:

| Variable | Meaning |
| --- | --- |
| `MOSAIC_REAL_TESTS` | master switch for real integration tests |
| `OPENAI_API_KEY` | enable OpenAI provider real test |
| `AZURE_OPENAI_API_KEY` | enable Azure provider real test |
| `ANTHROPIC_API_KEY` | enable Anthropic provider real test |
| `MOSAIC_TEST_OPENAI_BASE_URL` | optional OpenAI-compatible override for OpenAI real test |
| `MOSAIC_TEST_OPENAI_MODEL` | optional OpenAI model override for the OpenAI real test |
| `MOSAIC_TEST_AZURE_BASE_URL` | required Azure endpoint for Azure real test |
| `MOSAIC_TEST_AZURE_MODEL` | optional Azure deployment override for the Azure real test |
| `MOSAIC_TEST_ANTHROPIC_BASE_URL` | optional Anthropic base URL override |
| `MOSAIC_TEST_ANTHROPIC_MODEL` | optional Anthropic model override for the Anthropic real test |
| `MOSAIC_TEST_OLLAMA_BASE_URL` | optional Ollama endpoint override, defaults to `http://127.0.0.1:11434` |
| `MOSAIC_TEST_OLLAMA_MODEL` | Ollama model name for real test, defaults to `llama3.1` |
| `MOSAIC_TEST_TELEGRAM_SECRET` | optional shared secret header for the real Telegram ingress test |
| `MOSAIC_OPERATOR_TOKEN` | optional operator auth token for remote gateway SDK tests |

## Secrets and Service Conventions

- Provider secrets are never committed. Real tests read them from environment variables only.
- Vendor-specific base URLs are configurable so the same test can target direct APIs, proxies, or private gateways.
- Local daemons such as Ollama are treated as real integrations when the process is external to the test run.
- MCP real tests spawn an actual subprocess and communicate over stdio instead of mocking the transport.
- Gateway real tests boot an actual HTTP server and consume real SSE frames through the SDK client.

## Golden Examples and Docs

`scripts/test-golden-examples.sh` is the docs/examples gate.

It verifies:

- `mosaic setup init`
- `mosaic setup validate`
- `mosaic setup doctor`
- mock-backed example runs
- workflow example execution
- MCP example execution
- trace inspection after a real run artifact exists

This is the guardrail that keeps `README.md`, `examples/`, and the operator getting-started path runnable.

## CI Strategy

Recommended CI split:

1. `ci-fast`
   Run on every push and pull request.
   Includes `check`, unit tests, local integration tests, and golden examples.

2. `ci-real`
   Run on a scheduled lane, protected branch, or manual dispatch.
   Requires secrets and any local daemons such as Ollama.

3. Provider-specific spot checks
   Run when rotating credentials, changing provider payloads, or upgrading SDK dependencies.

## Flaky Test Policy

- A real test may skip when its required secret or daemon is absent.
- A local integration test must not depend on public internet access.
- If a test is flaky because of timing, prefer longer readiness polling or explicit process health checks over retries with no diagnosis.
- If a real upstream API becomes unstable, quarantine only the affected `real_*` test file and keep `ci-fast` green.
- Do not silently downgrade a real test into a mock test. If the dependency disappears, the test should skip with a clear reason.

## Adding New Tests

When you add a new crate or a new operator surface:

1. add or update a `tests/` integration file under the owning crate
2. decide whether the test is local integration or real integration
3. document any new env or secret in this file
4. wire the command into `scripts/test-real-integrations.sh` or `scripts/test-golden-examples.sh` if it is part of a repo-wide lane
