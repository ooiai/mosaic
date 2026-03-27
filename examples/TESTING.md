# Examples Testing

## Fast example checks

`basic-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/basic-agent.yaml`

预期：输出 `mock response: Explain what Mosaic is.`，并在 `.mosaic/runs/` 下生成 trace 文件。

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

`full-stack mock`

运行：`./scripts/test-full-stack-example.sh mock`

预期：启动本地 HTTP Gateway，经由 Telegram ingress 产生 `telegram--100123-99` session、trace、audit/replay 和 incident bundle。

## Delivery smoke

仓库级 smoke：

```bash
make smoke
```

完整发布前检查：

```bash
make release-check
```
