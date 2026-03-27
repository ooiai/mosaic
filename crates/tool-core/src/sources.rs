use serde::{Deserialize, Serialize};

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
