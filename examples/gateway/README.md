# Gateway Examples

- `webchat-message.json`: legacy sample JSON payload for `POST /ingress/webchat`

Preferred channel payloads now live under:

- [../channels/webchat-message.json](../channels/webchat-message.json)
- [../channels/telegram-update.json](../channels/telegram-update.json)

Use it with a local HTTP Gateway:

```bash
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

For the end-to-end operator walkthrough, continue with [../full-stack/README.md](../full-stack/README.md).
