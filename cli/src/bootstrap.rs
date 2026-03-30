use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use mosaic_config::{AppConfig, LoadedMosaicConfig, MosaicConfig};
use mosaic_extension_core::{ExtensionStatus, load_extension_set};
use mosaic_gateway::{GatewayHandle, GatewayReloadSource, GatewayRuntimeComponents};
use mosaic_inspect::ExtensionTrace;
use mosaic_memory::{FileMemoryStore, MemoryPolicy};
use mosaic_node_protocol::FileNodeStore;
use mosaic_provider::ProviderProfileRegistry;
use mosaic_runtime::{
    RuntimeContext,
    events::{CompositeEventSink, SharedRunEventSink},
};
use mosaic_scheduler_core::{CronStore, FileCronStore};
use mosaic_session_core::FileSessionStore;
use mosaic_tui::{TuiEventBuffer, build_tui_event_buffer, build_tui_event_sink};
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
    let memory_store_root = resolve_workspace_path(".mosaic/memory")?;
    let cron_store_root = resolve_workspace_path(".mosaic/cron")?;
    let node_store_root = resolve_workspace_path(".mosaic/nodes")?;
    let memory_policy = MemoryPolicy::default();
    let cron_store: Arc<dyn CronStore> = Arc::new(FileCronStore::new(cron_store_root));
    let extension_set = load_extension_set(config, app_config, &env::current_dir()?, cron_store)?;

    Ok(RuntimeContext {
        profiles,
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(session_store_root)),
        memory_store: Arc::new(FileMemoryStore::new(memory_store_root)),
        memory_policy,
        runtime_policy: config.runtime.clone(),
        attachments: config.attachments.clone(),
        telegram: config.telegram.clone(),
        app_name: app_config
            .and_then(|app| app.app.as_ref())
            .and_then(|app| app.name.clone()),
        tools: Arc::new(extension_set.tools),
        skills: Arc::new(extension_set.skills),
        workflows: Arc::new(extension_set.workflows),
        node_router: Some(Arc::new(FileNodeStore::new(node_store_root))),
        active_extensions: extension_set
            .extensions
            .iter()
            .map(extension_trace)
            .collect(),
        event_sink,
    })
}

pub fn build_gateway_components(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
) -> Result<GatewayRuntimeComponents> {
    build_gateway_components_for_workspace(config, app_config, &env::current_dir()?)
}

pub fn build_local_gateway(
    runtime_handle: Handle,
    loaded: &LoadedMosaicConfig,
    app_config: Option<&AppConfig>,
) -> Result<GatewayHandle> {
    let workspace_root = env::current_dir()?;
    let components =
        build_gateway_components_for_workspace(&loaded.config, app_config, &workspace_root)?;
    let reload_source = GatewayReloadSource {
        workspace_root,
        workspace_config_path: loaded.workspace_config_path.clone(),
        user_config_path: loaded.user_config_path.clone(),
        app_config: app_config.cloned(),
    };

    Ok(GatewayHandle::new_local_with_reload_source(
        runtime_handle,
        components,
        Some(reload_source),
    ))
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

fn build_gateway_components_for_workspace(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
) -> Result<GatewayRuntimeComponents> {
    let profiles = Arc::new(ProviderProfileRegistry::from_config(config)?);
    let session_store_root = resolve_workspace_path(&config.session_store.root_dir)?;
    let memory_store_root = resolve_workspace_path(".mosaic/memory")?;
    let cron_store_root = resolve_workspace_path(".mosaic/cron")?;
    let node_store_root = resolve_workspace_path(".mosaic/nodes")?;
    let memory_policy = MemoryPolicy::default();
    let runs_dir = resolve_workspace_path(&config.inspect.runs_dir)?;
    let cron_store: Arc<dyn CronStore> = Arc::new(FileCronStore::new(cron_store_root));
    let extension_set = load_extension_set(config, app_config, workspace_root, cron_store.clone())?;

    let audit_root = resolve_workspace_path(&config.audit.root_dir)?;

    Ok(GatewayRuntimeComponents {
        profiles,
        provider_override: None,
        session_store: Arc::new(FileSessionStore::new(session_store_root)),
        memory_store: Arc::new(FileMemoryStore::new(memory_store_root)),
        memory_policy,
        runtime_policy: config.runtime.clone(),
        attachments: config.attachments.clone(),
        telegram: config.telegram.clone(),
        app_name: app_config
            .and_then(|app| app.app.as_ref())
            .and_then(|app| app.name.clone()),
        tools: Arc::new(extension_set.tools),
        skills: Arc::new(extension_set.skills),
        workflows: Arc::new(extension_set.workflows),
        node_store: Arc::new(FileNodeStore::new(node_store_root)),
        mcp_manager: extension_set.mcp_manager,
        cron_store,
        workspace_root: workspace_root.to_path_buf(),
        runs_dir,
        audit_root,
        extensions: extension_set.extensions,
        policies: extension_set.policies,
        deployment: config.deployment.clone(),
        auth: config.auth.clone(),
        audit: config.audit.clone(),
        observability: config.observability.clone(),
    })
}

fn extension_trace(status: &ExtensionStatus) -> ExtensionTrace {
    ExtensionTrace {
        name: status.name.clone(),
        version: status.version.clone(),
        source: status.source.clone(),
        enabled: status.enabled,
        active: status.active,
        error: status.error.clone(),
    }
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
        AgentConfig, AppConfig, McpConfig, McpServerConfig, MosaicConfig, PolicyConfig,
        ProviderConfig, SkillConfig, TaskConfig, ToolConfig,
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
                    visibility: mosaic_tool_core::CapabilityVisibility::Visible,
                    invocation_mode: mosaic_tool_core::CapabilityInvocationMode::Conversational,
                    required_policy: None,
                    allowed_channels: Vec::new(),
                    accepts_attachments: false,
                },
                ToolConfig {
                    tool_type: "builtin".to_owned(),
                    name: "read_file".to_owned(),
                    visibility: mosaic_tool_core::CapabilityVisibility::Visible,
                    invocation_mode: mosaic_tool_core::CapabilityInvocationMode::Conversational,
                    required_policy: None,
                    allowed_channels: Vec::new(),
                    accepts_attachments: false,
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
                visibility: mosaic_tool_core::CapabilityVisibility::Visible,
                invocation_mode: mosaic_tool_core::CapabilityInvocationMode::Conversational,
                required_policy: None,
                allowed_channels: Vec::new(),
                accepts_attachments: false,
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
        assert_eq!(ctx.profiles.active_profile_name(), "gpt-5.4-mini");
        assert_eq!(ctx.active_extensions.len(), 2);
        assert!(
            ctx.active_extensions
                .iter()
                .any(|ext| ext.name == "builtin.core")
        );
        assert!(
            ctx.active_extensions
                .iter()
                .any(|ext| ext.name == "app.inline")
        );
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
        assert_eq!(ctx.active_extensions.len(), 1);
        assert_eq!(ctx.active_extensions[0].name, "builtin.core");
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
        assert_eq!(components.profiles.active_profile_name(), "gpt-5.4-mini");
        assert!(components.runs_dir.ends_with(".mosaic/runs"));
        assert_eq!(components.extensions.len(), 2);
        assert!(
            components
                .extensions
                .iter()
                .any(|ext| ext.name == "builtin.core")
        );
        assert!(
            components
                .extensions
                .iter()
                .any(|ext| ext.name == "app.inline")
        );
        assert_eq!(components.policies, PolicyConfig::default());
    }

    #[test]
    fn cli_and_tui_sinks_expose_a_tui_buffer_and_broadcast_events() {
        let sinks = build_cli_and_tui_sinks();
        let buffer = sinks.tui_buffer.expect("tui buffer should be present");

        sinks.event_sink.emit(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });

        assert_eq!(
            buffer.drain(),
            vec![RunEvent::RunStarted {
                run_id: "run-1".to_owned(),
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
        assert_eq!(components.extensions.len(), 2);
        assert!(
            components
                .extensions
                .iter()
                .any(|ext| ext.name == "builtin.core")
        );
        assert!(components.extensions.iter().any(
            |ext| ext.name == "app.inline" && ext.mcp_servers == vec!["filesystem".to_owned()]
        ));
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
