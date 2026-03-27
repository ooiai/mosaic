# Gateway Guide

The Gateway is the control-plane hub for routing, sessions, event broadcast, status, audit, replay, and incident export.

Related examples:

- [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)
- [examples/full-stack/README.md](../examples/full-stack/README.md)

## Local status

```bash
mosaic gateway status
```

This shows:

- health
- readiness
- metrics

## Local event monitor

```bash
mosaic gateway serve --local
```

This prints Gateway event envelopes until you press `Ctrl-C`.

## HTTP control plane

Serve HTTP and SSE locally:

```bash
mosaic gateway serve --http 127.0.0.1:8080
```

Then attach CLI or TUI commands to it:

```bash
mosaic gateway --attach http://127.0.0.1:8080 status
mosaic session --attach http://127.0.0.1:8080 list
mosaic tui --attach http://127.0.0.1:8080 --session remote-demo
```

## Sessions

List Gateway-visible sessions:

```bash
mosaic gateway sessions
```

## Audit trail

Show the latest audit events:

```bash
mosaic gateway audit --limit 20
```

## Replay window

Inspect the recent event replay buffer:

```bash
mosaic gateway replay --limit 20
```

## Incident bundle

Export one incident bundle by run ID, gateway run ID, correlation ID, or saved trace ID:

```bash
mosaic gateway incident <run-id>
```

Write it to a custom path:

```bash
mosaic gateway incident <run-id> --out /tmp/incident.json
```

## Webchat ingress example

Start the HTTP Gateway and send a sample webchat payload.

Sample payload: [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)

```bash
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

If you configure `auth.webchat_shared_secret_env`, also send `x-mosaic-shared-secret`.

## Telegram ingress example

Sample payload: [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)

```bash
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
mosaic gateway serve --http 127.0.0.1:8080
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

## Adapter checks

```bash
mosaic adapter status
mosaic adapter doctor
```

Use these before exposing Gateway ingress to other channels.

For the end-to-end provider + Gateway + Telegram walkthrough, continue with [full-stack.md](./full-stack.md).
