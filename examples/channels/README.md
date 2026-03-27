# Channel Examples

These files are payloads for the current HTTP ingress adapters.

- `webchat-message.json`: sample payload for `POST /ingress/webchat`
- `telegram-update.json`: sample Telegram webhook payload for `POST /ingress/telegram`

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

Telegram:

```bash
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

For the full operator walkthrough, continue with [../full-stack/README.md](../full-stack/README.md).
