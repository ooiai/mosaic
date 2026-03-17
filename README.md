# Mosaic CLI

Rust-first local agent runtime and operations CLI.

Mosaic is a standalone rewrite focused on a production-usable CLI core: agent loop, tools, channels, gateway ops, policy/sandbox, and diagnostics. Web/Desktop UI is explicitly out of this phase.

- Docs site: https://ooiai.github.io/mosaic/
- Guide hub: https://ooiai.github.io/mosaic/guide.html
- Quickstart tutorial: https://ooiai.github.io/mosaic/quickstart.html
- Learning path tutorial: https://ooiai.github.io/mosaic/learning-path.html
- Models & profiles tutorial: https://ooiai.github.io/mosaic/models-profiles.html
- Sessions tutorial: https://ooiai.github.io/mosaic/sessions.html
- TUI tutorial: https://ooiai.github.io/mosaic/tui.html
- Policy tutorial (approvals + sandbox): https://ooiai.github.io/mosaic/policy.html
- Azure end-to-end ops playbook: https://ooiai.github.io/mosaic/playbook-azure-ops.html
- Agents module tutorial: https://ooiai.github.io/mosaic/agents.html
- Channels module tutorial: https://ooiai.github.io/mosaic/channels.html
- Memory module tutorial: https://ooiai.github.io/mosaic/memory.html
- Knowledge module tutorial: https://ooiai.github.io/mosaic/knowledge.html
- Plugins module tutorial: https://ooiai.github.io/mosaic/plugins.html
- Skills module tutorial: https://ooiai.github.io/mosaic/skills.html
- Gateway module tutorial: https://ooiai.github.io/mosaic/gateway.html
- Gateway Call API tutorial: https://ooiai.github.io/mosaic/gateway-call.html
- Operations tutorial: https://ooiai.github.io/mosaic/operations.html
- Regression tutorial: https://ooiai.github.io/mosaic/regression.html
- Chinese docs site: https://ooiai.github.io/mosaic/cn/
- Chinese README: `README_CN.md`
- Legacy long docs: `README.legacy.md`, `README_CN.legacy.md`

## Current Scope

- Core: `setup`, `configure`, `models`, `ask`, `chat`, `tui`, `session`
- Ops: `status`, `health`, `doctor`, `logs`, `system`, `dashboard`
- Gateway: `gateway install|start|status|probe|discover|call|stop|uninstall`
- Channels: `add|update|list|status|test|send|logs|capabilities|resolve|remove|logout`
- Policy runtime: `approvals`, `sandbox`, `safety`
- Extended modules: `mcp`, `memory`, `knowledge`, `security`, `plugins`, `skills`, `agents`, `nodes`, `devices`, `pairing`, `hooks`, `cron`, `webhooks`, `browser`, `tts`, `voicecall`

## Install

### macOS (Homebrew)

```bash
brew tap ooiai/mosaic https://github.com/ooiai/mosaic
brew install mosaic
```

### Linux / macOS (source installer)

```bash
curl -fsSL https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.sh | bash -s -- --from-source
```

### Windows (PowerShell, source installer)

```powershell
irm https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.ps1 -OutFile install.ps1
powershell -ExecutionPolicy Bypass -File .\install.ps1 -FromSource
```

Verify:

```bash
mosaic --version
mosaic --help
```

## 2-Minute Quick Start

```bash
# 1) configure project-local state
mosaic --project-state setup \
  --base-url https://api.openai.com \
  --api-key-env OPENAI_API_KEY \
  --model gpt-4o-mini

# 2) list models
mosaic --project-state models list

# 3) one-shot ask
mosaic --project-state ask "summarize this repository"

# 4) interactive chat
mosaic --project-state chat

# 4b) chat-first terminal UI
mosaic --project-state

# 4c) explicit TUI entry still works
mosaic --project-state tui
```

## Workspace

- CLI workspace: `cli/`
- Main binary crate: `cli/crates/mosaic-cli`
- Core docs: `cli/README.md`
- Distribution docs: `cli/docs/distribution.md`
- Parity/progress: `planing.md`

## Development

```bash
# fast quality gate
make cli-quality

# full CLI tests
make cli-test

# full regression
make cli-regression

# release tooling smoke (verifier pass/fail + dry-run summary)
make cli-release-tooling-smoke v=v0.2.0-beta.6

# release installer smoke (install.sh local assets + release-only guardrail)
make cli-release-install-smoke v=v0.2.0-beta.6

# release notes draft from worklog
make cli-release-notes v=v0.2.0-beta.6

# one-command local release prepare
make cli-release-prepare v=v0.2.0-beta.6

# verify archive internal layout (binary/docs/installers)
make cli-release-verify-archives v=v0.2.0-beta.6 assets=../release-assets

# verify release assets directory
make cli-release-verify v=v0.2.0-beta.6 assets=../release-assets

# verify published GitHub release by tag
make cli-release-publish-check v=v0.2.0-beta.6 repo=ooiai/mosaic
```

## License

MIT
