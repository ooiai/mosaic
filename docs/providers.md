# Providers

This guide explains how provider configuration works in Mosaic and how each first-class provider is proven in the real test matrix.

If you want the i2 no-mock acceptance path, start with:

- [examples/full-stack/openai-webchat.config.yaml](../examples/full-stack/openai-webchat.config.yaml)
- [docs/full-stack.md](./full-stack.md)
- [docs/testing.md](./testing.md)

## Current provider types

Mosaic validates and runs these provider types:

- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`

`openai-compatible` is supported, but it is not a substitute for real acceptance of the first-class vendors above.

## Vendor Real Proof Lanes

This is the provider addendum matrix for j5.

| Provider | Primary role | Real proof lane | Release role |
| --- | --- | --- | --- |
| OpenAI | default operator provider and Telegram-first default provider | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` and `MOSAIC_REAL_TESTS=1 ./scripts/test-full-stack-example.sh openai-webchat` | automated release-blocking |
| Azure OpenAI | first-class vendor compatibility | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | compatibility addendum |
| Anthropic | first-class vendor compatibility | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | compatibility addendum |
| Ollama | local real-model compatibility | `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture` | compatibility addendum |

OpenAI is the default provider for the automated no-mock full-stack lane and for the Telegram-first acceptance runbook.

## OpenAI

Example: [examples/providers/openai.yaml](../examples/providers/openai.yaml)

```yaml
active_profile: openai
profiles:
  openai:
    type: openai
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

Required environment variable:

```bash
export OPENAI_API_KEY=your_api_key_here
```

Full-stack no-mock example:

- [examples/full-stack/openai-webchat.config.yaml](../examples/full-stack/openai-webchat.config.yaml)
- [docs/telegram-real-e2e.md](./telegram-real-e2e.md) when Telegram is in scope

## Azure OpenAI

Example: [examples/providers/azure.yaml](../examples/providers/azure.yaml)

```yaml
active_profile: azure
profiles:
  azure:
    type: azure
    model: gpt-5.4
    base_url: https://your-resource.openai.azure.com
    api_key_env: AZURE_OPENAI_API_KEY
    vendor:
      azure_api_version: 2024-10-21
```

Required environment variable:

```bash
export AZURE_OPENAI_API_KEY=your_api_key_here
```

## Anthropic

Example: [examples/providers/anthropic.yaml](../examples/providers/anthropic.yaml)

```yaml
active_profile: anthropic
profiles:
  anthropic:
    type: anthropic
    model: claude-sonnet-4-5
    base_url: https://api.anthropic.com/v1
    api_key_env: ANTHROPIC_API_KEY
    vendor:
      anthropic_version: 2023-06-01
```

Required environment variable:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

## Ollama

Example: [examples/providers/ollama.yaml](../examples/providers/ollama.yaml)

```yaml
active_profile: ollama
profiles:
  ollama:
    type: ollama
    model: qwen3:14b
    base_url: http://127.0.0.1:11434
```

Ollama is treated as a real provider when the daemon is external to the test process.

## OpenAI-compatible

Use `openai-compatible` only when you are targeting a compatible proxy or a vendor endpoint that is not one of the native providers above.

```yaml
active_profile: gateway-bridge
profiles:
  gateway-bridge:
    type: openai-compatible
    model: custom-model
    base_url: https://your-gateway.example/v1
    api_key_env: COMPAT_API_KEY
```

## Transport and Vendor Policy

The operator-visible provider knobs are:

```yaml
provider_defaults:
  timeout_ms: 45000
  max_retries: 2
  retry_backoff_ms: 250

profiles:
  openai:
    type: openai
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
    transport:
      timeout_ms: 60000
      max_retries: 3
      retry_backoff_ms: 300
    vendor:
      allow_custom_headers: false
```

See [provider-runtime-policy-matrix.md](./provider-runtime-policy-matrix.md) for the complete matrix.

## Switching models

List configured profiles:

```bash
mosaic model list
```

Switch the active profile in the current workspace:

```bash
mosaic model use openai
```

Override the profile for one run:

```bash
mosaic run examples/time-now-agent.yaml --profile openai
```

Or start the TUI on a specific profile:

```bash
mosaic tui --profile openai
```

## Validation checklist

After any provider change:

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
```

For effective per-run values:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

For the no-mock full provider + Gateway + ingress walkthrough, continue with [full-stack.md](./full-stack.md).

For provider release roles and acceptance scope, continue with:

- [testing.md](./testing.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [release.md](./release.md)
