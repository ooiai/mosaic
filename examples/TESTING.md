# Examples Testing

`basic-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/basic-agent.yaml`

预期：输出 `mock response: Explain what Mosaic is.`，并在 `.mosaic/runs/` 下生成 trace 文件。

`research-skill.yaml`

运行：`cargo run -p mosaic-cli -- run examples/research-skill.yaml --skill summarize`

预期：输出 `summary: Rust async allows efficient concurrency with futures and executors.`。

`mcp-filesystem.yaml`

运行：`cargo run -p mosaic-cli -- run examples/mcp-filesystem.yaml`

说明：当前阶段只验证配置骨架，MCP transport 还没有接通，因此不会真正调用远端 MCP server。

`time-now.yaml`

运行：`cargo run -p mosaic-cli -- run examples/time-now.yaml`

说明：当前 `MockProvider` 还不会自动触发 tool call，这个示例主要用于验证 `time_now` 能力已可注册。

`time-now-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/time-now-agent.yaml`

预期：`MockProvider` 会先触发 `time_now`，runtime 执行工具后再返回最终答案；trace 里应有 1 条 `tool_calls`。

`read-file-agent.yaml`

运行：`cargo run -p mosaic-cli -- run examples/read-file-agent.yaml`

预期：`MockProvider` 会触发 `read_file`；在仓库根目录运行时会读取 `README.md`，最终输出应以 `Tool returned:` 开头，trace 里应有 1 条带 `call_id` 的 `tool_calls`。

`inspect trace`

运行：`cargo run -p mosaic-cli -- inspect .mosaic/runs/<trace-id>.json`
