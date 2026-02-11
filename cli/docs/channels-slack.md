# Channels: Slack Webhook (Beta)

This guide covers the `slack_webhook` channel in Mosaic CLI.

## 1) Add channel

```bash
mosaic --project-state channels add \
  --name team-alerts \
  --kind slack_webhook \
  --endpoint https://hooks.slack.com/services/T000/B000/XXXXX
```

Notes:
- Endpoint is fixed at `add` time.
- `send` uses only `channel_id`; no endpoint argument required.
- For `slack_webhook`, endpoint must match `https://hooks.slack.com/services/...`.

## 2) Connectivity test

```bash
mosaic --project-state --json channels test <channel-id>
```

This sends a probe message (`kind=test_probe`) and writes an event log entry without updating business `last_send_at`.

## 3) Send message

```bash
mosaic --project-state --json channels send <channel-id> --text "deploy complete"
```

Response shape (success):

```json
{
  "ok": true,
  "channel_id": "ch_xxx",
  "kind": "message",
  "delivered_via": "slack_webhook",
  "attempts": 1,
  "http_status": 200,
  "endpoint_masked": "https://hooks.slack.com/***XXXX",
  "target_masked": "https://hooks.slack.com/***XXXX",
  "event_path": "/abs/path/.mosaic/data/channel-events/ch_xxx.jsonl"
}
```

Response shape (failure):

```json
{
  "ok": false,
  "error": {
    "code": "network",
    "message": "network error: webhook returned client error status 429",
    "exit_code": 4
  }
}
```

## 4) List channels

```bash
mosaic --project-state --json channels list
```

Each entry includes:
- `kind`
- `endpoint_masked`
- `target_masked`
- `last_send_at`
- `last_error`

## 5) Retry and timeout defaults

- Timeout: 15000ms (override with `MOSAIC_CHANNELS_HTTP_TIMEOUT_MS`)
- Retry: 3 backoff steps (`200ms`, `500ms`, `1000ms`)
- Behavior:
  - `2xx`: success
  - `4xx`: fail immediately (no retry)
  - `5xx` / timeout: retry with exponential backoff

## 6) Auth model

- Channel token values are never persisted.
- Optional token source is environment variable name (`--token-env` or stored `auth.token_env`).
- If `token_env` is provided but not set, command fails with `auth` error.

## 7) Observability

- Channel events path:
  - `.mosaic/data/channel-events/<channel_id>.jsonl` (project mode)
- Event fields:
  - `ts`, `channel_id`, `kind`, `delivery_status`, `attempt`, `http_status`, `error`, `text_preview`
- `doctor` includes channel endpoint validity and token env checks.

## 8) Troubleshooting

- `validation` error on add:
  - endpoint format invalid for selected kind.
- `network` error on send/test:
  - remote endpoint unavailable, timeout, or non-2xx response.
- `auth` error:
  - referenced env var for token is missing.
