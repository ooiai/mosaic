# Channel Setup

This guide covers the current ingress channels and the sample payloads you can use to exercise them.

Examples used by this guide:

- [examples/channels/README.md](../examples/channels/README.md)
- [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)
- [examples/channels/webchat-openai-message.json](../examples/channels/webchat-openai-message.json)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)
- [examples/full-stack/README.md](../examples/full-stack/README.md)

## Current ingress paths

Mosaic currently exposes these HTTP channel entrypoints:

- `POST /ingress/webchat`
- `POST /ingress/telegram`

Both routes are served by:

```bash
mosaic gateway serve --http 127.0.0.1:8080
```

## Webchat

Sample payload:

- [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)
- [examples/channels/webchat-openai-message.json](../examples/channels/webchat-openai-message.json)

Send it:

```bash
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

If `auth.webchat_shared_secret_env` is configured, also send:

```bash
-H "x-mosaic-shared-secret: $MOSAIC_WEBCHAT_SHARED_SECRET"
```

For the release-blocking no-mock lane, use:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

## Telegram

Sample payload:

- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)

Send it:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

The sample payload maps to session id `telegram--100123-99`.

The repo automation covers the real Gateway Telegram ingress path, but a live Telegram bot token and public webhook endpoint remain a manual operator acceptance lane.

## Adapter health checks

Before exposing ingress publicly, check:

```bash
mosaic adapter status
mosaic adapter doctor
```

## Golden path example

If you want the full provider + gateway + channel + inspect walkthrough, continue with:

- [full-stack.md](./full-stack.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [examples/full-stack/README.md](../examples/full-stack/README.md)
