# Upgrade

This guide describes the safe upgrade flow for Mosaic.

## Before you upgrade

Review:

- [compatibility.md](./compatibility.md)
- [operations.md](./operations.md)
- [security.md](./security.md)

## Pre-upgrade checklist

- confirm the current service is healthy with `mosaic gateway status`
- back up `.mosaic/` and the env file
- export incident bundles for any open investigations
- verify the target release completed `make release-check`

## Upgrade steps

1. Stop the service.
2. Install the new binary or unpack the new bundle.
3. Compare `.mosaic/config.yaml` with the new docs and examples.
4. Run:

```bash
mosaic setup validate
mosaic setup doctor
```

5. Start the service.
6. Run:

```bash
make smoke
```

## After you upgrade

Verify:

- `mosaic session list`
- `mosaic session show <session-id>`
- `mosaic gateway status`
- `mosaic gateway audit --limit 20`
- `mosaic inspect .mosaic/runs/<run-id>.json`

## Rollback

If the new binary or config is not acceptable:

1. Stop the service.
2. Restore the previous binary.
3. Restore the previous `.mosaic/` backup and env file if needed.
4. Start the service again.
5. Re-run `mosaic gateway status` and `make smoke`.
