# Examples

These examples are organized for operators, not just tests.

Repo-wide example verification commands live in [TESTING.md](./TESTING.md).

## Providers

- [providers/openai.yaml](./providers/openai.yaml): direct OpenAI configuration for `.mosaic/config.yaml`
- [providers/ollama.yaml](./providers/ollama.yaml): local Ollama configuration
- [providers/anthropic.yaml](./providers/anthropic.yaml): direct Anthropic configuration
- [providers/azure.yaml](./providers/azure.yaml): Azure OpenAI configuration
- [full-stack/openai-telegram.config.yaml](./full-stack/openai-telegram.config.yaml): OpenAI profile already wired for the Gateway + Telegram golden path

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

- [channels/webchat-message.json](./channels/webchat-message.json): sample payload for `/ingress/webchat`
- [channels/telegram-update.json](./channels/telegram-update.json): sample payload for `/ingress/telegram`

Use it with a local HTTP Gateway:

```bash
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

## Channels

- [channels/README.md](./channels/README.md): channel payloads and local ingress commands

## Full Stack

- [full-stack/README.md](./full-stack/README.md): the complete provider + Gateway + channel + session + inspect path
- [full-stack/mock-telegram.config.yaml](./full-stack/mock-telegram.config.yaml): fast local Gateway + Telegram config
- [full-stack/openai-telegram.config.yaml](./full-stack/openai-telegram.config.yaml): real OpenAI + Telegram config

Automated verification:

```bash
./scripts/test-full-stack-example.sh mock
```

## Deployment

- [deployment/production.config.yaml](./deployment/production.config.yaml): production-oriented `.mosaic/config.yaml` starter
- [deployment/mosaic.service](./deployment/mosaic.service): example `systemd` unit for an HTTP Gateway
