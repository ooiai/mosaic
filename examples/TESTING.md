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

`inspect trace`

运行：`cargo run -p mosaic-cli -- inspect .mosaic/runs/<trace-id>.json`
