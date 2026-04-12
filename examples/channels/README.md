# Channel Examples

These files are payloads for the current HTTP ingress adapters and the attachment-aware Telegram lanes added in k1-k4.

- `webchat-message.json`: sample payload for `POST /ingress/webchat` targeting the built-in real-provider-first profile
- `webchat-openai-message.json`: no-mock WebChat payload used by the OpenAI full-stack lane
- `telegram-update.json`: sample Telegram webhook payload for `POST /ingress/telegram`
- `telegram-photo-update.json`: Telegram photo upload payload used to explain image ingress and multimodal routing
- `telegram-document-update.json`: Telegram document upload payload used to explain document ingress and specialized processor routing

Serve a local Gateway first:

```bash
mosaic gateway serve --http 127.0.0.1:8080
```

Webchat:

```bash
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

Webchat with shared-secret auth and the no-mock full-stack payload:

```bash
export MOSAIC_WEBCHAT_SHARED_SECRET=full-stack-secret
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  -H "x-mosaic-shared-secret: $MOSAIC_WEBCHAT_SHARED_SECRET" \
  --data @examples/channels/webchat-openai-message.json
```

Telegram:

```bash
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

Telegram photo upload:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-photo-update.json
```

Telegram document upload:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-document-update.json
```

For a multi-bot Gateway, post to the bot-specific path instead:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram/media \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_MEDIA_SECRET_TOKEN" \
  --data @examples/channels/telegram-photo-update.json
```

If you are new to Telegram bot setup, continue with [../../docs/telegram-step-by-step.md](../../docs/telegram-step-by-step.md).

For the full operator walkthrough, continue with [../full-stack/README.md](../full-stack/README.md).
