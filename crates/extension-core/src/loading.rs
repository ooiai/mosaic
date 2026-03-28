use super::*;

pub(crate) fn load_extension_set(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
    cron_store: Arc<dyn CronStore>,
) -> Result<LoadedExtensionSet> {
    let report = crate::validation::validate_extension_set(config, app_config, workspace_root);
    if !report.is_ok() {
        let message = report
            .issues
            .iter()
            .map(|issue| match &issue.extension {
                Some(extension) => format!("{}: {}", extension, issue.message),
                None => issue.message.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ");
        bail!(message);
    }

    let planned = crate::planning::plan_extensions(config, app_config, workspace_root)
        .into_iter()
        .filter(|extension| extension.status.active && extension.status.error.is_none())
        .collect::<Vec<_>>();

    let mut tools = ToolRegistry::new();
    let mut skills = SkillRegistry::new();
    let mut workflows = WorkflowRegistry::new();
    let mut server_specs = Vec::new();
    let mut server_origins = BTreeMap::new();

    for extension in &planned {
        for tool in &extension.tools {
            let built = crate::builtin::build_builtin_tool(
                tool,
                workspace_root,
                cron_store.clone(),
                &extension.status.name,
                &extension.status.version,
                &extension.status.source,
                extension.schema_version,
            )?;
            tools.register(built);
        }

        for skill in &extension.skills {
            crate::builtin::register_skill(
                &mut skills,
                skill,
                &extension.status.name,
                &extension.status.version,
                &extension.status.source,
                extension.schema_version,
            )?;
        }

        for workflow in &extension.workflows {
            workflows.register_with_metadata(
                workflow.clone(),
                WorkflowMetadata::new(workflow.name.clone())
                    .with_exposure(
                        CapabilityExposure::new(extension.status.source.clone())
                            .with_visibility(workflow.visibility.visibility)
                            .with_invocation_mode(workflow.visibility.invocation_mode)
                            .with_required_policy(workflow.visibility.required_policy.clone())
                            .with_allowed_channels(workflow.visibility.allowed_channels.clone()),
                    )
                    .with_extension(
                        extension.status.name.clone(),
                        extension.status.version.clone(),
                    )
                    .with_compatibility(WorkflowCompatibility {
                        schema_version: extension.schema_version,
                    }),
            );
        }

        if let Some(mcp) = &extension.mcp {
            for server in &mcp.servers {
                server_specs.push(McpServerSpec {
                    name: server.name.clone(),
                    command: server.command.clone(),
                    args: server.args.clone(),
                });
                server_origins.insert(
                    server.name.clone(),
                    (
                        extension.status.name.clone(),
                        extension.status.version.clone(),
                        extension.schema_version,
                    ),
                );
            }
        }
    }

    let mcp_manager = if server_specs.is_empty() {
        None
    } else {
        let manager = Arc::new(McpServerManager::start(&server_specs)?);
        let before = tools.list().len();
        let registered = manager.register_tools(&mut tools)?;
        if tools.list().len() != before + registered.len() {
            bail!("MCP tool registration collided with an existing tool name");
        }
        crate::builtin::apply_mcp_extension_metadata(&mut tools, &registered, &server_origins);
        Some(manager)
    };

    Ok(LoadedExtensionSet {
        tools,
        skills,
        workflows,
        mcp_manager,
        extensions: report.extensions,
        policies: report.policies,
    })
}
