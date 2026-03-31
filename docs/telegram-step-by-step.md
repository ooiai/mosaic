# Telegram Step-by-Step Setup

This guide is the beginner path for connecting Mosaic to a real Telegram bot.

Important baseline:

- this Telegram guide does not require `mosaic node serve`
- local tools such as `read_file` work without a node
- nodes are optional and only matter when you intentionally want device-local execution

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
cp examples/full-stack/openai-telegram-single-bot.config.yaml .mosaic/config.yaml
cp examples/extensions/telegram-e2e.yaml .mosaic/extensions/telegram-e2e.yaml
```

This gives you:

- a real-provider-first Telegram config
- one bot-aware Telegram baseline
- one attachment-aware manifest skill: `summarize_notes`
- one attachment-aware workflow: `summarize_operator_note`

If your runtime workspace is outside the repo, copy those two files into the matching `.mosaic/` paths manually.

If you want the stricter release runbook later, the acceptance workspace lives at [examples/full-stack/openai-telegram-e2e.config.yaml](../examples/full-stack/openai-telegram-e2e.config.yaml).

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

You do not need to start a node in this step.

## Step 5: Start the Gateway

Start Mosaic on a stable local port:

```bash
mosaic gateway serve --http 127.0.0.1:18080
```

Leave this process running.

Your reverse proxy or tunnel must forward:

```text
https://your-public-host.example.com/ingress/telegram/primary
```

to:

```text
http://127.0.0.1:18080/ingress/telegram/primary
```

If you are still using the legacy single-bot path instead of the bot registry, the inbound path is `/ingress/telegram`.

## Step 5A: Quick HTTPS URL with `cloudflared`

If you want the fastest local test setup, you can start a temporary Cloudflare Quick Tunnel.

Official reference:

- Cloudflare Quick Tunnels: <https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/>
- Cloudflare Tunnel setup: <https://developers.cloudflare.com/tunnel/setup/>

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

## Step 5B: Quick HTTPS URL with `ngrok`

If you prefer `ngrok`, start an HTTP endpoint that forwards to your local Mosaic Gateway.

Official reference:

- ngrok webhook guide: <https://ngrok.com/docs/guides/share-localhost/webhooks>
- ngrok HTTP endpoints: <https://ngrok.com/docs/http>

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

## Step 6: Register the Telegram Webhook

Once the public HTTPS URL is working, register the webhook from Mosaic CLI:

```bash
mosaic adapter telegram webhook set --bot primary --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/primary" --secret-token "$MOSAIC_TELEGRAM_SECRET_TOKEN" --drop-pending-updates

mosaic adapter telegram webhook set \
  --bot primary \
  --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/primary" \
  --secret-token "$MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --drop-pending-updates
```

Then verify it:

```bash
mosaic adapter telegram webhook info --bot primary
```

Check for:

- the expected public URL
- no current Telegram delivery error
- no obvious mismatch in secret or target path

If you need to remove it later:

```bash
mosaic adapter telegram webhook delete --bot primary --drop-pending-updates
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

## Step 8: Discover the Channel Command Catalog

After plain chat works, ask the bot what it can do:

```text
/mosaic help
/mosaic help tools
/mosaic help skills
/mosaic session status
```

This proves the dynamic channel command catalog is live for the current bot and channel.

## Step 9: Verify That Mosaic Saved the Run

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

## Step 10: Find the `chat_id` for Outbound Smoke Tests

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

## Step 11: Run an Outbound Smoke Test

After you know the `chat_id`, ask Mosaic to send a direct outbound Telegram message:

```bash
mosaic adapter telegram test-send --bot primary --chat-id 123456789 "hello from mosaic"
```

If you are using a forum topic or thread, add `--thread-id`:

```bash
mosaic adapter telegram test-send --bot primary --chat-id -1001234567890 --thread-id 99 "hello from mosaic"
```

Expected result:

- `status: delivered`
- a non-empty `provider_message_id`
- the message appears in Telegram

## Step 12: Prove Tool, Skill, and Workflow Paths

After plain chat works, test the capability routes from Telegram.

### Builtin tool

Send this in Telegram:

```text
/mosaic tool read_file .mosaic/config.yaml
```

Expected behavior:

- if you have no node configured, Mosaic reads the file locally
- if you intentionally registered a healthy node, Mosaic may prefer that node
- if a node exists but is stale or offline, Mosaic falls back to local execution

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

## Step 13: Prove Image and Document Uploads

Photo path:

1. send a photo to the bot with a short caption
2. ask for a brief summary or description

Document path:

1. send a small PDF or text document
2. ask the bot to summarize it

Repo sample payloads for the same shapes:

- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)

Relevant example configs:

- [examples/full-stack/openai-telegram-multimodal.config.yaml](../examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml)

These map to the repo payload shapes:

- `telegram-photo-update.json`
- `telegram-document-update.json`

## Step 14: Common Problems

### Telegram webhook registered, but inbound messages never arrive

Check:

- the public URL is really HTTPS
- the reverse proxy or tunnel really forwards to `127.0.0.1:18080`
- `mosaic adapter telegram webhook info --bot primary` shows the same URL you expect
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

### Telegram is working, but `/mosaic tool read_file ...` reports a node problem

This usually means Telegram itself is fine and Mosaic is receiving the webhook correctly.

What happened instead is:

- an old node is still registered under `.mosaic/nodes/registry`
- or a session/default node affinity still points at an old node

Check:

```bash
mosaic node list
mosaic session show <session-id>
```

Clean it up with:

```bash
mosaic node prune --stale
mosaic node detach --session <session-id>
```

If the stale binding is the default node route, use:

```bash
mosaic node detach --default
```

Remember:

- Telegram baseline does not require node
- node is optional
- starting a node is only necessary if you intentionally want node-routed execution

### I changed my public URL

Run:

```bash
mosaic adapter telegram webhook set \
  --bot primary \
  --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/primary" \
  --secret-token "$MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --drop-pending-updates
```

Then verify again with:

```bash
mosaic adapter telegram webhook info --bot primary
```

## Step 15: Multi-Bot Onboarding Appendix

When you are ready for more than one bot, switch to:

- [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml)

Then manage each bot explicitly:

```bash
mosaic adapter telegram webhook set --bot ops --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/ops"
mosaic adapter telegram webhook set --bot media --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/media"
mosaic adapter telegram webhook info --bot ops
mosaic adapter telegram webhook info --bot media
mosaic adapter telegram test-send --bot ops --chat-id <chat-id> "hello from ops"
mosaic adapter telegram test-send --bot media --chat-id <chat-id> "hello from media"
```

## Step 16: Clean Up

When you are done testing:

```bash
mosaic adapter telegram webhook delete --bot primary --drop-pending-updates
```

Your saved artifacts remain in:

- `.mosaic/sessions/`
- `.mosaic/runs/`
- `.mosaic/audit/`

## What To Read Next

- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [channels.md](./channels.md)
- [configuration.md](./configuration.md)
- [session-inspect-incident.md](./session-inspect-incident.md)
- [testing.md](./testing.md)
