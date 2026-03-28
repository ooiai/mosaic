# CLI Reference

This reference is organized around the operator tasks you actually perform with Mosaic.

## Global Help

Use this when you need the full command map and the high-level operator groupings.

```bash
mosaic --help
```

## Operator Groupings

- `setup` and `config`: bootstrap the workspace, validate it, and explain the merged config.
- `tui`, `run`, `session`, and `model`: operate conversations and provider routing.
- `inspect`: explain one saved run trace.
- `gateway`, `adapter`, and `node`: operate the control plane and its edges.
- `capability`, `cron`, `extension`, and `memory`: operate automations, plugins, and stored state.

## Setup and Config

Initialize the workspace the first time:

```bash
mosaic setup init
```

Initialize on a specific real provider profile:

```bash
mosaic setup init --profile anthropic-sonnet
```

Opt into the dev-only mock template:

```bash
mosaic setup init --dev-mock
```

Overwrite an existing generated template:

```bash
mosaic setup init --force
```

Validate the merged config:

```bash
mosaic setup validate
```

Run the categorized doctor checks:

```bash
mosaic setup doctor
```

Show the merged redacted config that Mosaic will actually run:

```bash
mosaic config show
```

Show only the config source stack and precedence:

```bash
mosaic config sources
```

Emit the merged config as machine-readable JSON:

```bash
mosaic config show --json
```

## Conversations and Models

Start the long-lived operator console:

```bash
mosaic tui
```

Resume or create a specific operator conversation:

```bash
mosaic tui --session support --profile openai
```

Attach the operator console to a remote Gateway:

```bash
mosaic tui --attach http://127.0.0.1:8080 --session remote-demo
```

Run one file-backed job without entering the long-lived console:

```bash
mosaic run examples/time-now-agent.yaml
```

Run with persisted session routing and memory:

```bash
mosaic run examples/time-now-agent.yaml --session docs-demo
```

Force a workflow entrypoint:

```bash
mosaic run examples/workflows/research-brief.yaml --workflow research_brief
```

Watch a single run in the terminal observer:

```bash
mosaic run examples/time-now-agent.yaml --tui
```

List stored sessions:

```bash
mosaic session list
```

Inspect one session transcript and routing state:

```bash
mosaic session show docs-demo
```

List configured provider profiles:

```bash
mosaic model list
```

Switch the active provider profile in workspace config:

```bash
mosaic model use openai
```

## Inspect and Incident Response

Show the default run summary for one saved trace:

```bash
mosaic inspect .mosaic/runs/<run-id>.json
```

Expand the trace into provider, tool, workflow, memory, and governance detail:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --verbose
```

Emit the saved trace as machine-readable JSON:

```bash
mosaic inspect .mosaic/runs/<run-id>.json --json
```

## Gateway, Adapters, and Nodes

Show local Gateway health, readiness, and metrics:

```bash
mosaic gateway status
```

Serve the HTTP control plane:

```bash
mosaic gateway serve --http 127.0.0.1:8080
```

Watch the local in-process Gateway monitor:

```bash
mosaic gateway serve --local
```

List session summaries from the Gateway:

```bash
mosaic gateway sessions
```

List run summaries from the Gateway run registry:

```bash
mosaic gateway runs
```

Inspect one stored Gateway run:

```bash
mosaic gateway show-run <gateway-run-id>
```

Request cancellation for an active run:

```bash
mosaic gateway cancel <gateway-run-id>
```

Retry a terminal run through the same Gateway:

```bash
mosaic gateway retry <gateway-run-id>
```

Review recent audit events:

```bash
mosaic gateway audit --limit 20
```

Review the replay window:

```bash
mosaic gateway replay --limit 20
```

Export an incident bundle for one run:

```bash
mosaic gateway incident <run-id>
```

Show adapter status:

```bash
mosaic adapter status
```

Run adapter doctor:

```bash
mosaic adapter doctor
```

Show the live Telegram webhook state:

```bash
mosaic adapter telegram webhook info
```

Register the Telegram webhook from CLI:

```bash
mosaic adapter telegram webhook set --url https://public.example.com/ingress/telegram --drop-pending-updates
```

Delete the Telegram webhook from CLI:

```bash
mosaic adapter telegram webhook delete --drop-pending-updates
```

Send one direct outbound Telegram smoke message:

```bash
mosaic adapter telegram test-send --chat-id 123456789 "hello from mosaic"
```

Start a local node:

```bash
mosaic node serve --id laptop
```

List nodes:

```bash
mosaic node list
```

## Delivery and Release

Run the repository smoke path before shipping a change:

```bash
make smoke
```

Run the golden example and docs verification lane:

```bash
make test-golden
```

Run gated real integration tests when credentials or local daemons are available:

```bash
MOSAIC_REAL_TESTS=1 make test-real
```

Run the full delivery gate:

```bash
make release-check
```

Build a releasable bundle under `dist/`:

```bash
make package
```
