# Telegram Real E2E Runbook

This is the k5 Telegram-first release-blocking acceptance lane when Telegram is in release scope.

Maintenance rule:

- Telegram is the strongest real external interactive GUI acceptance lane and release-facing channel proof
- TUI is the primary local Codex-style operator surface
- CLI is the scripted/operator automation surface used to set up, validate, inspect, and sign off the lane
- when Telegram command behavior, skills, attachments, sandbox prerequisites, multi-bot behavior, or the TUI/Telegram proof split change, update this runbook, [telegram-step-by-step.md](./telegram-step-by-step.md), [tui.md](./tui.md), and the matching examples in the same change set

If you have not created a Telegram bot before, start with [telegram-step-by-step.md](./telegram-step-by-step.md) first and then come back here for the stricter acceptance checklist.

Use it when Telegram is a real release target and you need one repeatable path that proves:

- a real Telegram user message reaches Mosaic
- a real OpenAI-backed reply returns to the same Telegram chat
- `/mosaic` catalog discovery works in the live channel
- builtin tools, manifest skills, and workflows all work in a real Telegram session
- image and document uploads route through the expected attachment policy
- bot A and bot B remain isolated when the multi-bot lane is in scope
- `session`, `inspect`, `audit`, `replay`, and `incident` all describe the same run truth

Surface roles in this runbook:

- Telegram proves the external human-facing channel lane
- TUI proves the local operator shell when slash popup behavior, dynamic active turns, inline detail reveal, cancel/retry, or draft preservation are part of the release scope
- CLI drives the repeatable acceptance steps, artifact collection, and release sign-off

This lane is intentionally no-mock. It does not use fake ingress or a mock provider.

It is the primary product proof for these crates and surfaces:

- `mosaic-channel-telegram`
- `mosaic-gateway`
- `mosaic-runtime`
- `mosaic-tool-core`
- `mosaic-skill-core`
- `mosaic-workflow`
- `mosaic-session-core`
- `mosaic-inspect`
- `mosaic-extension-core`
- `mosaic-cli`

Related files:

- [examples/full-stack/openai-telegram-e2e.config.yaml](../examples/full-stack/openai-telegram-e2e.config.yaml)
- [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [examples/full-stack/openai-telegram-multimodal.config.yaml](../examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml)
- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)
- [examples/extensions/telegram-e2e.yaml](../examples/extensions/telegram-e2e.yaml)
- [docs/full-stack.md](./full-stack.md)
- [docs/real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)

## Prerequisites

You need all of these before starting:

- a real Telegram bot token in `MOSAIC_TELEGRAM_BOT_TOKEN`
- a real OpenAI API key in `OPENAI_API_KEY`
- a webhook secret token in `MOSAIC_TELEGRAM_SECRET_TOKEN`
- a public HTTPS base URL that Telegram can reach, for example `https://your-public-host.example.com`
- a writable workspace with persistent `.mosaic/` state

Mosaic currently serves plain HTTP with `mosaic gateway serve --http`.

For a live Telegram webhook you must place a real HTTPS reverse proxy or tunnel in front of that local HTTP listener. That is the current deployment fact, not a test loophole.

## Acceptance Workspace

Use a fixed workspace layout so every operator sees the same capability set:

- channel: `telegram`
- provider profile: `openai`
- builtin tools used for proof: `time_now`, `read_file`
- manifest skill: `summarize_notes`
- workflow: `summarize_operator_note`

Initialize the workspace:

```bash
mosaic setup init
cp examples/full-stack/openai-telegram-e2e.config.yaml .mosaic/config.yaml
cp examples/extensions/telegram-e2e.yaml .mosaic/extensions/telegram-e2e.yaml
```

If your acceptance workspace is outside the repo, copy those two files from the repo into the matching `.mosaic/` paths instead of using the relative `cp` commands above.

## Config File

`examples/full-stack/openai-telegram-e2e.config.yaml` is the workspace config for this lane. The related operator examples are:

- `examples/full-stack/openai-telegram-single-bot.config.yaml`
- `examples/full-stack/openai-telegram-multi-bot.config.yaml`
- `examples/full-stack/openai-telegram-multimodal.config.yaml`
- `examples/full-stack/openai-telegram-bot-split.config.yaml`

The acceptance manifest is:

```yaml
name: telegram-e2e
version: 0.1.0
description: Telegram-first real acceptance manifest with one explicit skill and one explicit workflow, both attachment-aware.
schema_version: 1
tools: []
skills:
  - type: manifest
    name: summarize_notes
    description: Summarize a short operator note or document caption inside a real Telegram session.
    visibility: visible
    invocation_mode: explicit_only
    allowed_channels:
      - telegram
    accepts_attachments: true
workflows:
  - name: summarize_operator_note
    visibility:
      source: telegram-e2e
      visibility: visible
      invocation_mode: explicit_only
      allowed_channels:
        - telegram
      accepts_attachments: true
```

## Environment Variables

Export the real secrets before validating the workspace:

```bash
export OPENAI_API_KEY=your_openai_api_key
export MOSAIC_TELEGRAM_BOT_TOKEN=your_real_telegram_bot_token
export MOSAIC_TELEGRAM_SECRET_TOKEN=your_long_random_webhook_secret
export MOSAIC_PUBLIC_WEBHOOK_BASE_URL=https://your-public-host.example.com
```

## Validate the Workspace

Run the full local operator checks before touching Telegram:

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
mosaic extension validate
mosaic extension list
mosaic sandbox status
mosaic sandbox list
mosaic adapter status
mosaic adapter doctor
```

Expected state:

- `active_profile: openai`
- `telegram` adapter reports ingress and outbound readiness
- the extension list includes `telegram-e2e`
- the loaded capabilities include `summarize_notes` and `summarize_operator_note`
- `mosaic model list` shows the profile capability summary, including attachment mode where relevant
- sandbox status is healthy enough for any Telegram-exposed markdown skill pack or attachment processor env

For local operator verification, the chat-first TUI should expose the same checks with:

- `/sandbox status`
- `/sandbox inspect <env>`
- `/sandbox rebuild <env>`

## Start Gateway

Start the local HTTP Gateway on a stable port:

```bash
mosaic gateway serve --http 127.0.0.1:18080
```

Expose that listener through your HTTPS reverse proxy or tunnel so the final public URL is:

```text
${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram
```

If you are validating a multi-bot workspace, the final public URLs become:

- `${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/ops`
- `${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/media`

## Register the Telegram Webhook

Register the live Telegram webhook from CLI:

```bash
mosaic adapter telegram webhook set \
  --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram" \
  --drop-pending-updates
```

Then confirm the webhook state from CLI:

```bash
mosaic adapter telegram webhook info
```

Expected response shape:

- `"ok": true`
- `"url": "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram"`
- no current delivery error

Before waiting for a real user message, you can verify outbound delivery from CLI with one direct smoke send:

```bash
mosaic adapter telegram test-send --chat-id <chat-id> "mosaic outbound smoke"
```

This should return `status: delivered` and a Telegram `provider_message_id`.

## Plain Conversation Proof

Send this message to the bot from a real Telegram chat:

```text
Hello Mosaic. Reply with one short sentence confirming this message came from Telegram.
```

Expected result:

- a real assistant reply appears in the same Telegram chat
- the workspace writes a new `.mosaic/runs/<run-id>.json`
- the workspace writes or updates a Telegram session in `.mosaic/sessions/`
- audit and replay facts include the inbound message and the completed run

Capture the latest saved artifacts:

```bash
TRACE_PATH=$(ls -t .mosaic/runs/*.json | head -n 1)
RUN_ID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["run_id"])' "$TRACE_PATH")
SESSION_ID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["session_id"])' "$TRACE_PATH")
```

Verify them:

```bash
mosaic gateway status
mosaic gateway audit --limit 20
mosaic gateway replay --limit 20
mosaic session show "$SESSION_ID"
mosaic inspect "$TRACE_PATH" --verbose
mosaic gateway incident "$RUN_ID"
```

If you want to verify the running HTTP process directly instead of the local workspace view, use the same commands with `--attach http://127.0.0.1:18080`.

In the saved trace you should see:

- `channel: telegram`
- `adapter: telegram_bot`
- `route decision:` with `route_mode: assistant`, `route_kind: assistant`, and `execution_target: provider`
- a real `effective_profile` using `openai`
- one outbound delivery back to Telegram

## Channel Command Catalog Proof

This lane must prove that chat-native capability discovery is live.

Send:

```text
/start
/help
/mosaic help
/mosaic help tools
/mosaic help workflows
```

Expected result:

- Telegram receives grouped catalog help
- Telegram shows a command keyboard with `/mosaic`, `/mosaic help`, category shortcuts, and status actions
- the groups include `Session`, `Runtime`, `Tools`, `Skills`, `Workflows`, and `Gateway`
- the visible items match the currently allowed channel and bot policy
- the trace exposes the channel command catalog scope in inspect output
- `/help@bot_name` and `/mosaic@bot_name ...` work in group contexts

## Tool Proof

This lane must prove one automatic tool path and one explicit tool path.

### Automatic `time_now`

Send this plain-language Telegram message:

```text
What time is it right now in UTC? Use the built-in time tool if it is available and return only the timestamp.
```

Expected result:

- Telegram receives a timestamp reply
- `mosaic inspect "$TRACE_PATH" --verbose` shows `route_mode: assistant`
- the same trace shows a `tool_calls` entry for `time_now`
- the capability trace shows `capability_source_kind: builtin` and `execution_target: local`

### Explicit `read_file`

Send this explicit Telegram command:

```text
/mosaic tool read_file .mosaic/config.yaml
```

Expected result:

- Telegram receives the file content or preview
- the trace shows `route_mode: tool` and `route_kind: tool`
- `selected_tool: read_file`
- `capability_invocations` contains the file read summary
- the audit log still ties the run back to the same Telegram conversation

## Skill Proof

Send this explicit Telegram command:

```text
/mosaic skill summarize_notes Shift handoff: webhook traffic recovered after rotating the Telegram secret token and restarting the gateway.
```

Expected result:

- Telegram receives a `summary:` style reply
- the trace shows `route_mode: skill` and `route_kind: skill`
- `selected_skill: summarize_notes`
- the trace includes `SkillStarted` and `SkillFinished` facts through `skill_calls`
- the skill trace shows `capability_source_kind: manifest_skill` or `markdown_skill_pack`, depending on the loaded skill

Check it with:

```bash
TRACE_PATH=$(ls -t .mosaic/runs/*.json | head -n 1)
mosaic inspect "$TRACE_PATH" --verbose
```

If the bot policy exposes a markdown skill pack rather than the manifest example, repeat the same Telegram proof and confirm the skill source and sandbox binding through `mosaic inspect --verbose` plus `mosaic sandbox status`.

## Workflow Proof

Send this explicit Telegram command:

```text
/mosaic workflow summarize_operator_note Operator note: the Telegram ingress, OpenAI response, and saved incident bundle all matched the same gateway run.
```

Expected result:

- Telegram receives the workflow result in the same chat
- the trace shows `route_mode: workflow` and `route_kind: workflow`
- `selected_workflow: summarize_operator_note`
- `step traces` include the workflow step chain with `execution_target` and `orchestration_owner`

Check it with:

```bash
TRACE_PATH=$(ls -t .mosaic/runs/*.json | head -n 1)
mosaic inspect "$TRACE_PATH" --verbose
```

## Attachment Proof

This lane must prove both the image path and the document path.

### Image Upload

Send a real photo to the bot with a short caption asking for a summary. The matching repo payload shape is:

- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)

Expected result:

- the trace contains `attachments: 1`
- `attachment_route.mode` is visible in inspect
- `selected_profile` matches the multimodal profile when the workspace uses provider-native multimodal routing

### Document Upload

Send a small PDF or text document and ask for a summary. The matching repo payload shape is:

- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)

Expected result:

- the trace records a document attachment
- inspect shows either provider-native document routing or `specialized_processor`
- the attachment-aware `summarize_notes` skill can be used explicitly when the bot policy allows it

## Bot A / Bot B Isolation Proof

This is the formal `bot A / bot B isolation` check for the multi-bot lane.

Run this when the multi-bot lane is in scope, using [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml) or [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml).

Register both webhook paths:

```bash
mosaic adapter telegram webhook set --bot ops --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/ops"
mosaic adapter telegram webhook set --bot media --url "${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram/media"
mosaic adapter telegram webhook info --bot ops
mosaic adapter telegram webhook info --bot media
```

Then prove:

- bot A writes sessions with its own `bot_name` and `bot_route`
- bot B writes sessions with its own `bot_name` and `bot_route`
- `mosaic adapter telegram test-send --bot ops ...` and `mosaic adapter telegram test-send --bot media ...` deliver through different bot tokens
- inspect, audit, and session metadata do not collapse both bots into one route

## CLI, Inspect, Audit, and Incident Proof

For every Telegram proof above, the operator should be able to reproduce the same facts from CLI only:

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
mosaic gateway status
mosaic gateway audit --limit 20
mosaic gateway replay --limit 20
mosaic session show "$SESSION_ID"
mosaic inspect "$TRACE_PATH" --verbose
mosaic gateway incident "$RUN_ID"
```

The same run should agree across all surfaces:

- Telegram reply text
- session transcript
- inspect ingress and route decision metadata
- attachment route and selected profile
- audit trail
- replay window
- `.mosaic/audit/incidents/${RUN_ID}.json`

When the Telegram lane depends on markdown skills or attachment processors, also verify:

```bash
mosaic sandbox status
mosaic sandbox list
```

## Expected Artifacts

At the end of the full Telegram lane you should have:

- at least one Telegram session in `.mosaic/sessions/`
- one or more traces in `.mosaic/runs/`
- audit events in `.mosaic/audit/`
- an incident bundle in `.mosaic/audit/incidents/`

The inspect output for the explicit command runs should expose:

- `route_mode`
- `route_kind`
- `selected_tool`, `selected_skill`, or `selected_workflow`
- `selection_reason`
- `capability_source`
- `capability_source_kind`
- `execution_target`
- `orchestration_owner`
- `profile_used`
- `attachment_route`
- `bot_identity`
- `failure_origin` when a run fails

## Troubleshooting

Use this order when the lane fails:

1. Run `mosaic setup validate` and `mosaic setup doctor` again.
2. Run `mosaic adapter status` and confirm Telegram outbound is ready.
3. Run `mosaic adapter telegram webhook info` and check for webhook URL mismatch or delivery errors.
4. Run `mosaic adapter telegram test-send --chat-id <chat-id> "mosaic outbound smoke"` to isolate outbound delivery from ingress.
5. Confirm the public HTTPS endpoint really forwards to `127.0.0.1:18080`.
6. Check `mosaic gateway audit --limit 20` for missing inbound or outbound events.
7. Check `mosaic inspect "$TRACE_PATH" --verbose` for `provider_failure`, `route decision`, `attachment_route`, or capability access failures.
8. If explicit `read_file` fails, verify the requested file is inside the current workspace root.
9. If the manifest capability is missing, rerun `mosaic extension validate` and confirm `.mosaic/extensions/telegram-e2e.yaml` exists.

For broader debugging patterns, continue with [troubleshooting.md](./troubleshooting.md).

## Cleanup

Delete the Telegram webhook when the acceptance window is over:

```bash
mosaic adapter telegram webhook delete --drop-pending-updates
```

You can then stop the local Gateway and archive the acceptance artifacts from:

- `.mosaic/sessions/`
- `.mosaic/runs/`
- `.mosaic/audit/`

## Appendix: Raw Telegram Bot API Curl

The CLI commands above are the primary operator path. If you need to compare the raw Telegram Bot API calls directly, the equivalent commands are:

```bash
curl -sS -X POST "https://api.telegram.org/bot${MOSAIC_TELEGRAM_BOT_TOKEN}/setWebhook" \
  --data-urlencode "url=${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram" \
  --data-urlencode "secret_token=${MOSAIC_TELEGRAM_SECRET_TOKEN}" \
  --data-urlencode 'allowed_updates=["message"]' \
  --data-urlencode "drop_pending_updates=true"

curl -sS "https://api.telegram.org/bot${MOSAIC_TELEGRAM_BOT_TOKEN}/getWebhookInfo"

curl -sS -X POST "https://api.telegram.org/bot${MOSAIC_TELEGRAM_BOT_TOKEN}/deleteWebhook" \
  --data-urlencode "drop_pending_updates=true"
```
