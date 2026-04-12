use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;

use crate::{
    CapabilityAudit, CapabilityKind, CapabilityMetadata, Tool, ToolContext, ToolMetadata,
    ToolResult,
    policy::{canonicalize_user_path, ensure_allowed_path},
};

pub struct ReadFileTool {
    meta: ToolMetadata,
    allowed_roots: Vec<PathBuf>,
}

impl ReadFileTool {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().ok().into_iter().collect::<Vec<_>>();
        Self::new_with_allowed_roots(cwd)
    }

    pub fn new_with_allowed_roots(allowed_roots: Vec<PathBuf>) -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "read_file",
                "Read a UTF-8 text file from disk",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
            )
            .with_capability(CapabilityMetadata::file_read().with_node_route(
                "read_file",
                true,
                false,
            )),
            allowed_roots,
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

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let path = input
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("missing required field: path"))?;

        let canonical = canonicalize_user_path(Path::new(path), None)?;
        ensure_allowed_path(&canonical, &self.allowed_roots, "read_file")?;

        if !canonical.exists() {
            bail!("file does not exist: {}", canonical.display());
        }

        if !canonical.is_file() {
            bail!("path is not a file: {}", canonical.display());
        }

        let content = fs::read_to_string(&canonical)?;

        Ok(ToolResult {
            content: content.clone(),
            structured: Some(serde_json::json!({
                "path": canonical.display().to_string(),
                "content": content,
            })),
            is_error: false,
            audit: Some(CapabilityAudit {
                kind: CapabilityKind::File,
                permission_scopes: self.meta.capability.permission_scopes.clone(),
                risk: self.meta.capability.risk.clone(),
                side_effect_summary: format!("read file {}", canonical.display()),
                target: Some(canonical.display().to_string()),
                exit_code: None,
                http_status: None,
            }),
        })
    }
}
