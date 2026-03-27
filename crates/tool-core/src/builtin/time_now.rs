use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::{Tool, ToolMetadata, ToolResult};

pub struct TimeNowTool {
    meta: ToolMetadata,
}

impl TimeNowTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "time_now",
                "Return the current UTC timestamp",
                serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            ),
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
            is_error: false,
            audit: None,
        })
    }
}
