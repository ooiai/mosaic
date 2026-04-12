use serde::{Deserialize, Serialize};

use mosaic_sandbox_core::SandboxBinding;

use crate::sources::{ToolSource, mcp_tool_name};

fn default_true() -> bool {
    true
}

fn default_compatibility_schema() -> u32 {
    1
}

fn default_capability_source() -> String {
    "unknown".to_owned()
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityVisibility {
    #[default]
    Visible,
    Restricted,
    Hidden,
}

impl CapabilityVisibility {
    pub fn label(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Restricted => "restricted",
            Self::Hidden => "hidden",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityInvocationMode {
    #[default]
    Conversational,
    ExplicitOnly,
    Hidden,
}

impl CapabilityInvocationMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Conversational => "conversational",
            Self::ExplicitOnly => "explicit_only",
            Self::Hidden => "hidden",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityExposure {
    #[serde(default = "default_capability_source")]
    pub source: String,
    #[serde(default)]
    pub visibility: CapabilityVisibility,
    #[serde(default)]
    pub invocation_mode: CapabilityInvocationMode,
    #[serde(default)]
    pub required_policy: Option<String>,
    #[serde(default)]
    pub allowed_channels: Vec<String>,
    #[serde(default)]
    pub accepts_attachments: bool,
}

impl Default for CapabilityExposure {
    fn default() -> Self {
        Self {
            source: default_capability_source(),
            visibility: CapabilityVisibility::Visible,
            invocation_mode: CapabilityInvocationMode::Conversational,
            required_policy: None,
            allowed_channels: Vec::new(),
            accepts_attachments: false,
        }
    }
}

impl CapabilityExposure {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    pub fn with_visibility(mut self, visibility: CapabilityVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn with_invocation_mode(mut self, invocation_mode: CapabilityInvocationMode) -> Self {
        self.invocation_mode = invocation_mode;
        self
    }

    pub fn with_required_policy(mut self, required_policy: Option<String>) -> Self {
        self.required_policy = required_policy;
        self
    }

    pub fn with_allowed_channels(mut self, allowed_channels: Vec<String>) -> Self {
        self.allowed_channels = allowed_channels;
        self
    }

    pub fn with_accepts_attachments(mut self, accepts_attachments: bool) -> Self {
        self.accepts_attachments = accepts_attachments;
        self
    }

    pub fn allows_channel(&self, channel: Option<&str>) -> bool {
        if self.allowed_channels.is_empty() {
            return true;
        }

        let Some(channel) = channel else {
            return false;
        };

        self.allowed_channels
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(channel))
    }

    pub fn allows_explicit(&self, channel: Option<&str>) -> bool {
        self.visibility != CapabilityVisibility::Hidden
            && self.invocation_mode != CapabilityInvocationMode::Hidden
            && self.allows_channel(channel)
    }

    pub fn allows_conversational(&self, channel: Option<&str>) -> bool {
        self.visibility == CapabilityVisibility::Visible
            && self.invocation_mode == CapabilityInvocationMode::Conversational
            && self.allows_channel(channel)
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NodeRouteMetadata {
    pub capability: Option<String>,
    #[serde(default)]
    pub prefer_node: bool,
    #[serde(default)]
    pub require_node: bool,
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
    #[serde(default)]
    pub node: NodeRouteMetadata,
    #[serde(default)]
    pub sandbox: Option<SandboxBinding>,
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
            node: NodeRouteMetadata::default(),
            sandbox: None,
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
            node: NodeRouteMetadata::default(),
            sandbox: None,
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
            node: NodeRouteMetadata::default(),
            sandbox: None,
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
            node: NodeRouteMetadata::default(),
            sandbox: None,
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
            node: NodeRouteMetadata::default(),
            sandbox: None,
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
            node: NodeRouteMetadata::default(),
            sandbox: None,
        }
    }

    pub fn with_node_route(
        mut self,
        capability: impl Into<String>,
        prefer_node: bool,
        require_node: bool,
    ) -> Self {
        self.node = NodeRouteMetadata {
            capability: Some(capability.into()),
            prefer_node,
            require_node,
        };
        self
    }

    pub fn routes_via_node(&self) -> bool {
        self.node.capability.is_some()
    }

    pub fn with_sandbox_binding(mut self, sandbox: Option<SandboxBinding>) -> Self {
        self.sandbox = sandbox;
        self
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
    #[serde(default)]
    pub exposure: CapabilityExposure,
    pub extension: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
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
            exposure: CapabilityExposure::default(),
            extension: None,
            version: None,
            source_path: None,
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
            exposure: CapabilityExposure::default(),
            extension: None,
            version: None,
            source_path: None,
            compatibility: ToolCompatibility::default(),
        }
    }

    pub fn with_capability(mut self, capability: CapabilityMetadata) -> Self {
        self.capability = capability;
        self
    }

    pub fn with_exposure(mut self, exposure: CapabilityExposure) -> Self {
        self.exposure = exposure;
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

    pub fn with_source_path(mut self, source_path: Option<String>) -> Self {
        self.source_path = source_path;
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
