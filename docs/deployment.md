# Deployment

This guide describes the production-oriented deployment path for Mosaic as a self-hosted service.

## Delivery options

### Install from source

```bash
cargo install --path cli
```

Use this when you build Mosaic directly on the target host.

### Run from a checked-out workspace

```bash
cargo run -p mosaic-cli -- --help
```

Use this for development, staging, or controlled operator environments.

### Build a release bundle

```bash
make package
```

This creates a tarball under `dist/` containing:

- the `mosaic` binary
- `README.md`
- `docs/`
- `examples/`
- `.env.example`

## Recommended host layout

```text
/opt/mosaic/
  bin/mosaic
  .mosaic/config.yaml
  .mosaic/sessions/
  .mosaic/runs/
  .mosaic/audit/
  .mosaic/extensions/
  .mosaic/memory/
  .mosaic/nodes/
  .mosaic/cron/
/etc/mosaic/mosaic.env
```

## Production config starter

Start from:

- [examples/deployment/production.config.yaml](../examples/deployment/production.config.yaml)
- [`.env.example`](../.env.example)

Copy the config into the service workspace:

```bash
mkdir -p /opt/mosaic/.mosaic
cp examples/deployment/production.config.yaml /opt/mosaic/.mosaic/config.yaml
cp .env.example /etc/mosaic/mosaic.env
```

Then edit the provider profile, workspace name, and auth env names for your environment.

## Service management

### systemd

Example unit: [examples/deployment/mosaic.service](../examples/deployment/mosaic.service)

Recommended flow:

```bash
sudo cp examples/deployment/mosaic.service /etc/systemd/system/mosaic.service
sudo systemctl daemon-reload
sudo systemctl enable mosaic
sudo systemctl start mosaic
```

The example unit serves the HTTP Gateway on `127.0.0.1:8080` and expects the env file at `/etc/mosaic/mosaic.env`.

## Post-deploy checks

Run these after first boot:

```bash
mosaic setup validate
mosaic setup doctor
mosaic gateway status
mosaic model list
```

If the Gateway is exposed over HTTP, verify the control plane health endpoint from the host:

```bash
curl http://127.0.0.1:8080/health
```

## Local vs production defaults

Use `deployment.profile: local` when you are exploring the repo.

Use `deployment.profile: production` when you want doctor and validation to enforce:

- operator auth token configuration
- audit input redaction
- explicit deployment metadata

Continue with [operations.md](./operations.md) and [security.md](./security.md) before putting Mosaic behind a long-lived service manager.
