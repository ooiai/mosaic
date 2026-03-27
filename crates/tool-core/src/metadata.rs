use serde::{Deserialize, Serialize};

use crate::sources::{ToolSource, mcp_tool_name};

fn default_true() -> bool {
    true
}

fn default_compatibility_schema() -> u32 {
    1
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
