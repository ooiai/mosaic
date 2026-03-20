use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub app: Option<AppSection>,
    pub provider: ProviderConfig,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
    pub agent: AgentConfig,
    pub task: TaskConfig,
    pub mcp: Option<McpConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSection {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolConfig {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillConfig {
    #[serde(rename = "type")]
    pub skill_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentConfig {
    pub system: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskConfig {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

pub fn load_from_file(path: impl AsRef<Path>) -> Result<AppConfig> {
    let content = fs::read_to_string(path)?;
    let cfg = serde_yaml::from_str::<AppConfig>(&content)?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "mosaic-config-{label}-{}-{nanos}-{count}.yaml",
            process::id()
        ))
    }

    #[test]
    fn loads_yaml_config_from_disk() {
        let path = temp_path("valid");
        fs::write(
            &path,
            r#"
app:
  name: basic-demo
provider:
  type: openai-compatible
  model: mock
  api_key_env: OPENAI_API_KEY
tools:
  - type: builtin
    name: echo
skills:
  - type: builtin
    name: summarize
agent:
  system: "You are helpful."
task:
  input: "Explain Mosaic."
"#,
        )
        .expect("fixture should be written");

        let cfg = load_from_file(&path).expect("config should load");

        assert_eq!(
            cfg.app.and_then(|app| app.name),
            Some("basic-demo".to_owned())
        );
        assert_eq!(cfg.provider.provider_type, "openai-compatible");
        assert_eq!(cfg.provider.model, "mock");
        assert_eq!(cfg.tools.len(), 1);
        assert_eq!(cfg.skills.len(), 1);
        assert_eq!(cfg.task.input, "Explain Mosaic.");

        fs::remove_file(path).ok();
    }

    #[test]
    fn returns_an_error_for_invalid_yaml() {
        let path = temp_path("invalid");
        fs::write(&path, "provider: [").expect("fixture should be written");

        let err = load_from_file(&path).expect_err("invalid yaml should fail");

        assert!(!err.to_string().is_empty());
        fs::remove_file(path).ok();
    }
}
