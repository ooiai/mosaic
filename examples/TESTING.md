# Examples Testing

This file explains how the repo examples map into the j5 test matrix.

## Fast local example checks

These are safety-net flows, not release evidence by themselves.

`basic-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/basic-agent.yaml`

预期：在已配置真实 provider 时成功生成回答，并在 `.mosaic/runs/` 下生成 trace 文件。

`research-skill.yaml`

运行：`cargo run -p mosaic-cli -- run examples/research-skill.yaml --skill summarize`

预期：输出 `summary:` 开头的结果。

`mcp-filesystem.yaml`

运行：`cargo run -p mosaic-cli -- run examples/mcp-filesystem.yaml`

预期：trace 中出现 `source: mcp` 与远端 tool 元数据。

`time-now-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/time-now-agent.yaml`

预期：provider 触发 `time_now`，trace 中有 1 条 `tool_calls`。

`read-file-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/read-file-agent.yaml`

预期：provider 触发 `read_file`，最终输出以 `Tool returned:` 开头。

`inspect trace`

运行：`cargo run -p mosaic-cli -- inspect .mosaic/runs/<trace-id>.json`

## Dev-only full-stack lane

运行：`./scripts/test-full-stack-example.sh mock`

预期：启动本地 HTTP Gateway，经由 Telegram ingress 产生 `telegram--100123-99` session、trace、audit/replay 和 incident bundle。

说明：该 lane 为显式 dev-only mock，不作为 release evidence。

## Automated no-mock release lane

运行：

```bash
MOSAIC_REAL_TESTS=1 OPENAI_API_KEY=... ./scripts/test-full-stack-example.sh openai-webchat
```

预期：

- real OpenAI provider
- real WebChat ingress
- real session / trace / audit / replay / incident artifacts
- `mosaic inspect ... --verbose` 能看到真实 provider 与 runtime policy

## Telegram-first release-blocking lane

运行：参考 `docs/telegram-real-e2e.md`

预期：

- 真实 Telegram bot webhook
- 真实 OpenAI provider
- 真实 `time_now` / `read_file`
- 真实 `summarize_notes` / `summarize_operator_note`
- `session`、`inspect`、`audit`、`replay`、`incident` 一致

## Matrix and release checks

测试矩阵一致性：

```bash
make test-matrix
```

黄金路径文档与示例：

```bash
make test-golden
```

仓库级 smoke：

```bash
make smoke
```

完整发布前检查：

```bash
make release-check
```
