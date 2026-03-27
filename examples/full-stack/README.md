# Full-Stack Examples

These examples bind one provider profile, one Gateway, one channel ingress path, one persisted session, one trace, and one incident flow into a single operator walkthrough.

Primary files:

- `mock-telegram.config.yaml`: local mock provider plus Telegram ingress secret
- `openai-telegram.config.yaml`: OpenAI plus Telegram ingress secret
- `../channels/telegram-update.json`: sample Telegram webhook payload

## Fast local lane

```bash
mosaic setup init
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

Then verify:

```bash
mosaic gateway --attach http://127.0.0.1:18080 status
mosaic session show telegram--100123-99
mosaic inspect .mosaic/runs/<run-id>.json
mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>
```

## Real OpenAI lane

```bash
mosaic setup init
cp examples/full-stack/openai-telegram.config.yaml .mosaic/config.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
mosaic setup validate
mosaic setup doctor
mosaic model list
mosaic gateway serve --http 127.0.0.1:18080
```

Send the same channel payload:

```bash
curl -X POST http://127.0.0.1:18080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

## Automated verification

Mock lane:

```bash
./scripts/test-full-stack-example.sh mock
```

Real OpenAI lane:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai
```

The same flow is documented in:

- [../../docs/full-stack.md](../../docs/full-stack.md)
- [../../docs/channels.md](../../docs/channels.md)
- [../../docs/session-inspect-incident.md](../../docs/session-inspect-incident.md)
