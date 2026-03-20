use std::sync::Arc;

use anyhow::{Result, bail};
use mosaic_config::{AppConfig, SkillConfig, ToolConfig};
use mosaic_provider::{LlmProvider, MockProvider};
use mosaic_runtime::RuntimeContext;
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{EchoTool, TimeNowTool, ToolRegistry};

pub fn build_runtime_context(cfg: &AppConfig) -> Result<RuntimeContext> {
    let provider = build_provider(&cfg.provider.provider_type)?;
    let tools = Arc::new(build_tools(&cfg.tools)?);
    let skills = Arc::new(build_skills(&cfg.skills)?);

    Ok(RuntimeContext {
        provider,
        tools,
        skills,
    })
}

fn build_provider(provider_type: &str) -> Result<Arc<dyn LlmProvider>> {
    match provider_type {
        "mock" | "openai-compatible" => Ok(Arc::new(MockProvider)),
        other => bail!("unsupported provider type in skeleton mode: {other}"),
    }
}

fn build_tools(configs: &[ToolConfig]) -> Result<ToolRegistry> {
    let mut tools = ToolRegistry::new();

    for tool in configs {
        match (tool.tool_type.as_str(), tool.name.as_str()) {
            ("builtin", "echo") => tools.register(Arc::new(EchoTool::new())),
            ("builtin", "time_now") => tools.register(Arc::new(TimeNowTool::new())),
            ("builtin", other) => bail!("unsupported builtin tool in skeleton mode: {other}"),
            (other, _) => bail!("unsupported tool type in skeleton mode: {other}"),
        }
    }

    Ok(tools)
}

fn build_skills(configs: &[SkillConfig]) -> Result<SkillRegistry> {
    let mut skills = SkillRegistry::new();

    for skill in configs {
        match (skill.skill_type.as_str(), skill.name.as_str()) {
            ("builtin", "summarize") => skills.register(Arc::new(SummarizeSkill)),
            ("builtin", other) => bail!("unsupported builtin skill in skeleton mode: {other}"),
            (other, _) => bail!("unsupported skill type in skeleton mode: {other}"),
        }
    }

    Ok(skills)
}

#[cfg(test)]
mod tests {
    use mosaic_config::{
        AgentConfig, AppConfig, ProviderConfig, SkillConfig, TaskConfig, ToolConfig,
    };

    use super::build_runtime_context;

    #[test]
    fn runtime_context_registers_time_now_tool() {
        let cfg = AppConfig {
            app: None,
            provider: ProviderConfig {
                provider_type: "mock".to_owned(),
                base_url: None,
                model: "mock".to_owned(),
                api_key_env: None,
            },
            tools: vec![ToolConfig {
                tool_type: "builtin".to_owned(),
                name: "time_now".to_owned(),
            }],
            skills: vec![SkillConfig {
                skill_type: "builtin".to_owned(),
                name: "summarize".to_owned(),
            }],
            agent: AgentConfig { system: None },
            task: TaskConfig {
                input: "hello".to_owned(),
            },
            mcp: None,
        };

        let ctx = build_runtime_context(&cfg).expect("runtime context should build");

        assert!(ctx.tools.get("time_now").is_some());
        assert!(ctx.skills.get("summarize").is_some());
    }
}
