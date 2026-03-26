# CLI Reference

This document groups the operator-facing commands you are most likely to use.

## Global help

```bash
mosaic --help
```

## Setup and configuration

Initialize the workspace:

```bash
mosaic setup init
```

Regenerate the template if you want to overwrite an existing config:

```bash
mosaic setup init --force
```

Validate config:

```bash
mosaic setup validate
```

Run doctor checks:

```bash
mosaic setup doctor
```

## Models

List configured profiles:

```bash
mosaic model list
```

Switch the active profile in the workspace config:

```bash
mosaic model use openai
```

## TUI

Start the local interactive TUI:

```bash
mosaic tui
```

Start the TUI on a specific session or profile:

```bash
mosaic tui --session support --profile openai
```

Attach the TUI to a remote Gateway:

```bash
mosaic tui --attach http://127.0.0.1:8080 --session remote-demo
```

## File-based runs

Run an app config file:

```bash
mosaic run examples/time-now-agent.yaml
```

Run with an explicit session:

```bash
mosaic run examples/time-now-agent.yaml --session docs-demo
```

Force a workflow entrypoint:

```bash
mosaic run examples/workflows/research-brief.yaml --workflow research_brief
```

Run against a remote Gateway:

```bash
mosaic run examples/time-now-agent.yaml --attach http://127.0.0.1:8080 --session remote-demo
```

## Sessions and traces

List sessions:

```bash
mosaic session list
```

Show one session:

```bash
mosaic session show docs-demo
```

Inspect one trace file:

```bash
mosaic inspect .mosaic/runs/<run-id>.json
```

## Gateway

Show local status:

```bash
mosaic gateway status
```

Serve the HTTP control plane:

```bash
mosaic gateway serve --http 127.0.0.1:8080
```

Watch the local in-process monitor:

```bash
mosaic gateway serve --local
```

List session summaries from the Gateway:

```bash
mosaic gateway sessions
```

View audit events:

```bash
mosaic gateway audit --limit 20
```

View replay window contents:

```bash
mosaic gateway replay --limit 20
```

Export an incident bundle:

```bash
mosaic gateway incident <run-id>
```

## Adapters

Show adapter status:

```bash
mosaic adapter status
```

Run adapter doctor:

```bash
mosaic adapter doctor
```

## Capabilities

List capability job state:

```bash
mosaic capability jobs
```

Show guardrails for `exec`:

```bash
mosaic capability exec guardrails
```

Run a small command through the capability layer:

```bash
mosaic capability exec run ./script.sh --session ops-demo
```

Test a webhook call:

```bash
mosaic capability webhook test http://127.0.0.1:8080 --method GET
```

## Cron

List cron registrations:

```bash
mosaic cron list
```

Register a cron job:

```bash
mosaic cron register every-hour "0 * * * *" "status report" --session ops-demo
```

Trigger one manually:

```bash
mosaic cron trigger every-hour
```

## Nodes

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

## Memory

List sessions with memory:

```bash
mosaic memory list
```

Show one session memory view:

```bash
mosaic memory show ops-demo
```

Search memory:

```bash
mosaic memory search deploy --tag note
```

## Extensions

List extension status:

```bash
mosaic extension list
```

Validate manifests:

```bash
mosaic extension validate
```

Reload manifests:

```bash
mosaic extension reload
```
