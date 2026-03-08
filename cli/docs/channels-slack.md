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

## 4.1) Target diagnostics (`capabilities --target`)

```bash
mosaic --project-state --json channels capabilities --target <channel-id>
```

For target-bound queries, each capability includes `diagnostics`:
- `channel_id` / `channel_name`
- `endpoint_configured` / `target_configured`
- `token_env` / `token_present`
- `ready_for_send`
- `issues` (blocking reasons when not ready)

## 4.2) Replay plan for failed deliveries

```bash
mosaic --project-state --json channels replay <channel-id> --tail 50 --limit 5
```

Notes:
- This command builds replay candidates from failed channel events.
- Add `--since-minutes <N>` to only include failures from the recent time window.
- Default mode includes retryable failures only.
- Add `--include-non-retryable` to include auth/config/client failures in the plan.
- Add repeatable `--reason <rate_limited|upstream_5xx|timeout|auth|target_not_found|client_4xx|unknown>` to narrow replay candidates.
- Add repeatable `--http-status <code>` and `--min-attempt <N>` to filter by status code and retry-attempt count.
- Add `--batch-size <N>` to emit grouped replay batches in `batch_plan`.
- Add `--apply` to replay using stored full payload when available (`replay_source=full_payload`).
- `--apply` runs a readiness preflight and blocks early when channel runtime config is not ready (`channels capabilities --target <channel-id>`).
- Add `--max-apply <N>` with `--apply` to cap how many candidates are executed in one run.
- Legacy events without payload fall back to `text_preview` (`replay_source=text_preview_fallback`) and return a warning.
- Add `--require-full-payload` together with `--apply` to hard-stop when any candidate only has `text_preview` fallback.
- Add `--stop-on-error` with `--apply` to stop batch replay immediately after the first failed send.
- Add `--report-out <path>` to persist the replay plan/result JSON for audits or scripts.

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
  - `ts`, `channel_id`, `kind`, `delivery_status`, `attempt`, `http_status`, `error`, `text_preview`, `replay_payload`
- `doctor` includes channel endpoint validity and token env checks.

## 8) Troubleshooting

- `validation` error on add:
  - endpoint format invalid for selected kind.
- `network` error on send/test:
  - remote endpoint unavailable, timeout, or non-2xx response.
- `auth` error:
  - referenced env var for token is missing.
