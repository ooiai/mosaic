# Providers

This guide explains how provider configuration works in Mosaic today.

## Current provider types

Mosaic currently validates and runs these provider types:

- `mock`
- `openai-compatible`

If you set another value, `mosaic setup validate` will fail.

## OpenAI

Use the OpenAI API directly through the `openai-compatible` provider type.

Example: [examples/providers/openai.yaml](../examples/providers/openai.yaml)

```yaml
active_profile: openai
profiles:
  openai:
    type: openai-compatible
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
```

Required environment variable:

```bash
export OPENAI_API_KEY=your_api_key_here
```

## Ollama

Ollama works through its OpenAI-compatible endpoint.

Example: [examples/providers/ollama.yaml](../examples/providers/ollama.yaml)

```yaml
active_profile: ollama
profiles:
  ollama:
    type: openai-compatible
    model: llama3.2
    base_url: http://127.0.0.1:11434/v1
    api_key_env: OLLAMA_API_KEY
```

If your local Ollama endpoint does not require auth, validation still expects `api_key_env`, so set a placeholder:

```bash
export OLLAMA_API_KEY=ollama
```

## Anthropic

Mosaic does not yet provide a native `anthropic` provider type. If you want to run Anthropic models today, put an OpenAI-compatible bridge or proxy in front of Anthropic and configure Mosaic against that endpoint.

Example: [examples/providers/anthropic.yaml](../examples/providers/anthropic.yaml)

```yaml
active_profile: anthropic-proxy
profiles:
  anthropic-proxy:
    type: openai-compatible
    model: claude-sonnet-4-5
    base_url: http://127.0.0.1:4000/v1
    api_key_env: ANTHROPIC_API_KEY
```

Required environment variable:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
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
