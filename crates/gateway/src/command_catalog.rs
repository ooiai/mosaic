use mosaic_config::PolicyConfig;
use mosaic_provider::ProviderProfile;
use mosaic_tool_core::{CapabilityExposure, CapabilityVisibility, ToolMetadata, ToolSource};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChannelCommandCategory {
    Session,
    Runtime,
    Tools,
    Skills,
    Workflows,
    Gateway,
}

impl ChannelCommandCategory {
    pub(crate) const ALL: [Self; 6] = [
        Self::Session,
        Self::Runtime,
        Self::Tools,
        Self::Skills,
        Self::Workflows,
        Self::Gateway,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Runtime => "Runtime",
            Self::Tools => "Tools",
            Self::Skills => "Skills",
            Self::Workflows => "Workflows",
            Self::Gateway => "Gateway",
        }
    }

    pub(crate) fn slug(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Runtime => "runtime",
            Self::Tools => "tools",
            Self::Skills => "skills",
            Self::Workflows => "workflows",
            Self::Gateway => "gateway",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "session" | "sessions" => Some(Self::Session),
            "runtime" | "profile" | "profiles" => Some(Self::Runtime),
            "tool" | "tools" => Some(Self::Tools),
            "skill" | "skills" => Some(Self::Skills),
            "workflow" | "workflows" => Some(Self::Workflows),
            "gateway" => Some(Self::Gateway),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelCommandKind {
    Control,
    Session,
    Profile,
    Tool,
    Skill,
    Workflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelCommandEntry {
    pub(crate) name: String,
    pub(crate) category: ChannelCommandCategory,
    pub(crate) summary: String,
    pub(crate) usage: String,
    pub(crate) kind: ChannelCommandKind,
    pub(crate) allowed_channels: Vec<String>,
    pub(crate) required_policy: Option<String>,
    pub(crate) visibility: CapabilityVisibility,
    pub(crate) source: String,
}

impl ChannelCommandEntry {
    fn visible_in(&self, channel: &str, policies: &PolicyConfig) -> bool {
        if self.visibility == CapabilityVisibility::Hidden {
            return false;
        }
        if !self.allowed_channels.is_empty()
            && !self
                .allowed_channels
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(channel))
        {
            return false;
        }
        policy_allows(self.required_policy.as_deref(), policies)
    }

    pub(crate) fn line(&self) -> String {
        format!("{} - {}", self.usage, self.summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelCommandContext {
    pub(crate) channel: String,
    pub(crate) session_id: Option<String>,
    pub(crate) profile: String,
}

impl ChannelCommandContext {
    pub(crate) fn scope_label(&self) -> String {
        let mut parts = vec![format!("channel={}", self.channel)];
        if let Some(session_id) = self.session_id.as_deref() {
            parts.push(format!("session={session_id}"));
        }
        parts.push(format!("profile={}", self.profile));
        parts.join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelCommandCatalog {
    pub(crate) selected_category: Option<ChannelCommandCategory>,
    pub(crate) scope: String,
    pub(crate) entries: Vec<ChannelCommandEntry>,
}

impl ChannelCommandCatalog {
    pub(crate) fn render(&self) -> String {
        if self.entries.is_empty() {
            if let Some(category) = self.selected_category {
                return format!(
                    "No {} commands are currently available in this conversation.\nscope: {}",
                    category.slug(),
                    self.scope
                );
            }
            return format!(
                "No /mosaic commands are currently available in this conversation.\nscope: {}",
                self.scope
            );
        }

        let intro = match self.selected_category {
            Some(category) => format!(
                "{} commands available in this conversation.",
                category.title()
            ),
            None => "Mosaic commands available in this conversation.".to_owned(),
        };

        let mut lines = vec![intro, format!("scope: {}", self.scope)];
        for category in ChannelCommandCategory::ALL {
            if self
                .selected_category
                .is_some_and(|selected| selected != category)
            {
                continue;
            }

            let entries = self
                .entries
                .iter()
                .filter(|entry| entry.category == category)
                .collect::<Vec<_>>();
            if entries.is_empty() {
                continue;
            }

            lines.push(String::new());
            lines.push(category.title().to_owned());
            lines.extend(entries.into_iter().map(ChannelCommandEntry::line));
        }
        lines.join("\n")
    }
}

pub(crate) fn build_command_catalog(
    components: &GatewayRuntimeComponents,
    context: &ChannelCommandContext,
    selected_category: Option<ChannelCommandCategory>,
) -> ChannelCommandCatalog {
    let mut entries = Vec::new();

    entries.extend(session_control_entries());
    entries.extend(runtime_control_entries());
    entries.extend(profile_entries(&components.profiles, context));
    entries.extend(tool_entries(components, context));
    entries.extend(skill_entries(components, context));
    entries.extend(workflow_entries(components, context));
    entries.extend(gateway_entries());

    let entries = entries
        .into_iter()
        .filter(|entry| {
            selected_category.map_or(true, |category| entry.category == category)
                && entry.visible_in(&context.channel, &components.policies)
        })
        .collect();

    ChannelCommandCatalog {
        selected_category,
        scope: context.scope_label(),
        entries,
    }
}

fn session_control_entries() -> Vec<ChannelCommandEntry> {
    vec![
        control_entry(
            "session.new",
            ChannelCommandCategory::Session,
            "/mosaic session new <name>",
            "Create a fresh session and bind this conversation to it",
            ChannelCommandKind::Session,
        ),
        control_entry(
            "session.switch",
            ChannelCommandCategory::Session,
            "/mosaic session switch <name>",
            "Switch this conversation to an existing session",
            ChannelCommandKind::Session,
        ),
        control_entry(
            "session.status",
            ChannelCommandCategory::Session,
            "/mosaic session status",
            "Show the current session binding, route, and last run state",
            ChannelCommandKind::Session,
        ),
    ]
}

fn runtime_control_entries() -> Vec<ChannelCommandEntry> {
    vec![
        control_entry(
            "catalog",
            ChannelCommandCategory::Runtime,
            "/mosaic",
            "Show the command catalog available in this conversation",
            ChannelCommandKind::Control,
        ),
        control_entry(
            "help",
            ChannelCommandCategory::Runtime,
            "/mosaic help <session|runtime|tools|skills|workflows|gateway>",
            "Show one command category or the full catalog",
            ChannelCommandKind::Control,
        ),
    ]
}

fn gateway_entries() -> Vec<ChannelCommandEntry> {
    vec![
        control_entry(
            "gateway.status",
            ChannelCommandCategory::Gateway,
            "/mosaic gateway status",
            "Show gateway health, readiness, and adapter status",
            ChannelCommandKind::Control,
        ),
        control_entry(
            "gateway.help",
            ChannelCommandCategory::Gateway,
            "/mosaic gateway help",
            "Show only gateway-related commands",
            ChannelCommandKind::Control,
        ),
    ]
}

fn profile_entries(
    profiles: &ProviderProfileRegistry,
    context: &ChannelCommandContext,
) -> Vec<ChannelCommandEntry> {
    let mut available = profiles
        .list()
        .into_iter()
        .filter(|profile| profile_is_available(profile))
        .cloned()
        .collect::<Vec<_>>();
    available.sort_by(|left, right| left.name.cmp(&right.name));
    available.sort_by_key(|profile| profile.name != context.profile);

    available
        .into_iter()
        .map(|profile| ChannelCommandEntry {
            name: format!("profile.{}", profile.name),
            category: ChannelCommandCategory::Runtime,
            summary: format!(
                "Switch this conversation to {} ({}/{})",
                profile.name, profile.provider_type, profile.model
            ),
            usage: format!("/mosaic profile {}", profile.name),
            kind: ChannelCommandKind::Profile,
            allowed_channels: Vec::new(),
            required_policy: None,
            visibility: CapabilityVisibility::Visible,
            source: "workspace_config".to_owned(),
        })
        .collect()
}

fn tool_entries(
    components: &GatewayRuntimeComponents,
    context: &ChannelCommandContext,
) -> Vec<ChannelCommandEntry> {
    let mut capability_entries = components
        .tools
        .iter()
        .filter_map(|tool| {
            let metadata = tool.metadata().clone();
            if !catalog_exposure_allows(&metadata.exposure, &context.channel, &components.policies)
            {
                return None;
            }

            Some(ChannelCommandEntry {
                name: metadata.name.clone(),
                category: ChannelCommandCategory::Tools,
                summary: metadata.description.clone(),
                usage: tool_usage(&metadata),
                kind: ChannelCommandKind::Tool,
                allowed_channels: metadata.exposure.allowed_channels.clone(),
                required_policy: metadata.exposure.required_policy.clone(),
                visibility: metadata.exposure.visibility,
                source: normalized_tool_source(&metadata),
            })
        })
        .collect::<Vec<_>>();
    capability_entries.sort_by(|left, right| left.usage.cmp(&right.usage));

    if capability_entries.is_empty() {
        return capability_entries;
    }

    let mut entries = vec![control_entry(
        "tool",
        ChannelCommandCategory::Tools,
        "/mosaic tool <tool_name> <input>",
        "Run one of the currently available tools by name",
        ChannelCommandKind::Tool,
    )];
    entries.extend(capability_entries);
    entries
}

fn skill_entries(
    components: &GatewayRuntimeComponents,
    context: &ChannelCommandContext,
) -> Vec<ChannelCommandEntry> {
    let mut capability_entries = components
        .skills
        .list()
        .into_iter()
        .filter_map(|name| {
            let skill = components.skills.get(&name)?;
            let metadata = skill.metadata().clone();
            if !catalog_exposure_allows(&metadata.exposure, &context.channel, &components.policies)
            {
                return None;
            }
            Some(ChannelCommandEntry {
                name: metadata.name.clone(),
                category: ChannelCommandCategory::Skills,
                summary: skill_summary(&metadata),
                usage: format!("/mosaic skill {} <input>", metadata.name),
                kind: ChannelCommandKind::Skill,
                allowed_channels: metadata.exposure.allowed_channels.clone(),
                required_policy: metadata.exposure.required_policy.clone(),
                visibility: metadata.exposure.visibility,
                source: normalized_catalog_source(
                    &metadata.exposure.source,
                    metadata.extension.is_some(),
                    None,
                ),
            })
        })
        .collect::<Vec<_>>();
    capability_entries.sort_by(|left, right| left.usage.cmp(&right.usage));

    if capability_entries.is_empty() {
        return capability_entries;
    }

    let mut entries = vec![control_entry(
        "skill",
        ChannelCommandCategory::Skills,
        "/mosaic skill <skill_name> <input>",
        "Run one of the currently available skills by name",
        ChannelCommandKind::Skill,
    )];
    entries.extend(capability_entries);
    entries
}

fn workflow_entries(
    components: &GatewayRuntimeComponents,
    context: &ChannelCommandContext,
) -> Vec<ChannelCommandEntry> {
    let mut capability_entries = components
        .workflows
        .list()
        .into_iter()
        .filter_map(|name| {
            let workflow = components.workflows.get_registered(&name)?;
            if !catalog_exposure_allows(
                &workflow.metadata.exposure,
                &context.channel,
                &components.policies,
            ) {
                return None;
            }
            Some(ChannelCommandEntry {
                name: workflow.metadata.name.clone(),
                category: ChannelCommandCategory::Workflows,
                summary: workflow
                    .workflow
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Run workflow {}", workflow.metadata.name)),
                usage: format!("/mosaic workflow {} <input>", workflow.metadata.name),
                kind: ChannelCommandKind::Workflow,
                allowed_channels: workflow.metadata.exposure.allowed_channels.clone(),
                required_policy: workflow.metadata.exposure.required_policy.clone(),
                visibility: workflow.metadata.exposure.visibility,
                source: normalized_catalog_source(
                    &workflow.metadata.exposure.source,
                    workflow.metadata.extension.is_some(),
                    None,
                ),
            })
        })
        .collect::<Vec<_>>();
    capability_entries.sort_by(|left, right| left.usage.cmp(&right.usage));

    if capability_entries.is_empty() {
        return capability_entries;
    }

    let mut entries = vec![control_entry(
        "workflow",
        ChannelCommandCategory::Workflows,
        "/mosaic workflow <workflow_name> <input>",
        "Run one of the currently available workflows by name",
        ChannelCommandKind::Workflow,
    )];
    entries.extend(capability_entries);
    entries
}

fn control_entry(
    name: &str,
    category: ChannelCommandCategory,
    usage: &str,
    summary: &str,
    kind: ChannelCommandKind,
) -> ChannelCommandEntry {
    ChannelCommandEntry {
        name: name.to_owned(),
        category,
        summary: summary.to_owned(),
        usage: usage.to_owned(),
        kind,
        allowed_channels: Vec::new(),
        required_policy: None,
        visibility: CapabilityVisibility::Visible,
        source: "gateway.control".to_owned(),
    }
}

fn catalog_exposure_allows(
    exposure: &CapabilityExposure,
    channel: &str,
    policies: &PolicyConfig,
) -> bool {
    exposure.allows_explicit(Some(channel))
        && policy_allows(exposure.required_policy.as_deref(), policies)
}

fn policy_allows(required_policy: Option<&str>, policies: &PolicyConfig) -> bool {
    match required_policy {
        None => true,
        Some("allow_exec") => policies.allow_exec,
        Some("allow_webhook") => policies.allow_webhook,
        Some("allow_cron") => policies.allow_cron,
        Some("allow_mcp") => policies.allow_mcp,
        Some("hot_reload_enabled") => policies.hot_reload_enabled,
        Some(_) => true,
    }
}

fn profile_is_available(profile: &ProviderProfile) -> bool {
    profile.api_key_env.is_none() || profile.api_key_present()
}

fn normalized_tool_source(metadata: &ToolMetadata) -> String {
    normalized_catalog_source(
        &metadata.exposure.source,
        metadata.extension.is_some(),
        Some(&metadata.source),
    )
}

fn normalized_catalog_source(
    raw_source: &str,
    has_extension: bool,
    tool_source: Option<&ToolSource>,
) -> String {
    if has_extension && raw_source != "builtin" {
        return "extension".to_owned();
    }

    match raw_source {
        "gateway.control" => "gateway.control".to_owned(),
        "builtin" => "builtin".to_owned(),
        "workspace_config" | "app_config" => "workspace_config".to_owned(),
        value if value.starts_with("manifest:") => "extension".to_owned(),
        "" | "unknown" => match tool_source {
            Some(ToolSource::Builtin) | None => "builtin".to_owned(),
            Some(ToolSource::Mcp { .. }) => "workspace_config".to_owned(),
        },
        other => other.to_owned(),
    }
}

fn tool_usage(metadata: &ToolMetadata) -> String {
    let required = metadata
        .input_schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if required.is_empty() {
        return format!("/mosaic tool {}", metadata.name);
    }
    if required.len() == 1 {
        return format!(
            "/mosaic tool {} <{}>",
            metadata.name,
            property_placeholder(&required[0])
        );
    }
    format!("/mosaic tool {} <json>", metadata.name)
}

fn property_placeholder(property: &str) -> &str {
    match property {
        "text" | "input" => "input",
        other => other,
    }
}

fn skill_summary(metadata: &mosaic_skill_core::SkillMetadata) -> String {
    if metadata.name == "summarize" {
        return "Summarize the provided input".to_owned();
    }
    if metadata.manifest_backed {
        return format!("Run manifest skill {}", metadata.name);
    }
    format!("Run skill {}", metadata.name)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_memory::{FileMemoryStore, MemoryPolicy};
    use mosaic_provider::MockProvider;
    use mosaic_scheduler_core::FileCronStore;
    use mosaic_skill_core::SummarizeSkill;
    use mosaic_tool_core::{
        CapabilityExposure, CapabilityInvocationMode, CapabilityVisibility, TimeNowTool, Tool,
        ToolMetadata, ToolResult, ToolSource,
    };

    use super::*;

    #[derive(Clone)]
    struct CatalogTestTool {
        metadata: ToolMetadata,
    }

    #[async_trait]
    impl Tool for CatalogTestTool {
        fn metadata(&self) -> &ToolMetadata {
            &self.metadata
        }

        async fn call(&self, _input: serde_json::Value) -> Result<ToolResult> {
            Ok(ToolResult::ok("ok"))
        }
    }

    fn components() -> GatewayRuntimeComponents {
        let mut config = MosaicConfig::default();
        config.active_profile = "demo-provider".to_owned();
        config.profiles.insert(
            "demo-provider".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
                transport: Default::default(),
                vendor: Default::default(),
            },
        );
        let profiles =
            ProviderProfileRegistry::from_config(&config).expect("profile registry should build");

        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));
        tools.register(Arc::new(CatalogTestTool {
            metadata: ToolMetadata {
                name: "telegram_only".to_owned(),
                description: "Visible only on telegram".to_owned(),
                input_schema: serde_json::json!({ "type": "object" }),
                source: ToolSource::Builtin,
                capability: Default::default(),
                exposure: CapabilityExposure::new("workspace_config")
                    .with_invocation_mode(CapabilityInvocationMode::ExplicitOnly)
                    .with_allowed_channels(vec!["telegram".to_owned()]),
                extension: None,
                version: None,
                compatibility: Default::default(),
            },
        }));
        tools.register(Arc::new(CatalogTestTool {
            metadata: ToolMetadata {
                name: "hidden_exec".to_owned(),
                description: "Should not appear".to_owned(),
                input_schema: serde_json::json!({ "type": "object" }),
                source: ToolSource::Builtin,
                capability: Default::default(),
                exposure: CapabilityExposure::new("workspace_config")
                    .with_visibility(CapabilityVisibility::Hidden)
                    .with_required_policy(Some("allow_exec".to_owned())),
                extension: None,
                version: None,
                compatibility: Default::default(),
            },
        }));

        let mut skills = SkillRegistry::new();
        skills.register_native(Arc::new(SummarizeSkill));

        GatewayRuntimeComponents {
            profiles: Arc::new(profiles),
            provider_override: Some(Arc::new(MockProvider)),
            session_store: Arc::new(crate::tests::MemorySessionStore::default()),
            memory_store: Arc::new(FileMemoryStore::new(
                std::env::temp_dir().join("mosaic-command-catalog-memory"),
            )),
            memory_policy: MemoryPolicy::default(),
            runtime_policy: config.runtime.clone(),
            tools: Arc::new(tools),
            skills: Arc::new(skills),
            workflows: Arc::new(WorkflowRegistry::new()),
            node_store: Arc::new(FileNodeStore::new(
                std::env::temp_dir().join("mosaic-command-catalog-nodes"),
            )),
            mcp_manager: None,
            cron_store: Arc::new(FileCronStore::new(
                std::env::temp_dir().join("mosaic-command-catalog-cron"),
            )),
            workspace_root: std::env::temp_dir().join("mosaic-command-catalog-workspace"),
            runs_dir: std::env::temp_dir().join("mosaic-command-catalog-runs"),
            audit_root: std::env::temp_dir().join("mosaic-command-catalog-audit"),
            extensions: Vec::new(),
            policies: PolicyConfig::default(),
            deployment: config.deployment.clone(),
            auth: config.auth.clone(),
            audit: config.audit.clone(),
            observability: config.observability.clone(),
        }
    }

    #[test]
    fn filters_catalog_entries_by_channel_and_visibility() {
        let components = components();
        let webchat_catalog = build_command_catalog(
            &components,
            &ChannelCommandContext {
                channel: "webchat".to_owned(),
                session_id: Some("demo".to_owned()),
                profile: "demo-provider".to_owned(),
            },
            Some(ChannelCommandCategory::Tools),
        );
        assert!(
            webchat_catalog
                .entries
                .iter()
                .any(|entry| entry.usage == "/mosaic tool time_now")
        );
        assert!(
            !webchat_catalog
                .entries
                .iter()
                .any(|entry| entry.usage.contains("telegram_only"))
        );
        assert!(
            !webchat_catalog
                .entries
                .iter()
                .any(|entry| entry.usage.contains("hidden_exec"))
        );

        let telegram_catalog = build_command_catalog(
            &components,
            &ChannelCommandContext {
                channel: "telegram".to_owned(),
                session_id: Some("demo".to_owned()),
                profile: "demo-provider".to_owned(),
            },
            Some(ChannelCommandCategory::Tools),
        );
        assert!(
            telegram_catalog
                .entries
                .iter()
                .any(|entry| entry.usage == "/mosaic tool telegram_only")
        );
    }

    #[test]
    fn renders_grouped_help_output() {
        let components = components();
        let catalog = build_command_catalog(
            &components,
            &ChannelCommandContext {
                channel: "telegram".to_owned(),
                session_id: Some("telegram-42".to_owned()),
                profile: "demo-provider".to_owned(),
            },
            None,
        );
        let rendered = catalog.render();

        assert!(rendered.contains("Mosaic commands available in this conversation."));
        assert!(
            rendered
                .contains("scope: channel=telegram, session=telegram-42, profile=demo-provider")
        );
        assert!(rendered.contains("\nSession\n"));
        assert!(rendered.contains("\nRuntime\n"));
        assert!(rendered.contains("\nTools\n"));
        assert!(rendered.contains("\nSkills\n"));
        assert!(rendered.contains("\nGateway\n"));
    }
}
