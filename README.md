# Mosaic CLI

Rust-first local agent runtime and operations CLI.

Mosaic is a standalone rewrite focused on a production-usable CLI core: agent loop, tools, channels, gateway ops, policy/sandbox, and diagnostics. Web/Desktop UI is explicitly out of this phase.

- Docs site: https://ooiai.github.io/mosaic/
- Full usage tutorial: https://ooiai.github.io/mosaic/guide.html
- Chinese docs site: https://ooiai.github.io/mosaic/cn/
- 中文完整教程: https://ooiai.github.io/mosaic/cn/guide.html
- Chinese README: `README_CN.md`
- Legacy long docs: `README.legacy.md`, `README_CN.legacy.md`

## Current Scope

- Core: `setup`, `configure`, `models`, `ask`, `chat`, `session`
- Ops: `status`, `health`, `doctor`, `logs`, `system`, `dashboard`
- Gateway: `gateway install|start|status|probe|discover|call|stop|uninstall`
- Channels: `add|update|list|status|test|send|logs|capabilities|resolve|remove|logout`
- Policy runtime: `approvals`, `sandbox`, `safety`
- Extended modules: `mcp`, `memory`, `security`, `plugins`, `skills`, `agents`, `nodes`, `devices`, `pairing`, `hooks`, `cron`, `webhooks`, `browser`, `tts`, `voicecall`

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
```

## License

MIT
