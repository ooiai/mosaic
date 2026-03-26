# Release

This guide defines the release checklist for Mosaic as a deliverable self-hosted product.

## Automated gate

Run the delivery gate before cutting a release:

```bash
make release-check
```

This currently verifies:

- workspace build
- workspace check
- workspace tests
- delivery artifact presence
- isolated smoke flow in a temporary workspace

## Release checklist

### 1. Documentation and artifacts

Confirm these are present and up to date:

- `README.md`
- `.env.example`
- `docs/deployment.md`
- `docs/operations.md`
- `docs/security.md`
- `docs/release.md`
- `docs/compatibility.md`
- `docs/upgrade.md`
- `examples/deployment/production.config.yaml`
- `examples/deployment/mosaic.service`

### 2. Validation and smoke

Run:

```bash
make check
make test
make smoke
```

### 3. Packaging

Build the release bundle:

```bash
make package
```

Verify that the tarball under `dist/` contains the binary, docs, examples, and `.env.example`.

### 4. Manual signoff

At minimum, re-check:

- `mosaic setup init`
- `mosaic setup validate`
- `mosaic setup doctor`
- `mosaic tui`
- `mosaic session list`
- `mosaic gateway status`
- `mosaic gateway incident <run-id>`
- `mosaic inspect .mosaic/runs/<run-id>.json`

### 5. Version and compatibility review

Review [compatibility.md](./compatibility.md) and [upgrade.md](./upgrade.md) before publishing release notes.
