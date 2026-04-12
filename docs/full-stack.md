# Full-Stack Guide

This is the operator golden path for Mosaic.

It binds one provider profile, one HTTP Gateway, one real ingress path, one saved session, one run trace, and one incident flow into a single walkthrough.

Use this guide when you want more than a local `mosaic run ...` smoke test.

Examples used by this guide:

- [examples/full-stack/README.md](../examples/full-stack/README.md)
- [examples/full-stack/openai-webchat.config.yaml](../examples/full-stack/openai-webchat.config.yaml)
- [examples/full-stack/openai-telegram.config.yaml](../examples/full-stack/openai-telegram.config.yaml)
- [examples/full-stack/openai-telegram-single-bot.config.yaml](../examples/full-stack/openai-telegram-single-bot.config.yaml)
- [examples/full-stack/openai-telegram-e2e.config.yaml](../examples/full-stack/openai-telegram-e2e.config.yaml)
- [examples/full-stack/openai-telegram-multi-bot.config.yaml](../examples/full-stack/openai-telegram-multi-bot.config.yaml)
- [examples/full-stack/openai-telegram-multimodal.config.yaml](../examples/full-stack/openai-telegram-multimodal.config.yaml)
- [examples/full-stack/openai-telegram-bot-split.config.yaml](../examples/full-stack/openai-telegram-bot-split.config.yaml)
- [examples/extensions/telegram-e2e.yaml](../examples/extensions/telegram-e2e.yaml)
- [examples/channels/webchat-openai-message.json](../examples/channels/webchat-openai-message.json)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)
- [examples/channels/telegram-photo-update.json](../examples/channels/telegram-photo-update.json)
- [examples/channels/telegram-document-update.json](../examples/channels/telegram-document-update.json)

## What this proves

The full-stack path verifies:

1. workspace config loads
2. provider profile and runtime policy are selected
3. Gateway serves HTTP and SSE
4. a real ingress path normalizes into a session
5. session, trace, audit, replay, and incident artifacts are written
6. inspect exposes the effective provider and runtime policy used by the run

## Release-Blocking No-Mock Path

This is the i2 acceptance lane.

It uses:

- a real OpenAI provider
- the real Gateway HTTP surface
- the real WebChat ingress endpoint
- real saved session, trace, audit, replay, and incident artifacts

1. Initialize the workspace.

```bash
mosaic setup init
cp examples/full-stack/openai-webchat.config.yaml .mosaic/config.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_WEBCHAT_SHARED_SECRET=full-stack-secret
```

2. Validate the setup and inspect the configured policy.

```bash
mosaic setup validate
mosaic setup doctor
mosaic config show
mosaic model list
```

3. Start the HTTP Gateway.

```bash
mosaic gateway serve --http 127.0.0.1:18080
```

4. In another shell, send a real WebChat ingress request.

```bash
curl -X POST http://127.0.0.1:18080/ingress/webchat \
  -H 'content-type: application/json' \
  -H "x-mosaic-shared-secret: $MOSAIC_WEBCHAT_SHARED_SECRET" \
  --data @examples/channels/webchat-openai-message.json
```

5. Inspect the same fact stream from the operator surfaces.

```bash
mosaic gateway --attach http://127.0.0.1:18080 status
mosaic gateway --attach http://127.0.0.1:18080 audit --limit 10
mosaic gateway --attach http://127.0.0.1:18080 replay --limit 10
mosaic session show full-stack-openai-webchat
mosaic inspect .mosaic/runs/<run-id>.json --verbose
mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>
```

In the verbose inspect output you should see:

- `provider_type: openai`
- `runtime policy:`
- `retry_backoff_ms`
- `max_provider_round_trips`

## Telegram Bot Acceptance Path

Use this path when Telegram is a real release target and you have:

- a real bot token
- a reachable HTTPS webhook endpoint
- a workspace configured from [examples/full-stack/openai-telegram.config.yaml](../examples/full-stack/openai-telegram.config.yaml)

This is a manual operator acceptance flow. When Telegram is in release scope, it becomes a release-blocking operator sign-off lane instead of an optional walkthrough.

For the complete Telegram-first runbook, use [telegram-real-e2e.md](./telegram-real-e2e.md).

That dedicated runbook adds:

- `/mosaic` channel command catalog discovery
- a fixed workspace config: [examples/full-stack/openai-telegram-e2e.config.yaml](../examples/full-stack/openai-telegram-e2e.config.yaml)
- a fixed manifest: [examples/extensions/telegram-e2e.yaml](../examples/extensions/telegram-e2e.yaml)
- plain conversation proof
- automatic `time_now` proof
- explicit `read_file` proof
- explicit `summarize_notes` skill proof
- explicit `summarize_operator_note` workflow proof
- image upload proof
- document upload proof
- multi-bot isolation proof when the release scope includes more than one Telegram bot
- `session`, `inspect`, `audit`, `replay`, and `incident` verification

## Automated Verification

Release-blocking OpenAI + WebChat lane:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

Continue with:

- [channels.md](./channels.md)
- [telegram-real-e2e.md](./telegram-real-e2e.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [provider-runtime-policy-matrix.md](./provider-runtime-policy-matrix.md)
- [gateway.md](./gateway.md)
- [session-inspect-incident.md](./session-inspect-incident.md)
- [troubleshooting.md](./troubleshooting.md)
