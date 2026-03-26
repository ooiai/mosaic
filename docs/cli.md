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

Start a local node:

```bash
mosaic node serve --id laptop
```

List nodes:

```bash
mosaic node list
```

Attach a session to a node:

```bash
mosaic node attach laptop --session ops-demo
```

Inspect node capabilities:

```bash
mosaic node capabilities laptop
```

## Automations and State

List capability jobs:

```bash
mosaic capability jobs
```

Show `exec` guardrails:

```bash
mosaic capability exec guardrails
```

Run a command through the capability layer:

```bash
mosaic capability exec run ./script.sh --session ops-demo
```

Test a webhook call:

```bash
mosaic capability webhook test http://127.0.0.1:8080 --method GET
```

List cron registrations:

```bash
mosaic cron list
```

Register a cron job:

```bash
mosaic cron register every-hour "0 * * * *" "status report" --session ops-demo
```

Trigger one cron job manually:

```bash
mosaic cron trigger every-hour
```

List loaded extensions:

```bash
mosaic extension list
```

Validate extensions:

```bash
mosaic extension validate
```

Reload extensions:

```bash
mosaic extension reload
```

List sessions with saved memory:

```bash
mosaic memory list
```

Show one session memory view:

```bash
mosaic memory show ops-demo
```

Search memory entries:

```bash
mosaic memory search summary --tag note
```
