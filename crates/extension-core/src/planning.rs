use super::*;

pub(crate) fn plan_extensions(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
) -> Vec<PlannedExtension> {
    let mut planned = vec![crate::builtin::builtin_planned_extension(&config.policies)];

    if !config.tools.is_empty()
        || !config.skills.is_empty()
        || !config.workflows.is_empty()
        || config.mcp.is_some()
    {
        planned.push(PlannedExtension {
            status: ExtensionStatus {
                name: WORKSPACE_EXTENSION_NAME.to_owned(),
                version: WORKSPACE_EXTENSION_VERSION.to_owned(),
                source: "workspace_config".to_owned(),
                enabled: true,
                active: true,
                tools: config.tools.iter().map(|tool| tool.name.clone()).collect(),
                skills: config
                    .skills
                    .iter()
                    .map(|skill| skill.name.clone())
                    .collect(),
                workflows: config
                    .workflows
                    .iter()
                    .map(|workflow| workflow.name.clone())
                    .collect(),
                mcp_servers: config
                    .mcp
                    .as_ref()
                    .map(|mcp| {
                        mcp.servers
                            .iter()
                            .map(|server| server.name.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                error: None,
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: config.tools.clone(),
            skills: resolve_skill_paths(config.skills.clone(), workspace_root),
            workflows: config.workflows.clone(),
            mcp: config.mcp.clone(),
        });
    }

    if let Some(app_config) = app_config {
        planned.push(PlannedExtension {
            status: ExtensionStatus {
                name: APP_EXTENSION_NAME.to_owned(),
                version: APP_EXTENSION_VERSION.to_owned(),
                source: "app_config".to_owned(),
                enabled: true,
                active: true,
                tools: app_config
                    .tools
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect(),
                skills: app_config
                    .skills
                    .iter()
                    .map(|skill| skill.name.clone())
                    .collect(),
                workflows: app_config
                    .workflows
                    .iter()
                    .map(|workflow| workflow.name.clone())
                    .collect(),
                mcp_servers: app_config
                    .mcp
                    .as_ref()
                    .map(|mcp| {
                        mcp.servers
                            .iter()
                            .map(|server| server.name.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                error: None,
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: app_config.tools.clone(),
            skills: resolve_skill_paths(app_config.skills.clone(), workspace_root),
            workflows: app_config.workflows.clone(),
            mcp: app_config.mcp.clone(),
        });
    }

    for manifest_ref in &config.extensions.manifests {
        planned.push(load_manifest_extension(manifest_ref, workspace_root));
    }

    planned
}

fn load_manifest_extension(
    manifest_ref: &ExtensionManifestRef,
    workspace_root: &Path,
) -> PlannedExtension {
    let resolved = resolve_manifest_path(workspace_root, &manifest_ref.path);
    if !manifest_ref.enabled {
        return PlannedExtension {
            status: ExtensionStatus {
                name: resolved
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("disabled.extension")
                    .to_owned(),
                version: manifest_ref
                    .version_pin
                    .clone()
                    .unwrap_or_else(|| "disabled".to_owned()),
                source: resolved.display().to_string(),
                enabled: false,
                active: false,
                tools: Vec::new(),
                skills: Vec::new(),
                workflows: Vec::new(),
                mcp_servers: Vec::new(),
                error: None,
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: Vec::new(),
            skills: Vec::new(),
            workflows: Vec::new(),
            mcp: None,
        };
    }

    match load_extension_manifest_from_file(&resolved) {
        Ok(manifest) => manifest_planned(manifest, manifest_ref, resolved),
        Err(err) => PlannedExtension {
            status: ExtensionStatus {
                name: manifest_ref.path.clone(),
                version: manifest_ref
                    .version_pin
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                source: resolved.display().to_string(),
                enabled: true,
                active: false,
                tools: Vec::new(),
                skills: Vec::new(),
                workflows: Vec::new(),
                mcp_servers: Vec::new(),
                error: Some(err.to_string()),
            },
            schema_version: CURRENT_SCHEMA_VERSION,
            tools: Vec::new(),
            skills: Vec::new(),
            workflows: Vec::new(),
            mcp: None,
        },
    }
}

fn manifest_planned(
    manifest: ExtensionManifest,
    manifest_ref: &ExtensionManifestRef,
    resolved: PathBuf,
) -> PlannedExtension {
    let manifest_dir = resolved.parent().unwrap_or(Path::new("."));
    let error = match manifest_ref.version_pin.as_deref() {
        Some(expected) if expected != manifest.version => Some(format!(
            "version pin {} does not match manifest version {}",
            expected, manifest.version
        )),
        _ => None,
    };

    PlannedExtension {
        status: ExtensionStatus {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            source: resolved.display().to_string(),
            enabled: true,
            active: error.is_none(),
            tools: manifest
                .tools
                .iter()
                .map(|tool| tool.name.clone())
                .collect(),
            skills: manifest
                .skills
                .iter()
                .map(|skill| skill.name.clone())
                .collect(),
            workflows: manifest
                .workflows
                .iter()
                .map(|workflow| workflow.name.clone())
                .collect(),
            mcp_servers: manifest
                .mcp
                .as_ref()
                .map(|mcp| {
                    mcp.servers
                        .iter()
                        .map(|server| server.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            error,
        },
        schema_version: manifest.schema_version,
        tools: manifest.tools,
        skills: resolve_skill_paths(manifest.skills, manifest_dir),
        workflows: manifest.workflows,
        mcp: manifest.mcp,
    }
}

fn resolve_manifest_path(workspace_root: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        workspace_root.join(candidate)
    }
}

fn resolve_skill_paths(skills: Vec<SkillConfig>, base_dir: &Path) -> Vec<SkillConfig> {
    skills
        .into_iter()
        .map(|mut skill| {
            if skill.skill_type == "markdown_pack" {
                if let Some(path) = skill
                    .path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let candidate = PathBuf::from(path);
                    let resolved = if candidate.is_absolute() {
                        candidate
                    } else {
                        base_dir.join(candidate)
                    };
                    skill.path = Some(resolved.display().to_string());
                }
            }
            skill
        })
        .collect()
}
