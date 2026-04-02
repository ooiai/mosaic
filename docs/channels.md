# Channel Setup

This guide describes the operator-facing channel baseline after k1-k4:

- every channel normalizes into one Gateway ingress contract
- every channel can expose the same `/mosaic` command story
- attachments are normalized before runtime routing
- Telegram can run as one bot or as a multi-bot workspace

Examples used by this guide:

- [examples/channels/README.md](../examples/channels/README.md)
- [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)
- [examples/channels/webchat-openai-message.json](../examples/channels/webchat-openai-message.json)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)
- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)
- [examples/full-stack/openai-telegram-single-bot.config.yaml](../examples/full-stack/openai-telegram-single-bot.config.yaml)
- [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml)

## Telegram Documentation Maintenance Rule

Telegram is the strongest real external GUI acceptance lane.
TUI is the primary local chat-first operator surface.

Because of that, a Telegram-affecting change is not complete unless the matching docs and examples change with it.

Minimum update matrix:

- command or help behavior changes:
  - `docs/telegram-step-by-step.md`
  - `docs/telegram-real-e2e.md`
  - `docs/channels.md`
- skill or markdown-skill behavior changes:
  - `docs/telegram-step-by-step.md`
  - `docs/telegram-real-e2e.md`
  - `docs/skills.md`
  - Telegram examples
- attachment or multimodal behavior changes:
  - `docs/telegram-step-by-step.md`
  - `docs/telegram-real-e2e.md`
  - `docs/channels.md`
  - `docs/capabilities.md`
  - Telegram examples and fixtures
- sandbox prerequisites change:
  - `docs/telegram-step-by-step.md`
  - `docs/telegram-real-e2e.md`
  - `docs/sandbox.md`
- multi-bot or webhook operations change:
  - `docs/telegram-step-by-step.md`
  - `docs/telegram-real-e2e.md`
  - `docs/channels.md`
  - CLI examples

Release work is not complete until those files move together.

When a change alters how proof is divided between local operator work and external channel work, also update:

- `docs/tui.md`
- `docs/testing.md`
- `docs/release.md`

## Unified Channel Story

Regardless of the source channel, Mosaic tries to preserve the same five facts:

1. `channel`: where the message came from
2. `conversation_id`: the durable conversation or chat identity
3. `thread_id`: the topic, forum thread, or sub-thread when the channel has one
4. `text`: the normalized user-visible input
5. `attachments`: normalized files, images, documents, audio, or video

That is why the same runtime can answer:

- plain assistant chat
- `/mosaic help`
- `/mosaic tool ...`
- `/mosaic skill ...`
- `/mosaic workflow ...`

without each adapter re-implementing product semantics.

## Current Ingress Paths

Mosaic currently exposes these HTTP channel entrypoints:

- `POST /ingress/webchat`
- `POST /ingress/telegram`
- `POST /ingress/telegram/<bot-route>` for multi-bot Telegram workspaces

All routes are served by:

```bash
mosaic gateway serve --http 127.0.0.1:8080
```

## Channel Command Catalog

The channel-native operator story is built around `/mosaic`.

Telegram currently exposes that story in a hybrid form:

- text commands such as `/mosaic help`, `/mosaic tool ...`, and `/mosaic workflow ...`
- Telegram command keyboard shortcuts for `/mosaic`, `/mosaic help`, category help, and status actions
- Telegram-friendly aliases such as `/start`, `/help`, and group-addressed forms like `/mosaic@your_bot`

Typical Telegram examples:

```text
/start
/help
/mosaic help
/mosaic help tools
/mosaic session status
/mosaic tool read_file .mosaic/config.yaml
/mosaic skill summarize_notes Shift handoff note goes here.
/mosaic workflow summarize_operator_note Workflow input goes here.
```

The Gateway command catalog is dynamic. A command only appears when:

- the current channel is allowed
- the current bot allowlist permits it
- the capability visibility and invocation mode allow it

That same catalog model is the standard future channels should reuse.

## Attachment Model

Attachments enter the system as normalized channel attachments. Today the main Telegram cases are:

- image uploads
- document uploads

The Gateway then applies three layers:

1. attachment policy: size, mime type, cache, download timeout
2. routing policy: `provider_native`, `specialized_processor`, or `disabled`
3. capability exposure: tool, skill, or workflow must explicitly accept attachments

Sample payloads:

- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)

Operator-visible outcomes:

- provider-native multimodal routing into a vision-capable profile
- specialized processor routing for documents
- inspect output that records attachment route, selected profile, and failures

## WebChat

Sample payload:

- [examples/channels/webchat-message.json](../examples/channels/webchat-message.json)
- [examples/channels/webchat-openai-message.json](../examples/channels/webchat-openai-message.json)

Send it:

```bash
curl -X POST http://127.0.0.1:8080/ingress/webchat \
  -H 'content-type: application/json' \
  --data @examples/channels/webchat-message.json
```

If `auth.webchat_shared_secret_env` is configured, also send:

```bash
-H "x-mosaic-shared-secret: $MOSAIC_WEBCHAT_SHARED_SECRET"
```

For the release-blocking no-mock lane, use:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

## Telegram Single-Bot Baseline

Beginner-friendly config:

- [examples/full-stack/openai-telegram-single-bot.config.yaml](../examples/full-stack/openai-telegram-single-bot.config.yaml)

Sample inbound payload:

- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)

Send it to the legacy single-bot path:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

The sample payload maps to session id `telegram--100123-99`.

## Telegram Attachments

Photo upload:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-photo-update.json
```

Document upload:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-document-update.json
```

Use these configs when you want the docs to match the attachment route:

- [examples/full-stack/openai-telegram-multimodal.config.yaml](../examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml)

## Telegram Multi-Bot

Multi-bot baseline:

- [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml)

Each bot gets its own:

- token env
- webhook secret env
- webhook path
- route key
- default profile
- tool / skill / workflow allowlist
- attachment policy override

Example bot-specific curl:

```bash
curl -X POST http://127.0.0.1:8080/ingress/telegram/media \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_MEDIA_SECRET_TOKEN" \
  --data @examples/channels/telegram-photo-update.json
```

Operator CLI for multi-bot management:

```bash
mosaic adapter telegram webhook info --bot media
mosaic adapter telegram webhook set --bot media --url https://public.example.com/ingress/telegram/media
mosaic adapter telegram test-send --bot media --chat-id <chat-id> "hello from media bot"
```

## Adapter Health Checks

Before exposing ingress publicly, check:

```bash
mosaic adapter status
mosaic adapter doctor
```

## Future Channel Standard

Future adapters should follow the same contract:

- normalize identities and thread context before runtime
- expose the same `/mosaic` command catalog model
- emit normalized attachments instead of channel-specific file structs
- preserve delivery traces and failures in inspect and audit

Do not bind a new channel directly to one hard-coded model or one hard-coded tool chain.

## Golden Path References

If you want the full provider + gateway + channel + inspect walkthrough, continue with:

- [full-stack.md](./full-stack.md)
- [telegram-step-by-step.md](./telegram-step-by-step.md)
- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [examples/full-stack/README.md](../examples/full-stack/README.md)
