use std::{env, sync::Arc};

use anyhow::{Result, bail};
use mosaic_config::{AppConfig, SkillConfig, ToolConfig};
use mosaic_provider::{LlmProvider, MockProvider, OpenAiCompatibleProvider};
use mosaic_runtime::{
    RuntimeContext,
    events::{CompositeEventSink, SharedRunEventSink},
};
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{EchoTool, ReadFileTool, TimeNowTool, ToolRegistry};
use mosaic_tui::{TuiEventBuffer, build_tui_event_buffer, build_tui_event_sink};

use crate::output::CliEventSink;

pub struct OutputSinks {
    pub event_sink: SharedRunEventSink,
    pub tui_buffer: Option<TuiEventBuffer>,
}

pub fn build_runtime_context(
    cfg: &AppConfig,
    event_sink: SharedRunEventSink,
) -> Result<RuntimeContext> {
    let provider = build_provider(cfg)?;
    let tools = Arc::new(build_tools(&cfg.tools)?);
    let skills = Arc::new(build_skills(&cfg.skills)?);

    Ok(RuntimeContext {
        provider,
        tools,
        skills,
        event_sink,
    })
}

pub fn build_cli_only_sinks() -> OutputSinks {
    OutputSinks {
        event_sink: Arc::new(CliEventSink),
        tui_buffer: None,
    }
}

pub fn build_cli_and_tui_sinks() -> OutputSinks {
    let buffer = build_tui_event_buffer();

    let cli_sink: SharedRunEventSink = Arc::new(CliEventSink);
    let tui_sink = build_tui_event_sink(buffer.clone());

    let composite: SharedRunEventSink = Arc::new(
        CompositeEventSink::new()
            .with_sink(cli_sink)
            .with_sink(tui_sink),
    );

    OutputSinks {
        event_sink: composite,
        tui_buffer: Some(buffer),
    }
}

fn build_provider(cfg: &AppConfig) -> Result<Arc<dyn LlmProvider>> {
    match cfg.provider.provider_type.as_str() {
        "mock" => Ok(Arc::new(MockProvider)),
        "openai-compatible" => {
            let base_url = cfg
                .provider
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_owned());

            let key_env = cfg
                .provider
                .api_key_env
                .clone()
                .unwrap_or_else(|| "OPENAI_API_KEY".to_owned());

            let api_key = env::var(&key_env)
                .map_err(|_| anyhow::anyhow!("missing provider api key env: {}", key_env))?;

            Ok(Arc::new(OpenAiCompatibleProvider::new(
                base_url,
                api_key,
                cfg.provider.model.clone(),
            )))
        }
        other => bail!("unsupported provider type: {other}"),
    }
}

fn build_tools(configs: &[ToolConfig]) -> Result<ToolRegistry> {
    let mut tools = ToolRegistry::new();

    for tool in configs {
        match (tool.tool_type.as_str(), tool.name.as_str()) {
            ("builtin", "echo") => tools.register(Arc::new(EchoTool::new())),
            ("builtin", "read_file") => tools.register(Arc::new(ReadFileTool::new())),
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
    use std::sync::Arc;

    use mosaic_config::{
        AgentConfig, AppConfig, ProviderConfig, SkillConfig, TaskConfig, ToolConfig,
    };
    use mosaic_runtime::events::{NoopEventSink, RunEvent};

    use super::{build_cli_and_tui_sinks, build_provider, build_runtime_context};

    #[test]
    fn runtime_context_registers_builtin_tools_and_skills() {
        let cfg = AppConfig {
            app: None,
            provider: ProviderConfig {
                provider_type: "mock".to_owned(),
                base_url: None,
                model: "mock".to_owned(),
                api_key_env: None,
            },
            tools: vec![
                ToolConfig {
                    tool_type: "builtin".to_owned(),
                    name: "time_now".to_owned(),
                },
                ToolConfig {
                    tool_type: "builtin".to_owned(),
                    name: "read_file".to_owned(),
                },
            ],
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

        let ctx = build_runtime_context(&cfg, Arc::new(NoopEventSink))
            .expect("runtime context should build");

        assert!(ctx.tools.get("time_now").is_some());
        assert!(ctx.tools.get("read_file").is_some());
        assert!(ctx.skills.get("summarize").is_some());
    }

    #[test]
    fn openai_compatible_provider_requires_api_key_env() {
        let cfg = AppConfig {
            app: None,
            provider: ProviderConfig {
                provider_type: "openai-compatible".to_owned(),
                base_url: None,
                model: "gpt-4o-mini".to_owned(),
                api_key_env: Some("MOSAIC_TEST_PROVIDER_KEY_SHOULD_NOT_EXIST_12345".to_owned()),
            },
            tools: vec![],
            skills: vec![],
            agent: AgentConfig { system: None },
            task: TaskConfig {
                input: "hello".to_owned(),
            },
            mcp: None,
        };

        let err = match build_provider(&cfg) {
            Ok(_) => panic!("missing env should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("missing provider api key env"));
    }

    #[test]
    fn cli_and_tui_sinks_expose_a_tui_buffer_and_broadcast_events() {
        let sinks = build_cli_and_tui_sinks();
        let buffer = sinks.tui_buffer.expect("tui buffer should be present");

        sinks.event_sink.emit(RunEvent::RunStarted {
            input: "hello".to_owned(),
        });

        assert_eq!(
            buffer.drain(),
            vec![RunEvent::RunStarted {
                input: "hello".to_owned(),
            }]
        );
    }
}
