use super::*;

pub(crate) fn builtin_planned_extension(policies: &PolicyConfig) -> PlannedExtension {
    PlannedExtension {
        status: ExtensionStatus {
            name: BUILTIN_EXTENSION_NAME.to_owned(),
            version: BUILTIN_EXTENSION_VERSION.to_owned(),
            source: "builtin".to_owned(),
            enabled: true,
            active: true,
            tools: builtin_tool_configs(policies)
                .into_iter()
                .map(|tool| tool.name)
                .collect(),
            skills: vec!["summarize".to_owned()],
            workflows: Vec::new(),
            mcp_servers: Vec::new(),
            error: None,
        },
        schema_version: CURRENT_SCHEMA_VERSION,
        tools: builtin_tool_configs(policies),
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
        mcp: None,
    }
}

pub(crate) fn tool_policy_issue(tool: &ToolConfig, policies: &PolicyConfig) -> Option<String> {
    match tool.name.as_str() {
        "exec_command" if !policies.allow_exec => Some("policy blocks exec_command".to_owned()),
        "webhook_call" if !policies.allow_webhook => Some("policy blocks webhook_call".to_owned()),
        "cron_register" if !policies.allow_cron => Some("policy blocks cron_register".to_owned()),
        _ => None,
    }
}

pub(crate) fn build_builtin_tool(
    tool: &ToolConfig,
    workspace_root: &Path,
    cron_store: Arc<dyn CronStore>,
    extension_name: &str,
    extension_version: &str,
    extension_source: &str,
    schema_version: u32,
) -> Result<Arc<dyn Tool>> {
    let roots = vec![workspace_root.to_path_buf()];
    let inner: Arc<dyn Tool> = match (tool.tool_type.as_str(), tool.name.as_str()) {
        ("builtin", "cron_register") => Arc::new(CronRegisterTool::new(cron_store)),
        ("builtin", "echo") => Arc::new(EchoTool::new()),
        ("builtin", "exec_command") => Arc::new(ExecTool::new(roots.clone())),
        ("builtin", "read_file") => Arc::new(ReadFileTool::new_with_allowed_roots(roots)),
        ("builtin", "time_now") => Arc::new(TimeNowTool::new()),
        ("builtin", "webhook_call") => Arc::new(WebhookTool::new()),
        ("builtin", other) => bail!("unsupported builtin tool: {}", other),
        (other, _) => bail!("unsupported tool type: {}", other),
    };

    let metadata = inner
        .metadata()
        .clone()
        .with_exposure(crate::exposure_from_tool_config(tool, extension_source));
    let inner: Arc<dyn Tool> = Arc::new(ExtensionWrappedTool { inner, metadata });

    Ok(wrap_tool(
        inner,
        extension_name,
        extension_version,
        schema_version,
    ))
}

pub(crate) fn register_skill(
    registry: &mut SkillRegistry,
    skill: &SkillConfig,
    extension_name: &str,
    extension_version: &str,
    extension_source: &str,
    schema_version: u32,
) -> Result<()> {
    let compatibility = SkillCompatibility { schema_version };
    match (skill.skill_type.as_str(), skill.name.as_str()) {
        ("builtin", "summarize") => registry.register_native_with_metadata(
            Arc::new(SummarizeSkill),
            SkillMetadata::native("summarize")
                .with_exposure(crate::exposure_from_skill_config(skill, extension_source))
                .with_extension(extension_name.to_owned(), extension_version.to_owned())
                .with_compatibility(compatibility),
        ),
        ("manifest", _) => registry.register_manifest_with_metadata(
            SkillManifest {
                name: skill.name.clone(),
                description: skill
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("manifest skill {}", skill.name)),
                input_schema: skill.input_schema.clone(),
                tools: skill.tools.clone(),
                system_prompt: skill.system_prompt.clone(),
                steps: skill.steps.clone(),
            },
            SkillMetadata::manifest(&SkillManifest {
                name: skill.name.clone(),
                description: skill
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("manifest skill {}", skill.name)),
                input_schema: skill.input_schema.clone(),
                tools: skill.tools.clone(),
                system_prompt: skill.system_prompt.clone(),
                steps: skill.steps.clone(),
            })
            .with_exposure(crate::exposure_from_skill_config(skill, extension_source))
            .with_extension(extension_name.to_owned(), extension_version.to_owned())
            .with_compatibility(compatibility),
        ),
        ("builtin", other) => bail!("unsupported builtin skill: {}", other),
        (other, _) => bail!("unsupported skill type: {}", other),
    }
    Ok(())
}

pub(crate) fn apply_mcp_extension_metadata(
    tools: &mut ToolRegistry,
    registered: &[McpRegisteredTool],
    origins: &BTreeMap<String, (String, String, u32)>,
) {
    for registration in registered {
        let Some((extension_name, extension_version, schema_version)) =
            origins.get(&registration.server_name)
        else {
            continue;
        };
        let Some(existing) = tools.get(&registration.qualified_name) else {
            continue;
        };
        tools.register(wrap_tool(
            existing,
            extension_name,
            extension_version,
            *schema_version,
        ));
    }
}

fn builtin_tool_configs(policies: &PolicyConfig) -> Vec<ToolConfig> {
    let mut tools = vec![
        ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "echo".to_owned(),
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
        ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "time_now".to_owned(),
            visibility: mosaic_tool_core::CapabilityVisibility::Visible,
            invocation_mode: mosaic_tool_core::CapabilityInvocationMode::Conversational,
            required_policy: None,
            allowed_channels: Vec::new(),
            accepts_attachments: false,
        },
    ];

    if policies.allow_cron {
        tools.push(ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "cron_register".to_owned(),
            visibility: mosaic_tool_core::CapabilityVisibility::Restricted,
            invocation_mode: mosaic_tool_core::CapabilityInvocationMode::ExplicitOnly,
            required_policy: Some("allow_cron".to_owned()),
            allowed_channels: Vec::new(),
            accepts_attachments: false,
        });
    }
    if policies.allow_exec {
        tools.push(ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "exec_command".to_owned(),
            visibility: mosaic_tool_core::CapabilityVisibility::Restricted,
            invocation_mode: mosaic_tool_core::CapabilityInvocationMode::ExplicitOnly,
            required_policy: Some("allow_exec".to_owned()),
            allowed_channels: Vec::new(),
            accepts_attachments: false,
        });
    }
    if policies.allow_webhook {
        tools.push(ToolConfig {
            tool_type: "builtin".to_owned(),
            name: "webhook_call".to_owned(),
            visibility: mosaic_tool_core::CapabilityVisibility::Restricted,
            invocation_mode: mosaic_tool_core::CapabilityInvocationMode::ExplicitOnly,
            required_policy: Some("allow_webhook".to_owned()),
            allowed_channels: Vec::new(),
            accepts_attachments: false,
        });
    }

    tools
}
