use std::path::PathBuf;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::process::Command;

use crate::{
    CapabilityAudit, CapabilityKind, CapabilityMetadata, Tool, ToolContext, ToolMetadata,
    ToolResult,
    policy::{canonicalize_user_path, ensure_allowed_path},
};

pub struct ExecTool {
    meta: ToolMetadata,
    allowed_roots: Vec<PathBuf>,
}

impl ExecTool {
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "exec_command",
                "Execute a local command with workspace guardrails",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "cwd": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            )
            .with_capability(CapabilityMetadata::exec().with_node_route(
                "exec_command",
                true,
                false,
            )),
            allowed_roots,
        }
    }
}

#[async_trait]
impl Tool for ExecTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let command = input
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: command"))?
            .to_owned();
        let args = input
            .get("args")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let cwd = input.get("cwd").and_then(serde_json::Value::as_str);
        let resolved_cwd = match cwd {
            Some(value) => {
                let canonical = canonicalize_user_path(std::path::Path::new(value), None)?;
                ensure_allowed_path(&canonical, &self.allowed_roots, "exec_command")?;
                Some(canonical)
            }
            None => ctx.sandbox.as_ref().map(|sandbox| sandbox.workdir.clone()),
        };

        if command.contains(std::path::MAIN_SEPARATOR) || command.contains('/') {
            let canonical =
                canonicalize_user_path(std::path::Path::new(&command), resolved_cwd.as_deref())?;
            ensure_allowed_path(&canonical, &self.allowed_roots, "exec_command")?;
        }

        let mut child = Command::new(&command);
        child.args(&args);
        if let Some(cwd) = &resolved_cwd {
            child.current_dir(cwd);
        }
        let output = child.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let exit_code = output.status.code();
        let success = output.status.success();
        let content = match (stdout.is_empty(), stderr.is_empty()) {
            (false, true) => stdout.clone(),
            (true, false) => stderr.clone(),
            (false, false) => format!("stdout:\n{}\n\nstderr:\n{}", stdout, stderr),
            (true, true) => format!("command exited with code {}", exit_code.unwrap_or_default()),
        };

        Ok(ToolResult {
            content,
            structured: Some(serde_json::json!({
                "command": command,
                "args": args,
                "cwd": resolved_cwd.as_ref().map(|path| path.display().to_string()),
                "sandbox_env_id": ctx.sandbox.as_ref().map(|sandbox| sandbox.env_id.clone()),
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code,
                "success": success,
            })),
            is_error: !success,
            audit: Some(CapabilityAudit {
                kind: CapabilityKind::Exec,
                permission_scopes: self.meta.capability.permission_scopes.clone(),
                risk: self.meta.capability.risk.clone(),
                side_effect_summary: format!(
                    "exec {} finished with code {}",
                    command,
                    exit_code
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "unknown".to_owned())
                ),
                target: Some(command),
                exit_code,
                http_status: None,
            }),
        })
    }
}
