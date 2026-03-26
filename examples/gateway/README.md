# Gateway Examples

- `webchat-message.json`: sample JSON payload for `POST /ingress/webchat`

Use it with a local HTTP Gateway:

```bash
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/webchat   -H 'content-type: application/json'   --data @examples/gateway/webchat-message.json
```
