# Channels: Discord Webhook (Beta)

This guide covers the `discord_webhook` channel in Mosaic CLI.

## 1) Add channel

```bash
mosaic --project-state channels add \
  --name team-discord \
  --kind discord_webhook \
  --endpoint https://discord.com/api/webhooks/123456/abcdef
```

Alias forms are also supported:
- `--kind discord`
- `--kind discord-webhook`

## 2) Test connectivity

```bash
mosaic --project-state --json channels test <channel-id>
```

## 3) Send message

```bash
mosaic --project-state --json channels send <channel-id> --text "deploy complete"
```

Discord payload uses `content` internally. CLI response shape is the same as other channels.

## 4) Endpoint rules

Accepted hosts:
- `discord.com`
- `canary.discord.com`
- `ptb.discord.com`
- `discordapp.com`

Endpoint path must include Discord webhook path semantics (`/api/webhooks/...`).

## 5) Retry behavior

- Timeout: 15000ms (override with `MOSAIC_CHANNELS_HTTP_TIMEOUT_MS`)
- Retry backoff: `200ms`, `500ms`, `1000ms`
- `2xx` success
- `4xx` no retry
- `5xx`/timeout retries
