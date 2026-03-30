use super::*;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const ACTIVE_PROFILE_ENV: &str = "MOSAIC_ACTIVE_PROFILE";
pub const DEFAULT_PRODUCT_ACTIVE_PROFILE: &str = "gpt-5.4-mini";
pub const DEV_MOCK_PROFILE: &str = "mock";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderUsage {
    FirstClassReal,
    Compatibility,
    DevOnlyMock,
}

impl ProviderUsage {
    pub fn label(self) -> &'static str {
        match self {
            Self::FirstClassReal => "first-class-real",
            Self::Compatibility => "compatibility",
            Self::DevOnlyMock => "dev-only-mock",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Mock,
    OpenAi,
    Azure,
    Anthropic,
    Ollama,
    OpenAiCompatible,
}

impl ProviderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::OpenAi => "openai",
            Self::Azure => "azure",
            Self::Anthropic => "anthropic",
            Self::Ollama => "ollama",
            Self::OpenAiCompatible => "openai-compatible",
        }
    }

    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            Self::Mock => None,
            Self::OpenAi => Some("https://api.openai.com/v1"),
            Self::Azure => None,
            Self::Anthropic => Some("https://api.anthropic.com/v1"),
            Self::Ollama => Some("http://127.0.0.1:11434"),
            Self::OpenAiCompatible => Some("https://api.openai.com/v1"),
        }
    }

    pub fn requires_api_key(self) -> bool {
        matches!(
            self,
            Self::OpenAi | Self::Azure | Self::Anthropic | Self::OpenAiCompatible
        )
    }

    pub fn requires_explicit_base_url(self) -> bool {
        matches!(self, Self::Azure)
    }

    pub fn usage(self) -> ProviderUsage {
        match self {
            Self::Mock => ProviderUsage::DevOnlyMock,
            Self::OpenAiCompatible => ProviderUsage::Compatibility,
            Self::OpenAi | Self::Azure | Self::Anthropic | Self::Ollama => {
                ProviderUsage::FirstClassReal
            }
        }
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn parse_provider_type(value: &str) -> Option<ProviderType> {
    match value {
        "mock" => Some(ProviderType::Mock),
        "openai" => Some(ProviderType::OpenAi),
        "azure" => Some(ProviderType::Azure),
        "anthropic" => Some(ProviderType::Anthropic),
        "ollama" => Some(ProviderType::Ollama),
        "openai-compatible" => Some(ProviderType::OpenAiCompatible),
        _ => None,
    }
}

pub fn supported_provider_types() -> &'static [&'static str] {
    &[
        "mock",
        "openai",
        "azure",
        "anthropic",
        "ollama",
        "openai-compatible",
    ]
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub app: Option<AppSection>,
    pub provider: ProviderConfig,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
    #[serde(default)]
    pub workflows: Vec<Workflow>,
    pub agent: AgentConfig,
    pub task: TaskConfig,
    pub mcp: Option<McpConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSection {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolConfig {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(default)]
    pub visibility: CapabilityVisibility,
    #[serde(default)]
    pub invocation_mode: CapabilityInvocationMode,
    #[serde(default)]
    pub required_policy: Option<String>,
    #[serde(default)]
    pub allowed_channels: Vec<String>,
    #[serde(default)]
    pub accepts_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillConfig {
    #[serde(rename = "type")]
    pub skill_type: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_skill_input_schema")]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub tools: Vec<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub steps: Vec<ManifestSkillStep>,
    #[serde(default)]
    pub visibility: CapabilityVisibility,
    #[serde(default)]
    pub invocation_mode: CapabilityInvocationMode,
    #[serde(default)]
    pub required_policy: Option<String>,
    #[serde(default)]
    pub allowed_channels: Vec<String>,
    #[serde(default)]
    pub accepts_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentConfig {
    pub system: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskConfig {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MosaicConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_active_profile")]
    pub active_profile: String,
    #[serde(default = "default_profiles")]
    pub profiles: BTreeMap<String, ProviderProfileConfig>,
    #[serde(default)]
    pub provider_defaults: ProviderTransportPolicyConfig,
    #[serde(default)]
    pub deployment: DeploymentConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub session_store: SessionStoreConfig,
    #[serde(default)]
    pub inspect: InspectConfig,
    #[serde(default)]
    pub audit: AuditConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub runtime: RuntimePolicyConfig,
    #[serde(default)]
    pub attachments: AttachmentConfig,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
    #[serde(default)]
    pub workflows: Vec<Workflow>,
    pub mcp: Option<McpConfig>,
    #[serde(default)]
    pub extensions: ExtensionsConfig,
    #[serde(default)]
    pub policies: PolicyConfig,
}

impl Default for MosaicConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            active_profile: default_active_profile(),
            profiles: default_profiles(),
            provider_defaults: ProviderTransportPolicyConfig::default(),
            deployment: DeploymentConfig::default(),
            auth: AuthConfig::default(),
            session_store: SessionStoreConfig::default(),
            inspect: InspectConfig::default(),
            audit: AuditConfig::default(),
            observability: ObservabilityConfig::default(),
            runtime: RuntimePolicyConfig::default(),
            attachments: AttachmentConfig::default(),
            tools: Vec::new(),
            skills: Vec::new(),
            workflows: Vec::new(),
            mcp: None,
            extensions: ExtensionsConfig::default(),
            policies: PolicyConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfileConfig {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub transport: ProviderTransportPolicyConfig,
    #[serde(default)]
    pub vendor: ProviderVendorPolicyConfig,
    #[serde(default)]
    pub attachments: ProviderAttachmentRoutingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProviderTransportPolicyConfig {
    pub timeout_ms: Option<u64>,
    pub max_retries: Option<u8>,
    pub retry_backoff_ms: Option<u64>,
    #[serde(default)]
    pub custom_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProviderVendorPolicyConfig {
    pub azure_api_version: Option<String>,
    pub anthropic_version: Option<String>,
    #[serde(default)]
    pub allow_custom_headers: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentRouteModeConfig {
    #[default]
    ProviderNative,
    SpecializedProcessor,
    Disabled,
}

impl AttachmentRouteModeConfig {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProviderNative => "provider_native",
            Self::SpecializedProcessor => "specialized_processor",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentRoutingTargetConfig {
    #[serde(default)]
    pub mode: AttachmentRouteModeConfig,
    #[serde(default)]
    pub processor: Option<String>,
}

impl Default for AttachmentRoutingTargetConfig {
    fn default() -> Self {
        Self {
            mode: AttachmentRouteModeConfig::ProviderNative,
            processor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProviderAttachmentRoutingConfig {
    #[serde(default)]
    pub mode: Option<AttachmentRouteModeConfig>,
    #[serde(default)]
    pub processor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentRoutingConfig {
    #[serde(default)]
    pub default: AttachmentRoutingTargetConfig,
    #[serde(default)]
    pub channel_overrides: BTreeMap<String, AttachmentRoutingTargetConfig>,
    #[serde(default)]
    pub bot_overrides: BTreeMap<String, AttachmentRoutingTargetConfig>,
}

impl Default for AttachmentRoutingConfig {
    fn default() -> Self {
        Self {
            default: AttachmentRoutingTargetConfig::default(),
            channel_overrides: BTreeMap::new(),
            bot_overrides: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttachmentPolicyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_attachment_cache_dir")]
    pub cache_dir: String,
    #[serde(default = "default_attachment_max_size_bytes")]
    pub max_size_bytes: u64,
    #[serde(default = "default_attachment_download_timeout_ms")]
    pub download_timeout_ms: u64,
    #[serde(default)]
    pub allowed_mime_types: Vec<String>,
    #[serde(default = "default_attachment_cleanup_after_hours")]
    pub cleanup_after_hours: u64,
}

impl Default for AttachmentPolicyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_dir: default_attachment_cache_dir(),
            max_size_bytes: default_attachment_max_size_bytes(),
            download_timeout_ms: default_attachment_download_timeout_ms(),
            allowed_mime_types: default_allowed_attachment_mime_types(),
            cleanup_after_hours: default_attachment_cleanup_after_hours(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AttachmentConfig {
    #[serde(default)]
    pub policy: AttachmentPolicyConfig,
    #[serde(default)]
    pub routing: AttachmentRoutingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePolicyConfig {
    #[serde(default = "default_max_provider_round_trips")]
    pub max_provider_round_trips: usize,
    #[serde(default = "default_max_workflow_provider_round_trips")]
    pub max_workflow_provider_round_trips: usize,
    #[serde(default)]
    pub continue_after_tool_error: bool,
}

impl Default for RuntimePolicyConfig {
    fn default() -> Self {
        Self {
            max_provider_round_trips: default_max_provider_round_trips(),
            max_workflow_provider_round_trips: default_max_workflow_provider_round_trips(),
            continue_after_tool_error: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeploymentConfig {
    #[serde(default = "default_deployment_profile")]
    pub profile: String,
    #[serde(default = "default_workspace_name")]
    pub workspace_name: String,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            profile: default_deployment_profile(),
            workspace_name: default_workspace_name(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AuthConfig {
    pub operator_token_env: Option<String>,
    pub webchat_shared_secret_env: Option<String>,
    pub telegram_secret_token_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionStoreConfig {
    #[serde(default = "default_session_store_root_dir")]
    pub root_dir: String,
}

impl Default for SessionStoreConfig {
    fn default() -> Self {
        Self {
            root_dir: default_session_store_root_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectConfig {
    #[serde(default = "default_runs_root_dir")]
    pub runs_dir: String,
}

impl Default for InspectConfig {
    fn default() -> Self {
        Self {
            runs_dir: default_runs_root_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditConfig {
    #[serde(default = "default_audit_root_dir")]
    pub root_dir: String,
    #[serde(default = "default_audit_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_event_replay_window")]
    pub event_replay_window: usize,
    #[serde(default = "default_true")]
    pub redact_inputs: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            root_dir: default_audit_root_dir(),
            retention_days: default_audit_retention_days(),
            event_replay_window: default_event_replay_window(),
            redact_inputs: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservabilityConfig {
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    #[serde(default = "default_true")]
    pub enable_readiness: bool,
    #[serde(default = "default_slow_consumer_lag_threshold")]
    pub slow_consumer_lag_threshold: usize,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            enable_readiness: true,
            slow_consumer_lag_threshold: default_slow_consumer_lag_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub manifests: Vec<ExtensionManifestRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionManifestRef {
    pub path: String,
    pub version_pin: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyConfig {
    #[serde(default = "default_true")]
    pub allow_exec: bool,
    #[serde(default = "default_true")]
    pub allow_webhook: bool,
    #[serde(default = "default_true")]
    pub allow_cron: bool,
    #[serde(default = "default_true")]
    pub allow_mcp: bool,
    #[serde(default = "default_true")]
    pub hot_reload_enabled: bool,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            allow_exec: true,
            allow_webhook: true,
            allow_cron: true,
            allow_mcp: true,
            hot_reload_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadConfigOptions {
    pub cwd: PathBuf,
    pub user_config_path: Option<PathBuf>,
    pub workspace_config_path: Option<PathBuf>,
    pub overrides: ConfigOverrides,
}

impl Default for LoadConfigOptions {
    fn default() -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            user_config_path: None,
            workspace_config_path: None,
            overrides: ConfigOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConfigSourceKind {
    Defaults,
    User,
    Workspace,
    Env,
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSource {
    pub kind: ConfigSourceKind,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadedMosaicConfig {
    pub config: MosaicConfig,
    pub sources: Vec<ConfigSource>,
    pub workspace_config_path: PathBuf,
    pub user_config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| matches!(issue.level, ValidationLevel::Error))
    }

    pub fn is_ok(&self) -> bool {
        !self.has_errors()
    }

    pub fn errors(&self) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|issue| matches!(issue.level, ValidationLevel::Error))
            .collect()
    }

    pub fn warnings(&self) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|issue| matches!(issue.level, ValidationLevel::Warning))
            .collect()
    }

    pub(crate) fn push(
        &mut self,
        level: ValidationLevel,
        field: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issues.push(ValidationIssue {
            level,
            field: field.into(),
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DoctorStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum DoctorCategory {
    Storage,
    Auth,
    Extensions,
    Providers,
}

impl DoctorCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Storage => "storage",
            Self::Auth => "auth",
            Self::Extensions => "extensions",
            Self::Providers => "providers",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorCheck {
    pub status: DoctorStatus,
    pub category: DoctorCategory,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorReport {
    pub validation: ValidationReport,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn has_errors(&self) -> bool {
        self.validation.has_errors()
            || self
                .checks
                .iter()
                .any(|check| matches!(check.status, DoctorStatus::Error))
    }

    pub fn summary(&self) -> DoctorSummary {
        let mut summary = DoctorSummary::default();

        for check in &self.checks {
            match check.status {
                DoctorStatus::Ok => summary.ok += 1,
                DoctorStatus::Warning => summary.warnings += 1,
                DoctorStatus::Error => summary.errors += 1,
            }

            if let Some(category) = summary
                .categories
                .iter_mut()
                .find(|entry| entry.category == check.category)
            {
                match check.status {
                    DoctorStatus::Ok => category.ok += 1,
                    DoctorStatus::Warning => category.warnings += 1,
                    DoctorStatus::Error => category.errors += 1,
                }
            } else {
                let mut category = DoctorCategorySummary {
                    category: check.category,
                    ok: 0,
                    warnings: 0,
                    errors: 0,
                };
                match check.status {
                    DoctorStatus::Ok => category.ok = 1,
                    DoctorStatus::Warning => category.warnings = 1,
                    DoctorStatus::Error => category.errors = 1,
                }
                summary.categories.push(category);
            }
        }

        summary.categories.sort_by_key(|entry| entry.category);
        summary
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DoctorSummary {
    pub ok: usize,
    pub warnings: usize,
    pub errors: usize,
    #[serde(default)]
    pub categories: Vec<DoctorCategorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorCategorySummary {
    pub category: DoctorCategory,
    pub ok: usize,
    pub warnings: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedProfileView {
    pub name: String,
    pub provider_type: String,
    pub usage: ProviderUsage,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub api_key_present: bool,
    pub timeout_ms: Option<u64>,
    pub max_retries: Option<u8>,
    pub retry_backoff_ms: Option<u64>,
    pub custom_header_keys: Vec<String>,
    pub allow_custom_headers: bool,
    pub azure_api_version: Option<String>,
    pub anthropic_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedProviderDefaultsView {
    pub timeout_ms: Option<u64>,
    pub max_retries: Option<u8>,
    pub retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedRuntimePolicyView {
    pub max_provider_round_trips: usize,
    pub max_workflow_provider_round_trips: usize,
    pub continue_after_tool_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedPolicyView {
    pub allow_exec: bool,
    pub allow_webhook: bool,
    pub allow_cron: bool,
    pub allow_mcp: bool,
    pub hot_reload_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedDeploymentView {
    pub profile: String,
    pub workspace_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedAuthView {
    pub operator_token_env: Option<String>,
    pub operator_token_present: bool,
    pub webchat_shared_secret_env: Option<String>,
    pub webchat_shared_secret_present: bool,
    pub telegram_secret_token_env: Option<String>,
    pub telegram_secret_token_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedAuditView {
    pub root_dir: String,
    pub retention_days: u32,
    pub event_replay_window: usize,
    pub redact_inputs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedObservabilityView {
    pub enable_metrics: bool,
    pub enable_readiness: bool,
    pub slow_consumer_lag_threshold: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedMosaicConfig {
    pub schema_version: u32,
    pub active_profile: String,
    pub profiles: Vec<RedactedProfileView>,
    pub provider_defaults: RedactedProviderDefaultsView,
    pub deployment: RedactedDeploymentView,
    pub auth: RedactedAuthView,
    pub session_store_root_dir: String,
    pub inspect_runs_dir: String,
    pub audit: RedactedAuditView,
    pub observability: RedactedObservabilityView,
    pub runtime: RedactedRuntimePolicyView,
    pub attachments: RedactedAttachmentView,
    pub extension_manifest_count: usize,
    pub policies: RedactedPolicyView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedAttachmentView {
    pub enabled: bool,
    pub cache_dir: String,
    pub max_size_bytes: u64,
    pub download_timeout_ms: u64,
    pub cleanup_after_hours: u64,
    #[serde(default)]
    pub allowed_mime_types: Vec<String>,
    pub default_route_mode: AttachmentRouteModeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(crate) struct MosaicConfigPatch {
    pub schema_version: Option<u32>,
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProviderProfileConfig>,
    pub provider_defaults: Option<ProviderTransportPolicyConfig>,
    pub deployment: Option<DeploymentConfig>,
    pub auth: Option<AuthConfig>,
    pub session_store: Option<SessionStoreConfig>,
    pub inspect: Option<InspectConfig>,
    pub audit: Option<AuditConfig>,
    pub observability: Option<ObservabilityConfig>,
    pub runtime: Option<RuntimePolicyConfig>,
    pub attachments: Option<AttachmentConfig>,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
    #[serde(default)]
    pub workflows: Vec<Workflow>,
    pub mcp: Option<McpConfig>,
    pub extensions: Option<ExtensionsConfig>,
    pub policies: Option<PolicyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
    #[serde(default)]
    pub workflows: Vec<Workflow>,
    pub mcp: Option<McpConfig>,
}

fn default_skill_input_schema() -> serde_json::Value {
    serde_json::json!({ "type": "object" })
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn default_active_profile() -> String {
    DEFAULT_PRODUCT_ACTIVE_PROFILE.to_owned()
}

fn default_deployment_profile() -> String {
    "local".to_owned()
}

fn default_workspace_name() -> String {
    "default".to_owned()
}

fn default_profiles() -> BTreeMap<String, ProviderProfileConfig> {
    BTreeMap::from([
        (
            "anthropic-sonnet".to_owned(),
            ProviderProfileConfig {
                provider_type: "anthropic".to_owned(),
                model: "claude-sonnet-4-5".to_owned(),
                base_url: Some("https://api.anthropic.com/v1".to_owned()),
                api_key_env: Some("ANTHROPIC_API_KEY".to_owned()),
                transport: ProviderTransportPolicyConfig::default(),
                vendor: ProviderVendorPolicyConfig::default(),
                attachments: ProviderAttachmentRoutingConfig::default(),
            },
        ),
        (
            "azure-gpt-5.4".to_owned(),
            ProviderProfileConfig {
                provider_type: "azure".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: Some("https://your-resource.openai.azure.com".to_owned()),
                api_key_env: Some("AZURE_OPENAI_API_KEY".to_owned()),
                transport: ProviderTransportPolicyConfig::default(),
                vendor: ProviderVendorPolicyConfig::default(),
                attachments: ProviderAttachmentRoutingConfig::default(),
            },
        ),
        (
            "gpt-5.4".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
                transport: ProviderTransportPolicyConfig::default(),
                vendor: ProviderVendorPolicyConfig::default(),
                attachments: ProviderAttachmentRoutingConfig::default(),
            },
        ),
        (
            "gpt-5.4-mini".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai".to_owned(),
                model: "gpt-5.4-mini".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
                transport: ProviderTransportPolicyConfig::default(),
                vendor: ProviderVendorPolicyConfig::default(),
                attachments: ProviderAttachmentRoutingConfig::default(),
            },
        ),
        (
            "mock".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
                transport: ProviderTransportPolicyConfig::default(),
                vendor: ProviderVendorPolicyConfig::default(),
                attachments: ProviderAttachmentRoutingConfig::default(),
            },
        ),
        (
            "ollama-qwen3".to_owned(),
            ProviderProfileConfig {
                provider_type: "ollama".to_owned(),
                model: "qwen3:14b".to_owned(),
                base_url: Some("http://127.0.0.1:11434".to_owned()),
                api_key_env: None,
                transport: ProviderTransportPolicyConfig::default(),
                vendor: ProviderVendorPolicyConfig::default(),
                attachments: ProviderAttachmentRoutingConfig::default(),
            },
        ),
    ])
}

fn default_session_store_root_dir() -> String {
    ".mosaic/sessions".to_owned()
}

fn default_runs_root_dir() -> String {
    ".mosaic/runs".to_owned()
}

fn default_attachment_cache_dir() -> String {
    ".mosaic/attachments".to_owned()
}

fn default_attachment_max_size_bytes() -> u64 {
    10 * 1024 * 1024
}

fn default_attachment_download_timeout_ms() -> u64 {
    15_000
}

fn default_attachment_cleanup_after_hours() -> u64 {
    24
}

fn default_allowed_attachment_mime_types() -> Vec<String> {
    vec![
        "image/".to_owned(),
        "text/".to_owned(),
        "application/pdf".to_owned(),
        "application/json".to_owned(),
        "application/octet-stream".to_owned(),
    ]
}

fn default_audit_root_dir() -> String {
    ".mosaic/audit".to_owned()
}

fn default_audit_retention_days() -> u32 {
    14
}

fn default_event_replay_window() -> usize {
    256
}

fn default_slow_consumer_lag_threshold() -> usize {
    32
}

fn default_max_provider_round_trips() -> usize {
    8
}

fn default_max_workflow_provider_round_trips() -> usize {
    8
}
