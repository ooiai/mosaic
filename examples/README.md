# Examples

These examples are organized for operators, not just tests.

Repo-wide example verification commands live in [TESTING.md](./TESTING.md).

## Providers

- [providers/openai.yaml](./providers/openai.yaml): direct OpenAI configuration for `.mosaic/config.yaml`
- [providers/ollama.yaml](./providers/ollama.yaml): local Ollama configuration
- [providers/anthropic.yaml](./providers/anthropic.yaml): direct Anthropic configuration
- [providers/azure.yaml](./providers/azure.yaml): Azure OpenAI configuration
- [full-stack/openai-webchat.config.yaml](./full-stack/openai-webchat.config.yaml): OpenAI profile already wired for the no-mock full-stack path

Use them by copying the relevant block into `.mosaic/config.yaml`, then run:

```bash
mosaic setup validate
mosaic setup doctor
```

## Workflows

- [workflows/research-brief.yaml](./workflows/research-brief.yaml): runnable real-provider-first workflow example

Run it:

```bash
mosaic run examples/workflows/research-brief.yaml --workflow research_brief --session workflow-demo
```

## Extensions

- [extensions/time-and-summary.yaml](./extensions/time-and-summary.yaml): extension manifest with one manifest skill and one workflow
- [extensions/telegram-e2e.yaml](./extensions/telegram-e2e.yaml): Telegram-first manifest with attachment-aware `summarize_notes` and `summarize_operator_note`
- [extensions/markdown-skill-pack.yaml](./extensions/markdown-skill-pack.yaml): extension manifest that registers a `SKILL.md` markdown skill pack

## Skills

- [skills/README.md](./skills/README.md): native, manifest, and markdown skill examples
- [skills/native-skill.yaml](./skills/native-skill.yaml): builtin native skill example
- [skills/manifest-skill.yaml](./skills/manifest-skill.yaml): declarative manifest skill example
- [skills/operator-note/SKILL.md](./skills/operator-note/SKILL.md): minimal markdown skill pack example

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

- [channels/webchat-message.json](./channels/webchat-message.json): sample payload for `/ingress/webchat` using the built-in real-provider-first profile name
- [channels/webchat-openai-message.json](./channels/webchat-openai-message.json): release-grade payload for `/ingress/webchat`
- [channels/telegram-update.json](./channels/telegram-update.json): sample payload for `/ingress/telegram`
- [channels/telegram-photo-update.json](./channels/telegram-photo-update.json): Telegram image upload payload for multimodal and attachment-aware docs
- [channels/telegram-document-update.json](./channels/telegram-document-update.json): Telegram document upload payload for attachment and specialized processor docs

Use it with a local HTTP Gateway:

```bash
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

## Channels

- [channels/README.md](./channels/README.md): channel payloads and local ingress commands

## Capabilities

- [capabilities/README.md](./capabilities/README.md): builtin tool, MCP tool, node-routed capability, and workflow taxonomy examples
- [capabilities/builtin-tool.yaml](./capabilities/builtin-tool.yaml): builtin tool exposure example
- [capabilities/node-routed-tool.yaml](./capabilities/node-routed-tool.yaml): node-preferred tool metadata example
- [capabilities/workflow.yaml](./capabilities/workflow.yaml): workflow example that composes a manifest skill

## Sandbox

- [sandbox/README.md](./sandbox/README.md): workspace-local sandbox env examples
- [sandbox/python-markdown-skill-pack.yaml](./sandbox/python-markdown-skill-pack.yaml): Python sandbox binding for a markdown skill pack
- [sandbox/node-manifest-skill.yaml](./sandbox/node-manifest-skill.yaml): Node sandbox binding for a manifest skill

## Composition

- [composition/README.md](./composition/README.md): capability composition walkthrough
- [composition/openai-capability-composition.config.yaml](./composition/openai-capability-composition.config.yaml): config + extension + MCP + markdown skill pack composition example

## Full Stack

- [full-stack/README.md](./full-stack/README.md): the complete provider + Gateway + channel + session + inspect path
- [full-stack/openai-webchat.config.yaml](./full-stack/openai-webchat.config.yaml): release-blocking OpenAI + WebChat config
- [full-stack/openai-telegram.config.yaml](./full-stack/openai-telegram.config.yaml): manual Telegram bot sign-off config
- [full-stack/openai-telegram-single-bot.config.yaml](./full-stack/openai-telegram-single-bot.config.yaml): single-bot Telegram baseline with `/mosaic` catalog and attachment routing
- [full-stack/openai-telegram-e2e.config.yaml](./full-stack/openai-telegram-e2e.config.yaml): Telegram-first real acceptance config with extension wiring
- [full-stack/openai-telegram-multi-bot.config.yaml](./full-stack/openai-telegram-multi-bot.config.yaml): multi-bot Telegram routing example
- [full-stack/openai-telegram-multimodal.config.yaml](./full-stack/openai-telegram-multimodal.config.yaml): Telegram image/document multimodal example
- [full-stack/openai-telegram-bot-split.config.yaml](./full-stack/openai-telegram-bot-split.config.yaml): bot-specific capability and specialized processor split example

Automated verification:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

## Deployment

- [deployment/production.config.yaml](./deployment/production.config.yaml): production-oriented `.mosaic/config.yaml` starter
- [deployment/mosaic.service](./deployment/mosaic.service): example `systemd` unit for an HTTP Gateway
