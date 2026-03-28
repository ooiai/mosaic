# Full-Stack Examples

These examples bind one provider profile, one Gateway, one ingress path, one persisted session, one trace, and one incident flow into a single operator walkthrough.

Primary files:

- `openai-webchat.config.yaml`: no-mock OpenAI + WebChat acceptance config
- `mock-telegram.config.yaml`: local dev-only mock provider plus Telegram ingress secret
- `openai-telegram.config.yaml`: OpenAI plus Telegram ingress secret for Telegram bot sign-off
- `openai-telegram-e2e.config.yaml`: Telegram-first real acceptance config with extension wiring
- `../extensions/telegram-e2e.yaml`: fixed manifest for `summarize_notes` and `summarize_operator_note`
- `../channels/webchat-openai-message.json`: no-mock WebChat ingress payload
- `../channels/telegram-update.json`: sample Telegram webhook payload

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

## Fast Local Dev Lane

```bash
mosaic setup init --dev-mock
cp examples/full-stack/mock-telegram.config.yaml .mosaic/config.yaml
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
mosaic setup validate
mosaic setup doctor
mosaic gateway serve --http 127.0.0.1:18080
```

In another shell:

```bash
curl -X POST http://127.0.0.1:18080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

## Telegram Bot Sign-Off Config

```bash
mosaic setup init
cp examples/full-stack/openai-telegram.config.yaml .mosaic/config.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
mosaic setup validate
mosaic setup doctor
mosaic model list
```

Use this config when you are validating a real Telegram bot webhook outside the default repo automation.

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

- webhook registration
- plain conversation proof
- automatic `time_now` proof
- explicit `read_file` proof
- explicit `summarize_notes` skill proof
- explicit `summarize_operator_note` workflow proof
- session, inspect, audit, replay, and incident verification

## Automated verification

Dev-only mock lane:

```bash
./scripts/test-full-stack-example.sh mock
```

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
