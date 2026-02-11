# Channels: Telegram Bot (Beta)

This guide covers `telegram_bot` in Mosaic CLI.

## 1) Prepare token env

Default environment variable:

```bash
export MOSAIC_TELEGRAM_BOT_TOKEN="<bot-token>"
```

You can also override with `--token-env <ENV>` at `add/send/test`.

## 2) Add channel

```bash
mosaic --project-state channels add \
  --name tg-alerts \
  --kind telegram_bot \
  --chat-id=-1001234567890
```

Notes:
- `--chat-id` is required and persisted as channel target.
- For negative chat IDs, pass as `--chat-id=-100...` (single argument form).
- Optional `--endpoint` can override Telegram API base URL (default `https://api.telegram.org`).

## 3) Test and send

```bash
mosaic --project-state --json channels test <channel-id>
mosaic --project-state --json channels send <channel-id> \
  --text "deploy complete" \
  --parse-mode markdown_v2 \
  --title "Release Notice" \
  --block "build=42" \
  --metadata '{"env":"staging"}' \
  --idempotency-key release-42
```

Success response includes:
- `delivered_via = "telegram_bot"`
- `target_masked` like `telegram://***7890`
- `endpoint_masked` for API base URL
- `parse_mode` canonicalized (`Markdown`, `MarkdownV2`, `HTML`)
- `deduplicated` indicates idempotency short-circuit
- `rate_limited_ms` reports wait duration before delivery (when throttled)

Capability discovery:

```bash
mosaic --project-state --json channels capabilities --channel telegram_bot
```

Expect:
- `supports_parse_mode = true`
- `supports_message_template = true`
- `supports_idempotency_key = true`
- `supports_rate_limit_report = true`

## 4) Retry behavior

- Timeout: 15000ms (override with `MOSAIC_CHANNELS_HTTP_TIMEOUT_MS`)
- Retry backoff: `200ms`, `500ms`, `1000ms`
- `2xx`: success only when Telegram JSON body has `"ok": true`
- `429`: retry (uses Telegram `parameters.retry_after` when present)
- other `4xx`: fail immediately
- `5xx` / timeout: retry
- Telegram send throttle: `MOSAIC_CHANNELS_TELEGRAM_MIN_INTERVAL_MS` (default `800`)
- Idempotency dedupe window: `MOSAIC_CHANNELS_IDEMPOTENCY_WINDOW_SECONDS` (default `86400`)

## 5) Troubleshooting

- `validation` error on add:
  - missing or empty `--chat-id`
- `auth` error on send/test:
  - token env variable missing
- `validation` error on send:
  - `--parse-mode` used on non-Telegram channels or unsupported parse mode value
- `network` error:
  - transport issue, timeout, or non-success HTTP response
