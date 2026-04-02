# Release

This guide defines the release gate for Mosaic as a self-hosted agent control plane.

As of k5, release sign-off is not just `cargo test`. It is a combination of:

- workspace verification
- matrix consistency checks
- automated no-mock real lanes
- operator-manual acceptance when a scoped channel requires it

## Automated gate

Run the delivery gate before cutting a release:

```bash
make release-check
```

The release gate must cover:

```bash
make check
make test
make test-matrix
make test-golden
MOSAIC_REAL_TESTS=1 make test-real
make smoke
make package
```

## Release roles

### Automated release-blocking lanes

These are required on every release:

- `make test-matrix`
- `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-provider --test real_vendors -- --nocapture`
- `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-sdk --test real_gateway_http -- --nocapture`
- `MOSAIC_REAL_TESTS=1 cargo test -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture`
- `MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat`

### Operator-manual release-blocking sign-off

When Telegram is in release scope, this is also required:

- Telegram-first release-blocking acceptance lane
- execute [telegram-real-e2e.md](./telegram-real-e2e.md)
- confirm [telegram-step-by-step.md](./telegram-step-by-step.md), [telegram-real-e2e.md](./telegram-real-e2e.md), and [channels.md](./channels.md) were updated if the release changed Telegram behavior
- confirm [skills.md](./skills.md) and [sandbox.md](./sandbox.md) changed too when Telegram-visible skill or sandbox prerequisites changed
- record the session id, trace path, and incident bundle path
- record the `mosaic adapter telegram webhook info` output used for sign-off
- record which bot or bots were used for sign-off
- record whether the image upload, document upload, and `/mosaic help` catalog discovery lanes were part of the scoped release

### Compatibility addendum lanes

These are real lanes, but they are compatibility evidence rather than the main product story:

- Azure OpenAI vendor lane
- Anthropic vendor lane
- Ollama local real-model lane

Review them whenever the release includes provider-facing changes.

## Release checklist

### 1. Documentation and artifacts

Confirm these are present and up to date:

- `README.md`
- `.env.example`
- `docs/skills.md`
- `docs/sandbox.md`
- `docs/capabilities.md`
- `docs/testing.md`
- `docs/real-vs-mock-acceptance.md`
- `docs/telegram-real-e2e.md`
- `docs/telegram-step-by-step.md`
- `docs/channels.md`
- `docs/providers.md`
- `docs/release.md`
- `examples/full-stack/openai-webchat.config.yaml`
- `examples/skills/README.md`
- `examples/capabilities/README.md`
- `examples/sandbox/README.md`
- `examples/composition/README.md`
- `examples/skills/native-skill.yaml`
- `examples/skills/manifest-skill.yaml`
- `examples/skills/operator-note/SKILL.md`
- `examples/capabilities/builtin-tool.yaml`
- `examples/capabilities/node-routed-tool.yaml`
- `examples/capabilities/workflow.yaml`
- `examples/sandbox/python-markdown-skill-pack.yaml`
- `examples/sandbox/node-manifest-skill.yaml`
- `examples/composition/openai-capability-composition.config.yaml`
- `examples/full-stack/openai-telegram-single-bot.config.yaml`
- `examples/full-stack/openai-telegram-e2e.config.yaml`
- `examples/full-stack/openai-telegram-multi-bot.config.yaml`
- `examples/full-stack/openai-telegram-multimodal.config.yaml`
- `examples/full-stack/openai-telegram-bot-split.config.yaml`
- `examples/extensions/telegram-e2e.yaml`
- `scripts/test-real-integrations.sh`
- `scripts/test-full-stack-example.sh`
- `scripts/verify-test-matrix.sh`

### 2. Workspace verification

Run:

```bash
make check
make test
make test-matrix
make test-golden
```

### 3. Real automated acceptance

Run:

```bash
MOSAIC_REAL_TESTS=1 make test-real
```

This should prove:

- the OpenAI provider-real lane
- the Gateway real HTTP + SSE lane
- the MCP real stdio lane
- the OpenAI + WebChat product-real lane

### 4. Operator-manual sign-off

If Telegram is in the release scope, also run the full Telegram-first acceptance flow:

```bash
See docs/telegram-real-e2e.md
```

At minimum, re-check:

- `mosaic setup validate`
- `mosaic setup doctor`
- `mosaic adapter status`
- `mosaic adapter telegram webhook info`
- `mosaic adapter telegram webhook info --bot <name>`
- `mosaic adapter telegram test-send --chat-id <chat-id> "mosaic outbound smoke"`
- `mosaic adapter telegram test-send --bot <name> --chat-id <chat-id> "mosaic outbound smoke"`
- `mosaic session show <session-id>`
- `mosaic inspect .mosaic/runs/<run-id>.json --verbose`
- `mosaic gateway incident <run-id>`

Telegram-affecting work is not release-ready unless the matching Telegram docs and examples changed in the same change set.

Also confirm the operator can answer:

- whether a failed run came from provider, tool, MCP, node, or sandbox
- where a skill came from
- why a capability was visible
- which sandbox env was selected

### 5. Packaging

Build the release bundle:

```bash
make package
```

Verify that the tarball under `dist/` contains the binary, docs, examples, and `.env.example`.

### 6. Version and compatibility review

Review these before publishing release notes:

- [testing.md](./testing.md)
- [real-vs-mock-acceptance.md](./real-vs-mock-acceptance.md)
- [providers.md](./providers.md)
- [compatibility.md](./compatibility.md)
- [upgrade.md](./upgrade.md)
