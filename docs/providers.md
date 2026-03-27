# Providers

This guide explains how provider configuration works in Mosaic today.

If you want one provider profile that already matches the full Gateway + Telegram example, use:

- [examples/full-stack/mock-telegram.config.yaml](../examples/full-stack/mock-telegram.config.yaml)
- [examples/full-stack/openai-telegram.config.yaml](../examples/full-stack/openai-telegram.config.yaml)

## Current provider types

Mosaic currently validates and runs these provider types:

- `mock`
- `openai`
- `azure`
- `anthropic`
- `ollama`
- `openai-compatible`

If you set another value, `mosaic setup validate` will fail.

## OpenAI

Use the OpenAI API directly through the `openai` provider type.

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

Full-stack example using the same provider:

- [examples/full-stack/openai-telegram.config.yaml](../examples/full-stack/openai-telegram.config.yaml)
- [docs/full-stack.md](./full-stack.md)

## Azure OpenAI

Use the native `azure` provider type when you have an Azure OpenAI deployment.

Example: [examples/providers/azure.yaml](../examples/providers/azure.yaml)

```yaml
active_profile: azure
profiles:
  azure:
    type: azure
    model: gpt-5.4
    base_url: https://your-resource.openai.azure.com
    api_key_env: AZURE_OPENAI_API_KEY
```

Required environment variable:

```bash
export AZURE_OPENAI_API_KEY=your_api_key_here
```

## Ollama

Ollama works through the native `ollama` provider type.

Example: [examples/providers/ollama.yaml](../examples/providers/ollama.yaml)

```yaml
active_profile: ollama
profiles:
  ollama:
    type: ollama
    model: qwen3:14b
    base_url: http://127.0.0.1:11434
```

## Anthropic

Mosaic ships a native `anthropic` provider type.

Example: [examples/providers/anthropic.yaml](../examples/providers/anthropic.yaml)

```yaml
active_profile: anthropic
profiles:
  anthropic:
    type: anthropic
    model: claude-sonnet-4-5
    base_url: https://api.anthropic.com/v1
    api_key_env: ANTHROPIC_API_KEY
```

Required environment variable:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

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

The generated config starts on `mock` so you can explore the CLI and TUI without credentials.

```yaml
active_profile: mock
```

Use `mock` when you want to test control-plane flows, not real model quality.

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
```

If validation fails, read [troubleshooting.md](./troubleshooting.md).

For secret handling and production auth guidance, continue with [security.md](./security.md).

For the full provider + Gateway + channel walkthrough, continue with [full-stack.md](./full-stack.md).
