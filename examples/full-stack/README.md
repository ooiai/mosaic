# Full-Stack Examples

These examples bind one provider profile, one Gateway, one ingress path, one persisted session, one trace, and one incident flow into a single operator walkthrough.

Telegram is the current strongest real interactive GUI acceptance lane while TUI remains incomplete, so Telegram-facing config examples are part of the release-facing operator story rather than optional demos.

Primary files:

- `openai-webchat.config.yaml`: no-mock OpenAI + WebChat acceptance config
- `openai-telegram.config.yaml`: legacy single-bot Telegram sign-off config
- `openai-telegram-single-bot.config.yaml`: beginner-friendly single-bot Telegram baseline with `/mosaic` catalog and attachment routing
- `openai-telegram-e2e.config.yaml`: Telegram-first real acceptance config with extension wiring
- `openai-telegram-multi-bot.config.yaml`: two Telegram bots with isolated webhook paths and profiles
- `openai-telegram-multimodal.config.yaml`: Telegram image/document routing into a multimodal profile
- `openai-telegram-bot-split.config.yaml`: per-bot capability split with a specialized processor document lane
- `../extensions/telegram-e2e.yaml`: fixed manifest for `summarize_notes` and `summarize_operator_note`
- `../channels/webchat-openai-message.json`: no-mock WebChat ingress payload
- `../channels/telegram-update.json`: sample Telegram webhook payload
- `../channels/telegram-photo-update.json`: image upload payload for Telegram docs
- `../channels/telegram-document-update.json`: document upload payload for Telegram docs

## Release-Blocking No-Mock Lane

```bash
mosaic setup init
cp examples/full-stack/openai-webchat.config.yaml .mosaic/config.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_WEBCHAT_SHARED_SECRET=full-stack-secret
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
mosaic gateway serve --http 127.0.0.1:18080
```

In another shell:

```bash
curl -X POST http://127.0.0.1:18080/ingress/webchat \
  -H 'content-type: application/json' \
  -H "x-mosaic-shared-secret: $MOSAIC_WEBCHAT_SHARED_SECRET" \
  --data @examples/channels/webchat-openai-message.json
```

Then verify:

```bash
mosaic gateway --attach http://127.0.0.1:18080 status
mosaic session show full-stack-openai-webchat
mosaic inspect .mosaic/runs/<run-id>.json --verbose
mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>
```

## Telegram Bot Sign-Off Config

```bash
mosaic setup init
cp examples/full-stack/openai-telegram-single-bot.config.yaml .mosaic/config.yaml
cp examples/extensions/telegram-e2e.yaml .mosaic/extensions/telegram-e2e.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_TELEGRAM_BOT_TOKEN=your_real_bot_token
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
mosaic setup validate
mosaic setup doctor
mosaic model list
```

Use this config when you are validating a real Telegram bot webhook outside the default repo automation. When Telegram is a release target, this validation path is part of release sign-off rather than an optional demo.

This baseline is the easiest way to prove:

- `/mosaic help` and category discovery
- Telegram command keyboard discovery through `/start` and `/help`
- plain text Telegram conversation
- explicit tool / skill / workflow routes
- image and document attachment routing
- outbound smoke from `mosaic adapter telegram test-send`

## Multi-Bot and Attachment Examples

Use these configs when the docs call out a more specific Telegram operator story:

- `openai-telegram-multi-bot.config.yaml`: bot A and bot B with separate tokens, secrets, webhook paths, and default profiles
- `openai-telegram-multimodal.config.yaml`: one bot dedicated to image and document uploads through `provider_native`
- `openai-telegram-bot-split.config.yaml`: one support bot and one files bot with a `specialized_processor` document lane

Pair them with:

- `../channels/telegram-photo-update.json`
- `../channels/telegram-document-update.json`
- `../extensions/telegram-e2e.yaml`

## Telegram-First Real Acceptance Lane

```bash
mosaic setup init
cp examples/full-stack/openai-telegram-e2e.config.yaml .mosaic/config.yaml
cp examples/extensions/telegram-e2e.yaml .mosaic/extensions/telegram-e2e.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_TELEGRAM_BOT_TOKEN=your_real_bot_token
export MOSAIC_TELEGRAM_SECRET_TOKEN=your_long_random_secret
export MOSAIC_PUBLIC_WEBHOOK_BASE_URL=https://your-public-host.example.com
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
mosaic extension validate
mosaic gateway serve --http 127.0.0.1:18080
```

Then follow [../../docs/telegram-real-e2e.md](../../docs/telegram-real-e2e.md) for:

- `/start` / `/help` and Telegram command keyboard discovery
- `/mosaic help` and category discovery
- webhook registration
- plain conversation proof
- automatic `time_now` proof
- explicit `read_file` proof
- explicit `summarize_notes` skill proof
- explicit `summarize_operator_note` workflow proof
- image upload proof
- document upload proof
- bot A / bot B isolation proof when multi-bot config is in scope
- session, inspect, audit, replay, and incident verification

## Automated verification

Release-blocking OpenAI + WebChat lane:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

The same flow is documented in:

- [../../docs/full-stack.md](../../docs/full-stack.md)
- [../../docs/telegram-real-e2e.md](../../docs/telegram-real-e2e.md)
- [../../docs/real-vs-mock-acceptance.md](../../docs/real-vs-mock-acceptance.md)
- [../../docs/provider-runtime-policy-matrix.md](../../docs/provider-runtime-policy-matrix.md)
- [../../docs/channels.md](../../docs/channels.md)
- [../../docs/session-inspect-incident.md](../../docs/session-inspect-incident.md)
