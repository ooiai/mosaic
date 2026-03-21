use std::{collections::HashMap, fs, path::Path, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub content: String,
    pub structured: Option<serde_json::Value>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> &ToolMetadata;
    async fn call(&self, input: serde_json::Value) -> Result<ToolResult>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.metadata().name.clone();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = Arc<dyn Tool>> + '_ {
        self.tools.values().cloned()
    }
}

pub struct EchoTool {
    meta: ToolMetadata,
}

impl EchoTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata {
                name: "echo".to_owned(),
                description: "Echo input as output".to_owned(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                }),
            },
        }
    }
}

impl Default for EchoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EchoTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let text = input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();

        Ok(ToolResult {
            content: text,
            structured: Some(input),
        })
    }
}

pub struct TimeNowTool {
    meta: ToolMetadata,
}

impl TimeNowTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata {
                name: "time_now".to_owned(),
                description: "Return the current UTC timestamp".to_owned(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        }
    }
}

impl Default for TimeNowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TimeNowTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, _input: serde_json::Value) -> Result<ToolResult> {
        let now = Utc::now().to_rfc3339();

        Ok(ToolResult {
            content: now.clone(),
            structured: Some(serde_json::json!({
                "utc": now
            })),
        })
    }
}

pub struct ReadFileTool {
    meta: ToolMetadata,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata {
                name: "read_file".to_owned(),
                description: "Read a UTF-8 text file from disk".to_owned(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
            },
        }
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let path = input
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;

        let path_ref = Path::new(path);

        if !path_ref.exists() {
            anyhow::bail!("file does not exist: {}", path);
        }

        if !path_ref.is_file() {
            anyhow::bail!("path is not a file: {}", path);
        }

        let content = fs::read_to_string(path_ref)?;

        Ok(ToolResult {
            content: content.clone(),
            structured: Some(serde_json::json!({
                "path": path,
                "content": content,
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs, process,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use chrono::DateTime;
    use futures::executor::block_on;

    use super::{EchoTool, ReadFileTool, TimeNowTool, Tool, ToolRegistry};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_file_path(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!(
            "mosaic-tool-core-{label}-{}-{nanos}-{count}.txt",
            process::id()
        ))
    }

    #[test]
    fn builtin_echo_tool_is_registered_and_callable() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool::new()));

        let tool = registry.get("echo").expect("echo tool should exist");
        let result = block_on(tool.call(serde_json::json!({ "text": "hello" })))
            .expect("tool call should succeed");

        assert_eq!(result.content, "hello");
        assert_eq!(registry.list(), vec!["echo".to_owned()]);
    }

    #[test]
    fn time_now_tool_returns_a_rfc3339_utc_timestamp() {
        let result = block_on(TimeNowTool::new().call(serde_json::json!({})))
            .expect("time_now tool should succeed");

        assert!(DateTime::parse_from_rfc3339(&result.content).is_ok());
        assert_eq!(
            result.structured,
            Some(serde_json::json!({ "utc": result.content }))
        );
    }

    #[test]
    fn read_file_tool_reads_utf8_text_files() {
        let path = temp_file_path("read-success");
        fs::write(&path, "hello from file").expect("temp file should be writable");

        let result = block_on(ReadFileTool::new().call(serde_json::json!({
            "path": path.to_string_lossy(),
        })))
        .expect("read_file tool should succeed");

        assert_eq!(result.content, "hello from file");
        assert_eq!(
            result.structured,
            Some(serde_json::json!({
                "path": path.to_string_lossy(),
                "content": "hello from file",
            }))
        );

        fs::remove_file(path).ok();
    }

    #[test]
    fn read_file_tool_rejects_missing_paths() {
        let path = temp_file_path("missing");

        let err = block_on(ReadFileTool::new().call(serde_json::json!({
            "path": path.to_string_lossy(),
        })))
        .expect_err("missing path should fail");

        assert!(err.to_string().contains("file does not exist"));
    }
}
