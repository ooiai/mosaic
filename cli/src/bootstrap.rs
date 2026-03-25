use std::{env, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use mosaic_config::{AppConfig, McpConfig, MosaicConfig, SkillConfig, ToolConfig};
use mosaic_gateway::{GatewayHandle, GatewayRuntimeComponents};
use mosaic_mcp_core::{McpServerManager, McpServerSpec};
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::{
    RuntimeContext,
    events::{CompositeEventSink, SharedRunEventSink},
};
use mosaic_session_core::FileSessionStore;
use mosaic_skill_core::{SkillManifest, SkillRegistry, SummarizeSkill};
use mosaic_tool_core::{EchoTool, ReadFileTool, TimeNowTool, ToolRegistry};
use mosaic_tui::{TuiEventBuffer, build_tui_event_buffer, build_tui_event_sink};
use mosaic_workflow::{Workflow, WorkflowRegistry};
use tokio::runtime::Handle;

use crate::output::CliEventSink;

#[cfg_attr(not(test), allow(dead_code))]
pub struct OutputSinks {
    pub event_sink: SharedRunEventSink,
    pub tui_buffer: Option<TuiEventBuffer>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_runtime_context(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    event_sink: SharedRunEventSink,
) -> Result<RuntimeContext> {
    let profiles = Arc::new(ProviderProfileRegistry::from_config(config)?);
    let session_store_root = resolve_workspace_path(&config.session_store.root_dir)?;
    let (tools, _mcp_manager) = build_tools_with_mcp(
        app_config.map(|cfg| cfg.tools.as_slice()),
        app_config.and_then(|cfg| cfg.mcp.as_ref()),
    )?;
    let tools = Arc::new(tools);
    let skills = Arc::new(build_skills(app_config.map(|cfg| cfg.skills.as_slice()))?);

    Ok(RuntimeContext {
        profiles,
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(session_store_root)),
        tools,
        skills,
        workflows: Arc::new(build_workflows(
            app_config.map(|cfg| cfg.workflows.as_slice()),
        )?),
        event_sink,
    })
}

pub fn build_gateway_components(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
) -> Result<GatewayRuntimeComponents> {
    let profiles = Arc::new(ProviderProfileRegistry::from_config(config)?);
    let session_store_root = resolve_workspace_path(&config.session_store.root_dir)?;
    let runs_dir = resolve_workspace_path(&config.inspect.runs_dir)?;
    let (tools, mcp_manager) = build_tools_with_mcp(
        app_config.map(|cfg| cfg.tools.as_slice()),
        app_config.and_then(|cfg| cfg.mcp.as_ref()),
    )?;
    let tools = Arc::new(tools);
    let skills = Arc::new(build_skills(app_config.map(|cfg| cfg.skills.as_slice()))?);
    let workflows = Arc::new(build_workflows(
        app_config.map(|cfg| cfg.workflows.as_slice()),
    )?);

    Ok(GatewayRuntimeComponents {
        profiles,
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(session_store_root)),
        tools,
        skills,
        workflows,
        mcp_manager,
        runs_dir,
    })
}

pub fn build_local_gateway(
    runtime_handle: Handle,
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
) -> Result<GatewayHandle> {
    let components = build_gateway_components(config, app_config)?;
    Ok(GatewayHandle::new_local(runtime_handle, components))
}

#[allow(dead_code)]
pub fn build_cli_only_sinks() -> OutputSinks {
    OutputSinks {
        event_sink: Arc::new(CliEventSink),
        tui_buffer: None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
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

fn build_tools_with_mcp(
    tool_configs: Option<&[ToolConfig]>,
    mcp_config: Option<&McpConfig>,
) -> Result<(ToolRegistry, Option<Arc<McpServerManager>>)> {
    let mut tools = build_tools(tool_configs)?;
    let mcp_manager = build_mcp_manager(mcp_config)?;

    if let Some(manager) = &mcp_manager {
        manager.register_tools(&mut tools)?;
    }

    Ok((tools, mcp_manager))
}

fn build_mcp_manager(config: Option<&McpConfig>) -> Result<Option<Arc<McpServerManager>>> {
    let Some(config) = config else {
        return Ok(None);
    };

    if config.servers.is_empty() {
        return Ok(None);
    }

    let specs = config
        .servers
        .iter()
        .map(|server| McpServerSpec {
            name: server.name.clone(),
            command: server.command.clone(),
            args: server.args.clone(),
        })
        .collect::<Vec<_>>();

    Ok(Some(Arc::new(McpServerManager::start(&specs)?)))
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
                    ("manifest", _) => skills.register_manifest(SkillManifest {
                        name: skill.name.clone(),
                        description: skill
                            .description
                            .clone()
                            .unwrap_or_else(|| format!("manifest skill {}", skill.name)),
                        input_schema: skill.input_schema.clone(),
                        tools: skill.tools.clone(),
                        system_prompt: skill.system_prompt.clone(),
                        steps: skill.steps.clone(),
                    }),
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

fn build_workflows(configs: Option<&[Workflow]>) -> Result<WorkflowRegistry> {
    let mut workflows = WorkflowRegistry::new();

    if let Some(configs) = configs {
        for workflow in configs {
            workflows.register(workflow.clone());
        }
    }

    Ok(workflows)
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
        AgentConfig, AppConfig, McpConfig, McpServerConfig, MosaicConfig, ProviderConfig,
        SkillConfig, TaskConfig, ToolConfig,
    };
    use mosaic_runtime::events::{NoopEventSink, RunEvent};

    use super::{build_cli_and_tui_sinks, build_gateway_components, build_runtime_context};

    fn mcp_script_path() -> String {
        format!(
            "{}/../scripts/mock_mcp_server.py",
            env!("CARGO_MANIFEST_DIR")
        )
    }

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
                description: None,
                input_schema: serde_json::json!({ "type": "object" }),
                tools: Vec::new(),
                system_prompt: None,
                steps: Vec::new(),
            }],
            workflows: Vec::new(),
            agent: AgentConfig { system: None },
            task: TaskConfig {
                input: "hello".to_owned(),
            },
            mcp: None,
        }
    }

    fn app_config_with_mcp() -> AppConfig {
        let mut config = app_config();
        config.tools = Vec::new();
        config.mcp = Some(McpConfig {
            servers: vec![McpServerConfig {
                name: "filesystem".to_owned(),
                command: "python3".to_owned(),
                args: vec![mcp_script_path(), "filesystem".to_owned()],
            }],
        });
        config
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
        assert!(ctx.workflows.is_empty());
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
        assert!(ctx.workflows.is_empty());
    }

    #[test]
    fn gateway_components_register_builtin_tools_skills_and_paths() {
        let config = MosaicConfig::default();
        let components = build_gateway_components(&config, Some(&app_config()))
            .expect("gateway components should build");

        assert!(components.tools.get("time_now").is_some());
        assert!(components.tools.get("read_file").is_some());
        assert!(components.skills.get("summarize").is_some());
        assert!(components.workflows.is_empty());
        assert_eq!(components.profiles.active_profile_name(), "mock");
        assert!(components.runs_dir.ends_with(".mosaic/runs"));
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

    #[test]
    fn gateway_components_register_discovered_mcp_tools() {
        let config = MosaicConfig::default();
        let components = build_gateway_components(&config, Some(&app_config_with_mcp()))
            .expect("gateway components with MCP should build");

        assert!(components.tools.get("mcp.filesystem.read_file").is_some());
        assert_eq!(
            components
                .mcp_manager
                .as_ref()
                .map(|manager| manager.server_count()),
            Some(1)
        );
    }

    #[test]
    fn runtime_context_keeps_discovered_mcp_tools_callable() {
        let ctx = build_runtime_context(
            &MosaicConfig::default(),
            Some(&app_config_with_mcp()),
            Arc::new(NoopEventSink),
        )
        .expect("runtime context with MCP should build");
        let tool = ctx
            .tools
            .get("mcp.filesystem.read_file")
            .expect("MCP tool should be registered");

        let result = tokio::runtime::Runtime::new()
            .expect("tokio runtime should build")
            .block_on(tool.call(serde_json::json!({ "path": "README.md" })))
            .expect("MCP tool should remain callable");

        assert!(result.content.contains("Mosaic"));
    }
}
