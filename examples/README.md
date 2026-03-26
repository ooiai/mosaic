# Examples

These examples are organized for operators, not just tests.

## Providers

- [providers/openai.yaml](./providers/openai.yaml): direct OpenAI configuration for `.mosaic/config.yaml`
- [providers/ollama.yaml](./providers/ollama.yaml): local Ollama through its OpenAI-compatible endpoint
- [providers/anthropic.yaml](./providers/anthropic.yaml): Anthropic through an OpenAI-compatible bridge or proxy

Use them by copying the relevant block into `.mosaic/config.yaml`, then run:

```bash
mosaic setup validate
mosaic setup doctor
```

## Workflows

- [workflows/research-brief.yaml](./workflows/research-brief.yaml): runnable mock workflow example

Run it:

```bash
mosaic run examples/workflows/research-brief.yaml --workflow research_brief --session workflow-demo
```

## Extensions

- [extensions/time-and-summary.yaml](./extensions/time-and-summary.yaml): extension manifest with one manifest skill and one workflow

Validate it by referencing it from `.mosaic/config.yaml`:

```yaml
extensions:
  manifests:
    - path: examples/extensions/time-and-summary.yaml
      version_pin: 0.1.0
      enabled: true
```

Then run:

```bash
mosaic extension validate
mosaic extension list
```

## Gateway

- [gateway/webchat-message.json](./gateway/webchat-message.json): sample payload for `/ingress/webchat`

Use it with a local HTTP Gateway:

```bash
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/webchat   -H 'content-type: application/json'   --data @examples/gateway/webchat-message.json
```
