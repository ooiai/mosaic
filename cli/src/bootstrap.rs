use std::{env, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use mosaic_config::{AppConfig, MosaicConfig, SkillConfig, ToolConfig};
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::{
    RuntimeContext,
    events::{CompositeEventSink, SharedRunEventSink},
};
use mosaic_session_core::FileSessionStore;
use mosaic_skill_core::{SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{EchoTool, ReadFileTool, TimeNowTool, ToolRegistry};
use mosaic_tui::{TuiEventBuffer, build_tui_event_buffer, build_tui_event_sink};

use crate::output::CliEventSink;

pub struct OutputSinks {
    pub event_sink: SharedRunEventSink,
    pub tui_buffer: Option<TuiEventBuffer>,
}

pub fn build_runtime_context(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    event_sink: SharedRunEventSink,
) -> Result<RuntimeContext> {
    let profiles = Arc::new(ProviderProfileRegistry::from_config(config)?);
    let session_store_root = resolve_workspace_path(&config.session_store.root_dir)?;
    let tools = Arc::new(build_tools(app_config.map(|cfg| cfg.tools.as_slice()))?);
    let skills = Arc::new(build_skills(app_config.map(|cfg| cfg.skills.as_slice()))?);

    Ok(RuntimeContext {
        profiles,
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(session_store_root)),
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

fn build_tools(configs: Option<&[ToolConfig]>) -> Result<ToolRegistry> {
    let mut tools = ToolRegistry::new();

    match configs {
        Some(configs) => {
            for tool in configs {
                register_tool(&mut tools, tool)?;
            }
        }
        None => {
            tools.register(Arc::new(EchoTool::new()));
            tools.register(Arc::new(ReadFileTool::new()));
            tools.register(Arc::new(TimeNowTool::new()));
        }
    }

    Ok(tools)
}

fn register_tool(registry: &mut ToolRegistry, tool: &ToolConfig) -> Result<()> {
    match (tool.tool_type.as_str(), tool.name.as_str()) {
        ("builtin", "echo") => registry.register(Arc::new(EchoTool::new())),
        ("builtin", "read_file") => registry.register(Arc::new(ReadFileTool::new())),
        ("builtin", "time_now") => registry.register(Arc::new(TimeNowTool::new())),
        ("builtin", other) => bail!("unsupported builtin tool in skeleton mode: {other}"),
        (other, _) => bail!("unsupported tool type in skeleton mode: {other}"),
    }

    Ok(())
}

fn build_skills(configs: Option<&[SkillConfig]>) -> Result<SkillRegistry> {
    let mut skills = SkillRegistry::new();

    match configs {
        Some(configs) => {
            for skill in configs {
                match (skill.skill_type.as_str(), skill.name.as_str()) {
                    ("builtin", "summarize") => skills.register(Arc::new(SummarizeSkill)),
                    ("builtin", other) => {
                        bail!("unsupported builtin skill in skeleton mode: {other}")
                    }
                    (other, _) => bail!("unsupported skill type in skeleton mode: {other}"),
                }
            }
        }
        None => {
            skills.register(Arc::new(SummarizeSkill));
        }
    }

    Ok(skills)
}

fn resolve_workspace_path(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }

    Ok(env::current_dir()?.join(path))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mosaic_config::{
        AgentConfig, AppConfig, MosaicConfig, ProviderConfig, SkillConfig, TaskConfig, ToolConfig,
    };
    use mosaic_runtime::events::{NoopEventSink, RunEvent};

    use super::{build_cli_and_tui_sinks, build_runtime_context};

    fn app_config() -> AppConfig {
        AppConfig {
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
        }
    }

    #[test]
    fn runtime_context_registers_builtin_tools_and_skills() {
        let ctx = build_runtime_context(
            &MosaicConfig::default(),
            Some(&app_config()),
            Arc::new(NoopEventSink),
        )
        .expect("runtime context should build");

        assert!(ctx.tools.get("time_now").is_some());
        assert!(ctx.tools.get("read_file").is_some());
        assert!(ctx.skills.get("summarize").is_some());
        assert_eq!(ctx.profiles.active_profile_name(), "mock");
    }

    #[test]
    fn runtime_context_uses_default_interactive_tools_when_no_app_recipe_is_present() {
        let ctx = build_runtime_context(&MosaicConfig::default(), None, Arc::new(NoopEventSink))
            .expect("runtime context should build");

        assert!(ctx.tools.get("echo").is_some());
        assert!(ctx.tools.get("read_file").is_some());
        assert!(ctx.tools.get("time_now").is_some());
        assert!(ctx.skills.get("summarize").is_some());
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
