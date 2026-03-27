use serde::{Deserialize, Serialize};

fn default_input_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string" }
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    #[serde(default = "default_input_schema")]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub tools: Vec<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub steps: Vec<ManifestSkillStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManifestSkillStep {
    Echo {
        name: String,
        input: Option<String>,
    },
    Summarize {
        name: String,
        input: Option<String>,
    },
    Tool {
        name: String,
        tool: String,
        #[serde(default)]
        input: serde_json::Value,
    },
}

impl ManifestSkillStep {
    pub fn name(&self) -> &str {
        match self {
            Self::Echo { name, .. } => name,
            Self::Summarize { name, .. } => name,
            Self::Tool { name, .. } => name,
        }
    }
}
