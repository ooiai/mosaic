# Channels: Terminal (Beta)

`terminal` is the local sink channel for CLI-only workflows.

## 1) Add channel

```bash
mosaic --project-state channels add \
  --name local-terminal \
  --kind terminal
```

Alias forms are supported:
- `--kind local`
- `--kind stdout`

## 2) Test and send

```bash
mosaic --project-state --json channels test <channel-id>
mosaic --project-state --json channels send <channel-id> --text "build finished"
```

## 3) Output fields

- `kind` resolves to `terminal` even when added via aliases.
- `target_masked` is `terminal://local`.
- `endpoint_masked` is `null`.

## 4) Notes

- No endpoint is required.
- No token is required.
- `--parse-mode` is not supported for terminal channels.

Capability discovery:

```bash
mosaic --project-state --json channels capabilities --channel terminal
mosaic --project-state --json channels capabilities --target <channel-id>
```

## 6) Replay plan for failed deliveries

```bash
mosaic --project-state --json channels replay <channel-id> --tail 50 --limit 5
```

Notes:
- `channels replay` returns replay candidates for failed events.
- Add `--since-minutes <N>` to focus replay analysis on a recent time window.
- Default output keeps retryable failures only.
- Add `--include-non-retryable` for full failure inspection.
- Add repeatable `--reason <rate_limited|upstream_5xx|timeout|auth|target_not_found|client_4xx|unknown>` to focus replay candidates.
- Add repeatable `--http-status <code>` and `--min-attempt <N>` for extra filtering.
- Add `--batch-size <N>` to emit grouped replay plan batches.
- Add `--apply` to replay from stored full payload when available (`replay_source=full_payload`).
- `--apply` runs a readiness preflight and blocks early when channel runtime config is not ready (`channels capabilities --target <channel-id>`).
- Add `--max-apply <N>` with `--apply` to cap execution batch size.
- For legacy events without payload, replay falls back to `text_preview` (`replay_source=text_preview_fallback`) and reports a warning.
- Add `--require-full-payload` with `--apply` to reject replay when only preview fallback is available.
- Add `--stop-on-error` with `--apply` to stop replay after the first failed send.
- Add `--report-out <path>` to persist replay result JSON.

`--target` includes per-channel `diagnostics` and should report `ready_for_send=true` for a valid terminal channel.
