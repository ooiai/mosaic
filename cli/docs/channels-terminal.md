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
```
