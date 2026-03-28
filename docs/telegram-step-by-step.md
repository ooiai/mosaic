# Telegram Step-by-Step Setup

This guide is the beginner path for connecting Mosaic to a real Telegram bot.

Use this when you want the exact order of operations:

1. create the bot
2. get the Telegram bot token
3. prepare the Mosaic workspace
4. start the Gateway
5. register the webhook
6. send the first real Telegram message
7. verify inbound and outbound behavior from the CLI

If you already know how Telegram bots and webhooks work and you only need the formal release sign-off path, go to [telegram-real-e2e.md](./telegram-real-e2e.md).

Official Telegram references:

- [BotFather](https://t.me/BotFather)
- [Telegram tutorial: obtain your bot token](https://core.telegram.org/bots/tutorial#obtain-your-bot-token)
- [Telegram Bot API: setWebhook](https://core.telegram.org/bots/api#setwebhook)
- [Telegram Bot API: getWebhookInfo](https://core.telegram.org/bots/api#getwebhookinfo)
- [Telegram Bot API: deleteWebhook](https://core.telegram.org/bots/api#deletewebhook)

## Before You Start

You need:

- a Telegram account
- Mosaic installed locally
- one real provider credential, usually `OPENAI_API_KEY`
- a public HTTPS URL that can forward traffic to your local or remote Mosaic Gateway

Important:

- Mosaic serves local HTTP with `mosaic gateway serve --http ...`
- Telegram requires a public HTTPS webhook URL
- so you need a reverse proxy or tunnel in front of Mosaic

Any setup that gives you a public URL like `https://your-host.example.com` and forwards it to `http://127.0.0.1:18080` is acceptable.

The quickest beginner options are:

- `cloudflared`
- `ngrok`

Those are covered explicitly below.

## Step 1: Create a Telegram Bot and Get the Token

Open Telegram and chat with [@BotFather](https://t.me/BotFather).

Then:

1. Send `/newbot`
2. Enter a display name for the bot
3. Enter a username for the bot

Rules for the username:

- it must be unique
- it must end with `bot` or `_bot`

After BotFather creates the bot, it will return a token that looks like this:

```text
1234567890:AAExampleTokenGoesHere
```

Save that token immediately. Treat it like a password.

If you later lose the token:

- go back to BotFather
- choose the bot
- use `/token` to show the current token
- or `/revoke` if you think it was leaked

## Step 2: Prepare the Mosaic Workspace

From the Mosaic repo root:

```bash
mosaic setup init
cp examples/full-stack/openai-telegram-e2e.config.yaml .mosaic/config.yaml
cp examples/extensions/telegram-e2e.yaml .mosaic/extensions/telegram-e2e.yaml
```

This gives you:

- a real-provider-first Telegram config
- one manifest skill: `summarize_notes`
- one workflow: `summarize_operator_note`

If your runtime workspace is outside the repo, copy those two files into the matching `.mosaic/` paths manually.

## Step 3: Set the Required Environment Variables

Export the provider key and the Telegram settings.

```bash
export OPENAI_API_KEY=your_openai_api_key
export MOSAIC_TELEGRAM_BOT_TOKEN=your_botfather_token
export MOSAIC_PUBLIC_WEBHOOK_BASE_URL=https://your-public-host.example.com
export MOSAIC_TELEGRAM_SECRET_TOKEN=$(openssl rand -hex 32)
```

What they mean:

- `OPENAI_API_KEY`: the model provider key
- `MOSAIC_TELEGRAM_BOT_TOKEN`: the token from BotFather
- `MOSAIC_PUBLIC_WEBHOOK_BASE_URL`: the public HTTPS base URL Telegram will call
- `MOSAIC_TELEGRAM_SECRET_TOKEN`: the shared secret Mosaic expects on inbound Telegram webhook requests

If you do not have `openssl`, set the secret manually to a long random string.

## Step 4: Validate the Workspace Before Touching Telegram

Run:

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
mosaic extension validate
mosaic extension list
mosaic adapter status
mosaic adapter doctor
```

What success should look like:

- the active profile is `openai`
- `telegram` appears in adapter output
- outbound readiness is true
- `telegram-e2e` appears in extension output
- `summarize_notes` and `summarize_operator_note` are visible

If this step is red, do not continue to webhook registration yet.

## Step 5: Start the Gateway

Start Mosaic on a stable local port:

```bash
mosaic gateway serve --http 127.0.0.1:18080
```

Leave this process running.

Your reverse proxy or tunnel must forward:

```text
https://your-public-host.example.com/ingress/telegram
```

to:

```text
http://127.0.0.1:18080/ingress/telegram
```

## Step 5A: Quick HTTPS URL with `cloudflared`

If you want the fastest local test setup, you can start a temporary Cloudflare Quick Tunnel.

Official reference:

- Cloudflare Quick Tunnels: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/
- Cloudflare Tunnel setup: https://developers.cloudflare.com/tunnel/setup/

Start the tunnel:

```bash
cloudflared tunnel --url http://localhost:18080
```

`cloudflared` will print a public URL ending in `trycloudflare.com`, for example:

```text
https://random-name.trycloudflare.com
```

Then export:

```bash
export MOSAIC_PUBLIC_WEBHOOK_BASE_URL=https://random-name.trycloudflare.com
```

Notes:

- this is good for testing and demos
- the URL changes every time you restart the quick tunnel
- Cloudflare documents Quick Tunnels as a development feature rather than a production setup

## Step 5B: Quick HTTPS URL with `ngrok`

If you prefer `ngrok`, start an HTTP endpoint that forwards to your local Mosaic Gateway.

Official reference:

- ngrok webhook guide: https://ngrok.com/docs/guides/share-localhost/webhooks
- ngrok HTTP endpoints: https://ngrok.com/docs/http

Start the tunnel:

```bash
ngrok http 18080
```

`ngrok` will print a public HTTPS forwarding URL, for example:

```text
https://example.ngrok.app
```

Then export:

```bash
export MOSAIC_PUBLIC_WEBHOOK_BASE_URL=https://example.ngrok.app
```

Notes:

- this is good for webhook testing and local iteration
- the URL may change unless your ngrok account is configured with a stable domain
- after `ngrok` is running, Telegram should be pointed at `${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram`

## Step 6: Register the Telegram Webhook

Once the public HTTPS URL is working, register the webhook from Mosaic CLI:

```bash
mosaic adapter telegram webhook set \
  --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram" \
  --drop-pending-updates
```

Then verify it:

```bash
mosaic adapter telegram webhook info
```

Check for:

- the expected public URL
- no current Telegram delivery error
- no obvious mismatch in secret or target path

If you need to remove it later:

```bash
mosaic adapter telegram webhook delete --drop-pending-updates
```

## Step 7: Send the First Real Telegram Message

In the Telegram app:

1. search for your bot username
2. open the chat
3. press `Start`
4. send a simple message such as:

```text
Hello Mosaic. Reply with one short sentence confirming this came from Telegram.
```

Expected result:

- Telegram sends the message to your webhook
- Mosaic creates a session
- Mosaic runs the provider
- the reply returns to the same Telegram chat

## Step 8: Verify That Mosaic Saved the Run

In another terminal, run:

```bash
mosaic gateway status
mosaic gateway audit --limit 20
mosaic gateway replay --limit 20
mosaic session list
```

Then inspect the new session:

```bash
mosaic session show <session-id>
```

For Telegram group topics, the session id often looks like:

```text
telegram--100123-99
```

Also inspect the latest trace:

```bash
TRACE_PATH=$(ls -t .mosaic/runs/*.json | head -n 1)
mosaic inspect "$TRACE_PATH" --verbose
```

If you need the incident bundle:

```bash
RUN_ID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["run_id"])' "$TRACE_PATH")
mosaic gateway incident "$RUN_ID"
```

## Step 9: Find the `chat_id` for Outbound Smoke Tests

`mosaic adapter telegram test-send` needs a real Telegram `chat_id`.

The easiest way to get it is:

1. send the first inbound Telegram message successfully
2. inspect the resulting session or trace
3. read the `conversation_id`

You will usually see something like:

```text
conversation_id: telegram:chat:123456789
```

In that example, the `chat_id` is `123456789`.

For a supergroup, the value is often negative, for example:

```text
telegram:chat:-1001234567890
```

## Step 10: Run an Outbound Smoke Test

After you know the `chat_id`, ask Mosaic to send a direct outbound Telegram message:

```bash
mosaic adapter telegram test-send --chat-id 123456789 "hello from mosaic"
```

If you are using a forum topic or thread, add `--thread-id`:

```bash
mosaic adapter telegram test-send --chat-id -1001234567890 --thread-id 99 "hello from mosaic"
```

Expected result:

- `status: delivered`
- a non-empty `provider_message_id`
- the message appears in Telegram

## Step 11: Prove Tool, Skill, and Workflow Paths

After plain chat works, test the capability routes from Telegram.

### Builtin tool

Send this in Telegram:

```text
/mosaic tool read_file .mosaic/config.yaml
```

### Manifest skill

Send this in Telegram:

```text
/mosaic skill summarize_notes 这里是一些运营笔记：今天修复了 webhook，模型切换正常，session 和 trace 都能看到。
```

### Workflow

Send this in Telegram:

```text
/mosaic workflow summarize_operator_note 这里是一段需要走 workflow 的说明：今天验证了 Telegram ingress、outbound reply 和 inspect。
```

Then verify each run again with:

```bash
mosaic gateway audit --limit 20
mosaic session show <session-id>
mosaic inspect "$TRACE_PATH" --verbose
```

## Step 12: Common Problems

### Telegram webhook registered, but inbound messages never arrive

Check:

- the public URL is really HTTPS
- the reverse proxy or tunnel really forwards to `127.0.0.1:18080`
- `mosaic adapter telegram webhook info` shows the same URL you expect
- `MOSAIC_TELEGRAM_SECRET_TOKEN` matches the workspace config expectation

### Mosaic receives inbound messages, but no reply is sent

Check:

- `mosaic adapter status`
- `mosaic adapter doctor`
- `OPENAI_API_KEY`
- `MOSAIC_TELEGRAM_BOT_TOKEN`
- `mosaic inspect "$TRACE_PATH" --verbose`

### `test-send` fails

Check:

- the `chat_id` is correct
- the bot has already been opened by that user or group
- the thread id is correct if you are using a topic
- the bot token is valid

### I changed my public URL

Run:

```bash
mosaic adapter telegram webhook set \
  --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram" \
  --drop-pending-updates
```

Then verify again with:

```bash
mosaic adapter telegram webhook info
```

## Step 13: Clean Up

When you are done testing:

```bash
mosaic adapter telegram webhook delete --drop-pending-updates
```

Your saved artifacts remain in:

- `.mosaic/sessions/`
- `.mosaic/runs/`
- `.mosaic/audit/`

## What To Read Next

- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [channels.md](./channels.md)
- [session-inspect-incident.md](./session-inspect-incident.md)
- [testing.md](./testing.md)
