# Telegram Real E2E Runbook

This is the j5 Telegram-first release-blocking acceptance lane when Telegram is in release scope.

If you have not created a Telegram bot before, start with [telegram-step-by-step.md](./telegram-step-by-step.md) first and then come back here for the stricter acceptance checklist.

Use it when Telegram is a real release target and you need one repeatable path that proves:

- a real Telegram user message reaches Mosaic
- a real OpenAI-backed reply returns to the same Telegram chat
- builtin tools, manifest skills, and workflows all work in a real Telegram session
- `session`, `inspect`, `audit`, `replay`, and `incident` all describe the same run truth

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

`examples/full-stack/openai-telegram-e2e.config.yaml` is the workspace config for this lane:

```yaml
schema_version: 1
active_profile: openai
provider_defaults:
  timeout_ms: 45000
  max_retries: 2
  retry_backoff_ms: 250
profiles:
  openai:
    type: openai
    model: gpt-5.4-mini
    base_url: https://api.openai.com/v1
    api_key_env: OPENAI_API_KEY
    transport:
      timeout_ms: 60000
      max_retries: 3
      retry_backoff_ms: 300
runtime:
  max_provider_round_trips: 8
  max_workflow_provider_round_trips: 6
  continue_after_tool_error: false
extensions:
  manifests:
    - path: .mosaic/extensions/telegram-e2e.yaml
      version_pin: 0.1.0
      enabled: true
deployment:
  profile: local
  workspace_name: mosaic-telegram-real-e2e
auth:
  telegram_secret_token_env: MOSAIC_TELEGRAM_SECRET_TOKEN
session_store:
  root_dir: .mosaic/sessions
inspect:
  runs_dir: .mosaic/runs
audit:
  root_dir: .mosaic/audit
  retention_days: 30
  event_replay_window: 512
  redact_inputs: true
observability:
  enable_metrics: true
  enable_readiness: true
  slow_consumer_lag_threshold: 64
policies:
  allow_exec: false
  allow_webhook: true
  allow_cron: false
  allow_mcp: false
  hot_reload_enabled: false
```

## Extension Manifest

`examples/extensions/telegram-e2e.yaml` adds the Telegram acceptance skill and workflow:

```yaml
name: telegram-e2e
version: 0.1.0
description: Telegram-first real acceptance manifest with one explicit skill and one explicit workflow.
schema_version: 1
tools: []
skills:
  - type: manifest
    name: summarize_notes
    description: Summarize a short operator note inside a real Telegram session.
    visibility: visible
    invocation_mode: explicit_only
    allowed_channels:
      - telegram
    tools: []
    system_prompt: null
    steps:
      - kind: echo
        name: draft
        input: "{{input}}"
      - kind: summarize
        name: summarize
        input: "{{current}}"
workflows:
  - name: summarize_operator_note
    description: Run summarize_notes against an operator note from Telegram.
    visibility:
      source: telegram-e2e
      visibility: visible
      invocation_mode: explicit_only
      allowed_channels:
        - telegram
    steps:
      - name: summarize
        kind: skill
        skill: summarize_notes
        input: "{{input}}"
mcp: null
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
mosaic adapter status
mosaic adapter doctor
```

Expected state:

- `active_profile: openai`
- `telegram` adapter reports ingress and outbound readiness
- the extension list includes `telegram-e2e`
- the loaded capabilities include `summarize_notes` and `summarize_operator_note`

## Start Gateway

Start the local HTTP Gateway on a stable port:

```bash
mosaic gateway serve --http 127.0.0.1:18080
```

Expose that listener through your HTTPS reverse proxy or tunnel so the final public URL is:

```text
${MOSAIC_PUBLIC_WEBHOOK_BASE_URL}/ingress/telegram
```

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
- `route decision:` with `route_mode: assistant`
- a real `effective_profile` using `openai`
- one outbound delivery back to Telegram

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

### Explicit `read_file`

Send this explicit Telegram command:

```text
/mosaic tool read_file .mosaic/config.yaml
```

Expected result:

- Telegram receives the file content or preview
- the trace shows `route_mode: tool`
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
- the trace shows `route_mode: skill`
- `selected_skill: summarize_notes`
- the trace includes `SkillStarted` and `SkillFinished` facts through `skill_calls`

Check it with:

```bash
TRACE_PATH=$(ls -t .mosaic/runs/*.json | head -n 1)
mosaic inspect "$TRACE_PATH" --verbose
```

## Workflow Proof

Send this explicit Telegram command:

```text
/mosaic workflow summarize_operator_note Operator note: the Telegram ingress, OpenAI response, and saved incident bundle all matched the same gateway run.
```

Expected result:

- Telegram receives the workflow result in the same chat
- the trace shows `route_mode: workflow`
- `selected_workflow: summarize_operator_note`
- `step traces` include the workflow step chain

Check it with:

```bash
TRACE_PATH=$(ls -t .mosaic/runs/*.json | head -n 1)
mosaic inspect "$TRACE_PATH" --verbose
```

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
- audit trail
- replay window
- `.mosaic/audit/incidents/${RUN_ID}.json`

## Expected Artifacts

At the end of the full Telegram lane you should have:

- at least one Telegram session in `.mosaic/sessions/`
- one or more traces in `.mosaic/runs/`
- audit events in `.mosaic/audit/`
- an incident bundle in `.mosaic/audit/incidents/`

The inspect output for the explicit command runs should expose:

- `route_mode`
- `selected_tool`, `selected_skill`, or `selected_workflow`
- `selection_reason`
- `capability_source`
- `profile_used`

## Troubleshooting

Use this order when the lane fails:

1. Run `mosaic setup validate` and `mosaic setup doctor` again.
2. Run `mosaic adapter status` and confirm Telegram outbound is ready.
3. Run `mosaic adapter telegram webhook info` and check for webhook URL mismatch or delivery errors.
4. Run `mosaic adapter telegram test-send --chat-id <chat-id> "mosaic outbound smoke"` to isolate outbound delivery from ingress.
5. Confirm the public HTTPS endpoint really forwards to `127.0.0.1:18080`.
6. Check `mosaic gateway audit --limit 20` for missing inbound or outbound events.
7. Check `mosaic inspect "$TRACE_PATH" --verbose` for `provider_failure`, `route decision`, or capability access failures.
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
