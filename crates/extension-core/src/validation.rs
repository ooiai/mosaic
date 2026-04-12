use super::*;
use mosaic_skill_core::MarkdownSkillPack;

pub(crate) fn validate_extension_set(
    config: &MosaicConfig,
    app_config: Option<&AppConfig>,
    workspace_root: &Path,
) -> ExtensionValidationReport {
    let mut planned = crate::planning::plan_extensions(config, app_config, workspace_root);
    let mut issues = Vec::new();
    let mut tool_names = BTreeMap::<String, String>::new();
    let mut skill_names = BTreeMap::<String, String>::new();
    let mut workflow_names = BTreeMap::<String, String>::new();
    let mut markdown_skill_tools = BTreeMap::<(String, String), Vec<String>>::new();

    for extension in &planned {
        if let Some(error) = extension.status.error.clone() {
            issues.push(ExtensionValidationIssue {
                extension: Some(extension.status.name.clone()),
                message: error,
            });
            continue;
        }

        if !extension.status.active {
            continue;
        }

        if extension.schema_version != CURRENT_SCHEMA_VERSION {
            issues.push(ExtensionValidationIssue {
                extension: Some(extension.status.name.clone()),
                message: format!(
                    "schema_version {} is not compatible with runtime schema {}",
                    extension.schema_version, CURRENT_SCHEMA_VERSION
                ),
            });
        }

        for tool in &extension.tools {
            if let Some(message) = crate::builtin::tool_policy_issue(tool, &config.policies) {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message,
                });
            }
            if let Some(previous) =
                tool_names.insert(tool.name.clone(), extension.status.name.clone())
            {
                if !inline_override_allowed(&extension.status.name, &previous) {
                    issues.push(ExtensionValidationIssue {
                        extension: Some(extension.status.name.clone()),
                        message: format!(
                            "tool '{}' collides with extension {}",
                            tool.name, previous
                        ),
                    });
                }
            }
        }

        if let Some(mcp) = &extension.mcp {
            if !config.policies.allow_mcp && !mcp.servers.is_empty() {
                issues.push(ExtensionValidationIssue {
                    extension: Some(extension.status.name.clone()),
                    message: "policy blocks MCP extensions".to_owned(),
                });
            }
        }

        for skill in &extension.skills {
            if skill.skill_type == "markdown_pack" {
                let Some(path) = skill
                    .path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    issues.push(ExtensionValidationIssue {
                        extension: Some(extension.status.name.clone()),
                        message: format!("markdown skill '{}' is missing path", skill.name),
                    });
                    continue;
                };
                match MarkdownSkillPack::load_from_dir(path) {
                    Ok(pack) => {
                        if pack.name() != skill.name {
                            issues.push(ExtensionValidationIssue {
                                extension: Some(extension.status.name.clone()),
                                message: format!(
                                    "markdown skill pack '{}' resolved to '{}' but config declares '{}'",
                                    path,
                                    pack.name(),
                                    skill.name
                                ),
                            });
                        }
                        markdown_skill_tools.insert(
                            (extension.status.name.clone(), skill.name.clone()),
                            if skill.tools.is_empty() {
                                pack.allowed_tools().to_vec()
                            } else {
                                skill.tools.clone()
                            },
                        );
                    }
                    Err(err) => issues.push(ExtensionValidationIssue {
                        extension: Some(extension.status.name.clone()),
                        message: format!("markdown skill '{}': {}", skill.name, err),
                    }),
                }
            }
            if let Some(previous) =
                skill_names.insert(skill.name.clone(), extension.status.name.clone())
            {
                if !inline_override_allowed(&extension.status.name, &previous) {
                    issues.push(ExtensionValidationIssue {
                        extension: Some(extension.status.name.clone()),
                        message: format!(
                            "skill '{}' collides with extension {}",
                            skill.name, previous
                        ),
                    });
                }
            }
        }

        for workflow in &extension.workflows {
            if let Some(previous) =
                workflow_names.insert(workflow.name.clone(), extension.status.name.clone())
            {
                if !inline_override_allowed(&extension.status.name, &previous) {
                    issues.push(ExtensionValidationIssue {
                        extension: Some(extension.status.name.clone()),
                        message: format!(
                            "workflow '{}' collides with extension {}",
                            workflow.name, previous
                        ),
                    });
                }
            }
        }
    }

    for extension in &planned {
        if !extension.status.active || extension.status.error.is_some() {
            continue;
        }

        for skill in &extension.skills {
            if skill.skill_type == "manifest" {
                for tool in &skill.tools {
                    if !tool_names.contains_key(tool) {
                        issues.push(ExtensionValidationIssue {
                            extension: Some(extension.status.name.clone()),
                            message: format!(
                                "manifest skill '{}' references unknown tool '{}'",
                                skill.name, tool
                            ),
                        });
                    }
                }
            } else if skill.skill_type == "markdown_pack" {
                let declared_tools = markdown_skill_tools
                    .get(&(extension.status.name.clone(), skill.name.clone()))
                    .cloned()
                    .unwrap_or_default();
                for tool in &declared_tools {
                    if !tool_names.contains_key(tool) {
                        issues.push(ExtensionValidationIssue {
                            extension: Some(extension.status.name.clone()),
                            message: format!(
                                "markdown skill '{}' references unknown tool '{}'",
                                skill.name, tool
                            ),
                        });
                    }
                }
            }
        }

        for workflow in &extension.workflows {
            for step in &workflow.steps {
                match &step.kind {
                    mosaic_workflow::WorkflowStepKind::Prompt { tools, .. } => {
                        for tool in tools {
                            if !tool_names.contains_key(tool) {
                                issues.push(ExtensionValidationIssue {
                                    extension: Some(extension.status.name.clone()),
                                    message: format!(
                                        "workflow '{}' references unknown tool '{}'",
                                        workflow.name, tool
                                    ),
                                });
                            }
                        }
                    }
                    mosaic_workflow::WorkflowStepKind::Skill { skill, .. } => {
                        if !skill_names.contains_key(skill) {
                            issues.push(ExtensionValidationIssue {
                                extension: Some(extension.status.name.clone()),
                                message: format!(
                                    "workflow '{}' references unknown skill '{}'",
                                    workflow.name, skill
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    planned.iter_mut().for_each(|planned| {
        if planned.status.error.is_none() {
            let relevant = issues
                .iter()
                .any(|issue| issue.extension.as_deref() == Some(planned.status.name.as_str()));
            if relevant {
                planned.status.error = Some("validation failed".to_owned());
            }
        }
    });

    ExtensionValidationReport {
        policies: config.policies.clone(),
        extensions: planned.into_iter().map(|planned| planned.status).collect(),
        issues,
    }
}

fn inline_override_allowed(current: &str, previous: &str) -> bool {
    matches!(current, WORKSPACE_EXTENSION_NAME | APP_EXTENSION_NAME)
        && matches!(
            previous,
            BUILTIN_EXTENSION_NAME | WORKSPACE_EXTENSION_NAME | APP_EXTENSION_NAME
        )
}
