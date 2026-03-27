# Providers

This guide explains how provider configuration works in Mosaic.

If you want the i2 no-mock acceptance path, start with:

- [examples/full-stack/openai-webchat.config.yaml](../examples/full-stack/openai-webchat.config.yaml)
- [docs/full-stack.md](./full-stack.md)

## Current provider types

Mosaic validates and runs these provider types:

- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`
- `mock` for explicit dev-only lanes

`openai-compatible` is supported, but it is not a substitute for real acceptance of the first-class vendors above.

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

## Built-in mock provider

`mock` remains available for local smoke tests, but it is no longer the default onboarding path.

```yaml
active_profile: mock
```

Enable it explicitly with:

```bash
mosaic setup init --dev-mock
```

Use `mock` for control-plane regression and local docs smoke tests. Do not use it as onboarding default or release acceptance evidence.

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
