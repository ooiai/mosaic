# Mosaic CLI

一个以 Rust 为核心的本地 Agent 运行时与运维 CLI。

Mosaic 当前是独立重写路线，重点是“可真实使用的 CLI 核心能力”：Agent 回路、工具系统、channels、gateway 运维、策略沙箱、诊断观测。此阶段不做 Web/Desktop UI。

- 文档站点: https://ooiai.github.io/mosaic/cn/
- 中文路径导航: https://ooiai.github.io/mosaic/cn/guide.html
- 中文 10 分钟上手: https://ooiai.github.io/mosaic/cn/quickstart.html
- 中文分阶段学习: https://ooiai.github.io/mosaic/cn/learning-path.html
- 中文 Models 与 Profiles 教程: https://ooiai.github.io/mosaic/cn/models-profiles.html
- 中文 Sessions 教程: https://ooiai.github.io/mosaic/cn/sessions.html
- 中文策略教程: https://ooiai.github.io/mosaic/cn/policy.html
- 中文 Azure 运维剧本: https://ooiai.github.io/mosaic/cn/playbook-azure-ops.html
- 中文 Agents 教程: https://ooiai.github.io/mosaic/cn/agents.html
- 中文 Channels 教程: https://ooiai.github.io/mosaic/cn/channels.html
- 中文 Memory 教程: https://ooiai.github.io/mosaic/cn/memory.html
- 中文 Knowledge 教程: https://ooiai.github.io/mosaic/cn/knowledge.html
- 中文 Plugins 教程: https://ooiai.github.io/mosaic/cn/plugins.html
- 中文 Skills 教程: https://ooiai.github.io/mosaic/cn/skills.html
- 中文 Gateway 教程: https://ooiai.github.io/mosaic/cn/gateway.html
- 中文 Gateway Call API: https://ooiai.github.io/mosaic/cn/gateway-call.html
- 中文生产运维教程: https://ooiai.github.io/mosaic/cn/operations.html
- 中文回归测试教程: https://ooiai.github.io/mosaic/cn/regression.html
- 英文站点: https://ooiai.github.io/mosaic/
- Guide hub: https://ooiai.github.io/mosaic/guide.html
- Quickstart tutorial: https://ooiai.github.io/mosaic/quickstart.html
- Learning path tutorial: https://ooiai.github.io/mosaic/learning-path.html
- Models & profiles tutorial: https://ooiai.github.io/mosaic/models-profiles.html
- Sessions tutorial: https://ooiai.github.io/mosaic/sessions.html
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
- 英文 README: `README.md`
- 旧版长文档备份: `README.legacy.md`, `README_CN.legacy.md`

## 当前能力范围

- 核心命令: `setup`, `configure`, `models`, `ask`, `chat`, `session`
- 运维命令: `status`, `health`, `doctor`, `logs`, `system`, `dashboard`
- Gateway: `gateway install|start|status|probe|discover|call|stop|uninstall`
- Channels: `add|update|list|status|test|send|logs|capabilities|resolve|remove|logout`
- 策略运行时: `approvals`, `sandbox`, `safety`
- 扩展模块: `mcp`, `memory`, `knowledge`, `security`, `plugins`, `skills`, `agents`, `nodes`, `devices`, `pairing`, `hooks`, `cron`, `webhooks`, `browser`, `tts`, `voicecall`

## 安装

### macOS (Homebrew)

```bash
brew tap ooiai/mosaic https://github.com/ooiai/mosaic
brew install mosaic
```

### Linux / macOS（源码安装脚本）

```bash
curl -fsSL https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.sh | bash -s -- --from-source
```

### Windows（PowerShell，源码安装脚本）

```powershell
irm https://raw.githubusercontent.com/ooiai/mosaic/main/cli/install.ps1 -OutFile install.ps1
powershell -ExecutionPolicy Bypass -File .\install.ps1 -FromSource
```

验证:

```bash
mosaic --version
mosaic --help
```

## 2 分钟上手

```bash
# 1) 初始化项目内状态
mosaic --project-state setup \
  --base-url https://api.openai.com \
  --api-key-env OPENAI_API_KEY \
  --model gpt-4o-mini

# 2) 列模型
mosaic --project-state models list

# 3) 单轮问答
mosaic --project-state ask "summarize this repository"

# 4) 进入 REPL
mosaic --project-state chat
```

## 代码结构

- CLI 工作区: `cli/`
- 主二进制 crate: `cli/crates/mosaic-cli`
- 核心文档: `cli/README.md`
- 分发文档: `cli/docs/distribution.md`
- 对齐与进度: `planing.md`

## 开发命令

```bash
# 快速质量门
make cli-quality

# CLI 全量测试
make cli-test

# 回归套件
make cli-regression
```

## License

MIT
