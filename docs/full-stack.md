# Full-Stack Guide

This is the operator golden path for Mosaic.

It binds one provider profile, one HTTP Gateway, one real channel ingress path, one saved session, one run trace, and one incident flow into a single walkthrough.

Use this guide when you want more than a local `mosaic run ...` smoke test.

Examples used by this guide:

- [examples/full-stack/README.md](../examples/full-stack/README.md)
- [examples/full-stack/mock-telegram.config.yaml](../examples/full-stack/mock-telegram.config.yaml)
- [examples/full-stack/openai-telegram.config.yaml](../examples/full-stack/openai-telegram.config.yaml)
- [examples/channels/telegram-update.json](../examples/channels/telegram-update.json)

## What this proves

The full-stack path verifies:

1. workspace config loads
2. provider profile is selected
3. Gateway serves HTTP and SSE
4. Telegram ingress normalizes into a session
5. session, trace, audit, replay, and incident artifacts are written

## Fast Local Full-Stack Path

This lane uses the mock provider but the real Gateway HTTP and Telegram ingress path.

1. Initialize the workspace.

```bash
mosaic setup init
cp examples/full-stack/mock-telegram.config.yaml .mosaic/config.yaml
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
```

2. Validate the config.

```bash
mosaic setup validate
mosaic setup doctor
```

3. Start the HTTP Gateway.

```bash
mosaic gateway serve --http 127.0.0.1:18080
```

4. In another shell, send a Telegram webhook payload.

```bash
curl -X POST http://127.0.0.1:18080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

5. Inspect the resulting control-plane state.

```bash
mosaic gateway --attach http://127.0.0.1:18080 status
mosaic gateway --attach http://127.0.0.1:18080 audit --limit 10
mosaic gateway --attach http://127.0.0.1:18080 replay --limit 10
mosaic session show telegram--100123-99
mosaic inspect .mosaic/runs/<run-id>.json
mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>
```

## Real Provider + Real Channel Path

This lane keeps the same Telegram ingress path and swaps the provider profile to OpenAI.

1. Prepare the workspace config and secrets.

```bash
mosaic setup init
cp examples/full-stack/openai-telegram.config.yaml .mosaic/config.yaml
export OPENAI_API_KEY=your_api_key_here
export MOSAIC_TELEGRAM_SECRET_TOKEN=full-stack-secret
```

2. Validate the setup.

```bash
mosaic setup validate
mosaic setup doctor
mosaic model list
```

3. Serve the Gateway and post the same Telegram payload.

```bash
mosaic gateway serve --http 127.0.0.1:18080
curl -X POST http://127.0.0.1:18080/ingress/telegram \
  -H 'content-type: application/json' \
  -H "x-telegram-bot-api-secret-token: $MOSAIC_TELEGRAM_SECRET_TOKEN" \
  --data @examples/channels/telegram-update.json
```

4. Verify the operator artifacts.

```bash
mosaic session show telegram--100123-99
mosaic inspect .mosaic/runs/<run-id>.json --verbose
mosaic gateway --attach http://127.0.0.1:18080 incident <run-id>
```

## Automated Verification

Mock full-stack lane:

```bash
./scripts/test-full-stack-example.sh mock
```

Real OpenAI + Telegram lane:

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai
```

Continue with:

- [channels.md](./channels.md)
- [gateway.md](./gateway.md)
- [session-inspect-incident.md](./session-inspect-incident.md)
- [troubleshooting.md](./troubleshooting.md)
