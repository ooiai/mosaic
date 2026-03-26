use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use chrono::Utc;
use mosaic_scheduler_core::{CronRegistration, CronStore};
use reqwest::{
    Client, Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

fn default_true() -> bool {
    true
}

fn default_compatibility_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolSource {
    Builtin,
    Mcp { server: String, remote_tool: String },
}

impl Default for ToolSource {
    fn default() -> Self {
        Self::Builtin
    }
}

impl ToolSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Mcp { .. } => "mcp",
        }
    }

    pub fn server_name(&self) -> Option<&str> {
        match self {
            Self::Builtin => None,
            Self::Mcp { server, .. } => Some(server),
        }
    }

    pub fn remote_tool_name(&self) -> Option<&str> {
        match self {
            Self::Builtin => None,
            Self::Mcp { remote_tool, .. } => Some(remote_tool),
        }
    }
}

pub fn mcp_tool_name(server: &str, remote_tool: &str) -> String {
    format!("mcp.{server}.{remote_tool}")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Utility,
    File,
    Exec,
    Webhook,
    Cron,
    Browser,
    Pdf,
    Image,
    Canvas,
}

impl CapabilityKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Utility => "utility",
            Self::File => "file",
            Self::Exec => "exec",
            Self::Webhook => "webhook",
            Self::Cron => "cron",
            Self::Browser => "browser",
            Self::Pdf => "pdf",
            Self::Image => "image",
            Self::Canvas => "canvas",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionScope {
    LocalRead,
    LocalExec,
    NetworkOutbound,
    ScheduleWrite,
    BrowserAutomation,
    PdfRead,
    ImageRead,
    CanvasWrite,
}

impl PermissionScope {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LocalRead => "local_read",
            Self::LocalExec => "local_exec",
            Self::NetworkOutbound => "network_outbound",
            Self::ScheduleWrite => "schedule_write",
            Self::BrowserAutomation => "browser_automation",
            Self::PdfRead => "pdf_read",
            Self::ImageRead => "image_read",
            Self::CanvasWrite => "canvas_write",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
}

impl ToolRiskLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionPolicy {
    pub timeout_ms: u64,
    pub retry_limit: u8,
    pub budget_units: u32,
}

impl Default for ToolExecutionPolicy {
    fn default() -> Self {
        Self {
            timeout_ms: 5_000,
            retry_limit: 0,
            budget_units: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityMetadata {
    pub kind: CapabilityKind,
    #[serde(default)]
    pub permission_scopes: Vec<PermissionScope>,
    pub risk: ToolRiskLevel,
    #[serde(default)]
    pub execution: ToolExecutionPolicy,
    #[serde(default = "default_true")]
    pub authorized: bool,
    #[serde(default = "default_true")]
    pub healthy: bool,
    #[serde(default)]
    pub long_running: bool,
}

impl Default for CapabilityMetadata {
    fn default() -> Self {
        Self::utility()
    }
}

impl CapabilityMetadata {
    pub fn utility() -> Self {
        Self {
            kind: CapabilityKind::Utility,
            permission_scopes: Vec::new(),
            risk: ToolRiskLevel::Low,
            execution: ToolExecutionPolicy::default(),
            authorized: true,
            healthy: true,
            long_running: false,
        }
    }

    pub fn file_read() -> Self {
        Self {
            kind: CapabilityKind::File,
            permission_scopes: vec![PermissionScope::LocalRead],
            risk: ToolRiskLevel::Low,
            execution: ToolExecutionPolicy {
                timeout_ms: 3_000,
                retry_limit: 0,
                budget_units: 0,
            },
            authorized: true,
            healthy: true,
            long_running: false,
        }
    }

    pub fn exec() -> Self {
        Self {
            kind: CapabilityKind::Exec,
            permission_scopes: vec![PermissionScope::LocalExec],
            risk: ToolRiskLevel::High,
            execution: ToolExecutionPolicy {
                timeout_ms: 10_000,
                retry_limit: 0,
                budget_units: 10,
            },
            authorized: true,
            healthy: true,
            long_running: false,
        }
    }

    pub fn webhook() -> Self {
        Self {
            kind: CapabilityKind::Webhook,
            permission_scopes: vec![PermissionScope::NetworkOutbound],
            risk: ToolRiskLevel::Medium,
            execution: ToolExecutionPolicy {
                timeout_ms: 15_000,
                retry_limit: 1,
                budget_units: 5,
            },
            authorized: true,
            healthy: true,
            long_running: false,
        }
    }

    pub fn cron() -> Self {
        Self {
            kind: CapabilityKind::Cron,
            permission_scopes: vec![PermissionScope::ScheduleWrite],
            risk: ToolRiskLevel::Medium,
            execution: ToolExecutionPolicy {
                timeout_ms: 3_000,
                retry_limit: 0,
                budget_units: 1,
            },
            authorized: true,
            healthy: true,
            long_running: true,
        }
    }

    pub fn abstraction(kind: CapabilityKind) -> Self {
        Self {
            kind,
            permission_scopes: Vec::new(),
            risk: ToolRiskLevel::Medium,
            execution: ToolExecutionPolicy::default(),
            authorized: false,
            healthy: false,
            long_running: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityAudit {
    pub kind: CapabilityKind,
    #[serde(default)]
    pub permission_scopes: Vec<PermissionScope>,
    pub risk: ToolRiskLevel,
    pub side_effect_summary: String,
    pub target: Option<String>,
    pub exit_code: Option<i32>,
    pub http_status: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCompatibility {
    #[serde(default = "default_compatibility_schema")]
    pub schema_version: u32,
}

impl Default for ToolCompatibility {
    fn default() -> Self {
        Self {
            schema_version: default_compatibility_schema(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub source: ToolSource,
    #[serde(default)]
    pub capability: CapabilityMetadata,
    pub extension: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub compatibility: ToolCompatibility,
}

impl ToolMetadata {
    pub fn builtin(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            source: ToolSource::Builtin,
            capability: CapabilityMetadata::utility(),
            extension: None,
            version: None,
            compatibility: ToolCompatibility::default(),
        }
    }

    pub fn mcp(
        server: impl Into<String>,
        remote_tool: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        let server = server.into();
        let remote_tool = remote_tool.into();

        Self {
            name: mcp_tool_name(&server, &remote_tool),
            description: description.into(),
            input_schema,
            source: ToolSource::Mcp {
                server,
                remote_tool,
            },
            capability: CapabilityMetadata::utility(),
            extension: None,
            version: None,
            compatibility: ToolCompatibility::default(),
        }
    }

    pub fn with_capability(mut self, capability: CapabilityMetadata) -> Self {
        self.capability = capability;
        self
    }

    pub fn with_extension(
        mut self,
        extension: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.extension = Some(extension.into());
        self.version = Some(version.into());
        self
    }

    pub fn with_compatibility(mut self, compatibility: ToolCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn is_compatible_with_schema(&self, schema_version: u32) -> bool {
        self.compatibility.schema_version == schema_version
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub content: String,
    pub structured: Option<serde_json::Value>,
    #[serde(default)]
    pub is_error: bool,
    pub audit: Option<CapabilityAudit>,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            structured: None,
            is_error: false,
            audit: None,
        }
    }
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

    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.remove(name)
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
            meta: ToolMetadata::builtin(
                "echo",
                "Echo input as output",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                }),
            ),
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
            is_error: false,
            audit: None,
        })
    }
}

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
            .with_capability(CapabilityMetadata::file_read()),
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

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
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
            .with_capability(CapabilityMetadata::exec()),
            allowed_roots,
        }
    }
}

#[async_trait]
impl Tool for ExecTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
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
                let canonical = canonicalize_user_path(Path::new(value), None)?;
                ensure_allowed_path(&canonical, &self.allowed_roots, "exec_command")?;
                Some(canonical)
            }
            None => None,
        };

        if command.contains(std::path::MAIN_SEPARATOR) || command.contains('/') {
            let canonical = canonicalize_user_path(Path::new(&command), resolved_cwd.as_deref())?;
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
            (false, false) => format!(
                "stdout:
{}

stderr:
{}",
                stdout, stderr
            ),
            (true, true) => format!("command exited with code {}", exit_code.unwrap_or_default()),
        };

        Ok(ToolResult {
            content,
            structured: Some(serde_json::json!({
                "command": command,
                "args": args,
                "cwd": resolved_cwd.as_ref().map(|path| path.display().to_string()),
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

pub struct WebhookTool {
    meta: ToolMetadata,
    client: Client,
}

impl WebhookTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "webhook_call",
                "Dispatch an outbound HTTP webhook request",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" },
                        "method": { "type": "string" },
                        "body": { "type": "string" },
                        "headers": {
                            "type": "object",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["url"]
                }),
            )
            .with_capability(CapabilityMetadata::webhook()),
            client: Client::new(),
        }
    }
}

impl Default for WebhookTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebhookTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let url = input
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: url"))?
            .to_owned();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("webhook url must start with http:// or https://");
        }

        let method = input
            .get("method")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("POST")
            .parse::<Method>()?;
        let mut headers = HeaderMap::new();
        if let Some(raw_headers) = input.get("headers").and_then(serde_json::Value::as_object) {
            for (name, value) in raw_headers {
                let value = value
                    .as_str()
                    .ok_or_else(|| anyhow!("webhook header values must be strings"))?;
                headers.insert(
                    HeaderName::from_bytes(name.as_bytes())?,
                    HeaderValue::from_str(value)?,
                );
            }
        }
        let body = input
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();

        let response = self
            .client
            .request(method.clone(), &url)
            .headers(headers)
            .body(body.clone())
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        Ok(ToolResult {
            content: text.clone(),
            structured: Some(serde_json::json!({
                "url": url,
                "method": method.as_str(),
                "status": status.as_u16(),
                "body": text,
                "request_body": body,
            })),
            is_error: !status.is_success(),
            audit: Some(CapabilityAudit {
                kind: CapabilityKind::Webhook,
                permission_scopes: self.meta.capability.permission_scopes.clone(),
                risk: self.meta.capability.risk.clone(),
                side_effect_summary: format!(
                    "webhook {} {} -> {}",
                    method.as_str(),
                    url,
                    status.as_u16()
                ),
                target: Some(url),
                exit_code: None,
                http_status: Some(status.as_u16()),
            }),
        })
    }
}

pub struct CronRegisterTool {
    meta: ToolMetadata,
    store: Arc<dyn CronStore>,
}

impl CronRegisterTool {
    pub fn new(store: Arc<dyn CronStore>) -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "cron_register",
                "Register a cron job in the local gateway scheduler",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "schedule": { "type": "string" },
                        "input": { "type": "string" },
                        "session_id": { "type": "string" },
                        "profile": { "type": "string" },
                        "skill": { "type": "string" },
                        "workflow": { "type": "string" }
                    },
                    "required": ["id", "schedule", "input"]
                }),
            )
            .with_capability(CapabilityMetadata::cron()),
            store,
        }
    }
}

#[async_trait]
impl Tool for CronRegisterTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let id = input
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: id"))?
            .to_owned();
        let schedule = input
            .get("schedule")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: schedule"))?
            .to_owned();
        let message = input
            .get("input")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: input"))?
            .to_owned();

        let mut registration = CronRegistration::new(id.clone(), schedule.clone(), message.clone());
        registration.session_id = input
            .get("session_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        registration.profile = input
            .get("profile")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        registration.skill = input
            .get("skill")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        registration.workflow = input
            .get("workflow")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        self.store.save(&registration)?;

        Ok(ToolResult {
            content: format!("registered cron {}", id),
            structured: Some(serde_json::to_value(&registration)?),
            is_error: false,
            audit: Some(CapabilityAudit {
                kind: CapabilityKind::Cron,
                permission_scopes: self.meta.capability.permission_scopes.clone(),
                risk: self.meta.capability.risk.clone(),
                side_effect_summary: format!("registered cron {} on {}", id, schedule),
                target: Some(id),
                exit_code: None,
                http_status: None,
            }),
        })
    }
}

fn canonicalize_user_path(path: &Path, base: Option<&Path>) -> Result<PathBuf> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match base {
            Some(base) => base.join(path),
            None => std::env::current_dir()?.join(path),
        }
    };

    if resolved.exists() {
        Ok(resolved.canonicalize()?)
    } else {
        Ok(resolved)
    }
}

fn ensure_allowed_path(path: &Path, allowed_roots: &[PathBuf], tool_name: &str) -> Result<()> {
    if allowed_roots.is_empty() {
        return Ok(());
    }

    let candidate = if path.exists() {
        path.canonicalize()?
    } else {
        path.to_path_buf()
    };
    let allowed = allowed_roots.iter().filter_map(|root| {
        if root.exists() {
            root.canonicalize().ok()
        } else {
            Some(root.clone())
        }
    });

    if allowed.into_iter().any(|root| candidate.starts_with(&root)) {
        return Ok(());
    }

    bail!(
        "{} path is outside the allowed capability roots: {}",
        tool_name,
        candidate.display()
    )
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
    use mosaic_scheduler_core::{CronStore, FileCronStore};

    use super::{
        CapabilityKind, CapabilityMetadata, CronRegisterTool, EchoTool, ExecTool, ReadFileTool,
        TimeNowTool, Tool, ToolMetadata, ToolRegistry, ToolSource, mcp_tool_name,
    };

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

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!(
            "mosaic-tool-core-{label}-{}-{nanos}-{count}",
            process::id()
        ))
    }

    #[test]
    fn builtin_echo_tool_is_registered_and_callable() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool::new()));

        let tool = registry.get("echo").expect("echo tool should exist");
        let result = block_on(tool.call(serde_json::json!({ "text": "hello" })))
            .expect("echo tool should succeed");

        assert_eq!(result.content, "hello");
        assert_eq!(
            result.structured,
            Some(serde_json::json!({ "text": "hello" }))
        );
        assert!(!result.is_error);
    }

    #[test]
    fn time_now_tool_returns_iso_timestamp() {
        let tool = TimeNowTool::new();
        let result =
            block_on(tool.call(serde_json::json!({}))).expect("time_now tool should succeed");

        let parsed = DateTime::parse_from_rfc3339(&result.content)
            .expect("time_now tool should return RFC3339 timestamp");

        assert_eq!(
            parsed.with_timezone(&chrono::Utc).to_rfc3339(),
            result.content
        );
        assert!(!result.is_error);
    }

    #[test]
    fn read_file_tool_reads_text_files_within_allowed_root() {
        let dir = temp_dir("read-file");
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = dir.join("example.txt");
        fs::write(&path, "hello from file").expect("temp file should be written");

        let tool = ReadFileTool::new_with_allowed_roots(vec![dir.clone()]);
        let result = block_on(tool.call(serde_json::json!({
            "path": path.display().to_string()
        })))
        .expect("read_file tool should succeed");

        assert_eq!(result.content, "hello from file");
        assert_eq!(
            result.audit.expect("audit should exist").kind,
            CapabilityKind::File
        );
    }

    #[test]
    fn read_file_tool_rejects_paths_outside_allowed_roots() {
        let dir = temp_dir("read-file-guard");
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = temp_file_path("blocked");
        fs::write(&path, "blocked").expect("temp file should be written");
        let tool = ReadFileTool::new_with_allowed_roots(vec![dir]);

        let err = block_on(tool.call(serde_json::json!({
            "path": path.display().to_string()
        })))
        .expect_err("read_file should reject paths outside the allowed roots");

        assert!(
            err.to_string()
                .contains("outside the allowed capability roots")
        );
    }

    #[tokio::test]
    async fn exec_tool_runs_local_command() {
        let tool = ExecTool::new(vec![std::env::current_dir().expect("cwd should exist")]);
        let result = tool
            .call(serde_json::json!({
                "command": "pwd"
            }))
            .await
            .expect("exec tool should run");

        assert!(!result.content.is_empty());
        assert!(!result.is_error);
        assert_eq!(
            result.audit.expect("audit should exist").kind,
            CapabilityKind::Exec
        );
    }

    #[tokio::test]
    async fn cron_register_tool_persists_registration() {
        let store: Arc<dyn CronStore> = Arc::new(FileCronStore::new(temp_dir("cron")));
        let tool = CronRegisterTool::new(store.clone());

        let result = tool
            .call(serde_json::json!({
                "id": "nightly",
                "schedule": "0 0 * * *",
                "input": "run nightly",
                "session_id": "demo"
            }))
            .await
            .expect("cron register should succeed");

        assert!(!result.is_error);
        assert!(
            store
                .load("nightly")
                .expect("load should succeed")
                .is_some()
        );
    }

    #[test]
    fn mcp_tool_name_uses_expected_prefix() {
        assert_eq!(
            mcp_tool_name("filesystem", "read_file"),
            "mcp.filesystem.read_file"
        );
    }

    #[test]
    fn builtin_tool_metadata_defaults_to_builtin_source() {
        let meta = ToolMetadata::builtin("echo", "Echo", serde_json::json!({}));
        assert_eq!(meta.source, ToolSource::Builtin);
        assert_eq!(meta.capability.kind, CapabilityKind::Utility);
    }

    #[test]
    fn mcp_tool_metadata_captures_server_context() {
        let meta = ToolMetadata::mcp(
            "filesystem",
            "read_file",
            "Read files over MCP",
            serde_json::json!({ "type": "object" }),
        );

        assert_eq!(meta.name, "mcp.filesystem.read_file");
        assert_eq!(meta.source.label(), "mcp");
        assert_eq!(meta.source.server_name(), Some("filesystem"));
        assert_eq!(meta.source.remote_tool_name(), Some("read_file"));
    }

    #[test]
    fn capability_metadata_can_describe_stubbed_capabilities() {
        let meta = CapabilityMetadata::abstraction(CapabilityKind::Browser);

        assert_eq!(meta.kind, CapabilityKind::Browser);
        assert!(!meta.authorized);
        assert!(!meta.healthy);
    }
}
