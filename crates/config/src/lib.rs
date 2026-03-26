use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use mosaic_skill_core::ManifestSkillStep;
use mosaic_workflow::Workflow;
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const ACTIVE_PROFILE_ENV: &str = "MOSAIC_ACTIVE_PROFILE";

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
            deployment: DeploymentConfig::default(),
            auth: AuthConfig::default(),
            session_store: SessionStoreConfig::default(),
            inspect: InspectConfig::default(),
            audit: AuditConfig::default(),
            observability: ObservabilityConfig::default(),
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

    fn push(
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
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub api_key_present: bool,
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
    pub deployment: RedactedDeploymentView,
    pub auth: RedactedAuthView,
    pub session_store_root_dir: String,
    pub inspect_runs_dir: String,
    pub audit: RedactedAuditView,
    pub observability: RedactedObservabilityView,
    pub extension_manifest_count: usize,
    pub policies: RedactedPolicyView,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
struct MosaicConfigPatch {
    pub schema_version: Option<u32>,
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProviderProfileConfig>,
    pub deployment: Option<DeploymentConfig>,
    pub auth: Option<AuthConfig>,
    pub session_store: Option<SessionStoreConfig>,
    pub inspect: Option<InspectConfig>,
    pub audit: Option<AuditConfig>,
    pub observability: Option<ObservabilityConfig>,
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

pub fn load_from_file(path: impl AsRef<Path>) -> Result<AppConfig> {
    let content = fs::read_to_string(path)?;
    let cfg = serde_yaml::from_str::<AppConfig>(&content)?;
    Ok(cfg)
}

pub fn load_extension_manifest_from_file(path: impl AsRef<Path>) -> Result<ExtensionManifest> {
    let content = fs::read_to_string(path)?;
    let cfg = serde_yaml::from_str::<ExtensionManifest>(&content)?;
    Ok(cfg)
}

pub fn default_workspace_config_path(cwd: impl AsRef<Path>) -> PathBuf {
    cwd.as_ref().join(".mosaic").join("config.yaml")
}

pub fn default_user_config_path() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/mosaic/config.yaml"))
}

pub fn default_config_template() -> Result<String> {
    let body = serde_yaml::to_string(&MosaicConfig::default())?;
    Ok(format!(
        "# Mosaic workspace configuration\n# Generated by `mosaic setup init`.\n\n{body}"
    ))
}

pub fn save_mosaic_config(path: impl AsRef<Path>, config: &MosaicConfig) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let body = serde_yaml::to_string(config)?;
    fs::write(path, body)?;
    Ok(())
}

pub fn init_workspace_config(cwd: impl AsRef<Path>, force: bool) -> Result<PathBuf> {
    let cwd = cwd.as_ref();
    let path = default_workspace_config_path(cwd);

    if path.exists() && !force {
        bail!(
            "workspace config already exists at {} (pass --force to overwrite)",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let config = MosaicConfig::default();
    save_mosaic_config(&path, &config)?;
    fs::create_dir_all(cwd.join(&config.session_store.root_dir))?;
    fs::create_dir_all(cwd.join(&config.inspect.runs_dir))?;
    fs::create_dir_all(cwd.join(&config.audit.root_dir))?;
    fs::create_dir_all(cwd.join(".mosaic/extensions"))?;

    Ok(path)
}

pub fn load_mosaic_config(options: &LoadConfigOptions) -> Result<LoadedMosaicConfig> {
    let mut config = MosaicConfig::default();
    let mut sources = vec![ConfigSource {
        kind: ConfigSourceKind::Defaults,
        detail: "built-in defaults".to_owned(),
    }];

    let user_config_path = options
        .user_config_path
        .clone()
        .or_else(default_user_config_path);
    if let Some(user_path) = user_config_path.as_ref().filter(|path| path.exists()) {
        let patch = load_config_patch(user_path)?;
        merge_patch(&mut config, patch);
        sources.push(ConfigSource {
            kind: ConfigSourceKind::User,
            detail: user_path.display().to_string(),
        });
    }

    let workspace_config_path = options
        .workspace_config_path
        .clone()
        .unwrap_or_else(|| default_workspace_config_path(&options.cwd));
    if workspace_config_path.exists() {
        let patch = load_config_patch(&workspace_config_path)?;
        merge_patch(&mut config, patch);
        sources.push(ConfigSource {
            kind: ConfigSourceKind::Workspace,
            detail: workspace_config_path.display().to_string(),
        });
    }

    if let Ok(active_profile) = env::var(ACTIVE_PROFILE_ENV) {
        if !active_profile.trim().is_empty() {
            config.active_profile = active_profile;
            sources.push(ConfigSource {
                kind: ConfigSourceKind::Env,
                detail: ACTIVE_PROFILE_ENV.to_owned(),
            });
        }
    }

    if let Some(active_profile) = options.overrides.active_profile.clone() {
        config.active_profile = active_profile;
        sources.push(ConfigSource {
            kind: ConfigSourceKind::Cli,
            detail: "active profile override".to_owned(),
        });
    }

    Ok(LoadedMosaicConfig {
        config,
        sources,
        workspace_config_path,
        user_config_path,
    })
}

pub fn validate_mosaic_config(config: &MosaicConfig) -> ValidationReport {
    let mut report = ValidationReport::default();

    if config.schema_version != CURRENT_SCHEMA_VERSION {
        report.push(
            ValidationLevel::Error,
            "schema_version",
            format!(
                "unsupported schema_version {}; expected {}",
                config.schema_version, CURRENT_SCHEMA_VERSION
            ),
        );
    }

    if config.profiles.is_empty() {
        report.push(
            ValidationLevel::Error,
            "profiles",
            "at least one provider profile must be configured",
        );
    }

    if config.deployment.profile.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "deployment.profile",
            "deployment profile must not be empty",
        );
    } else if !matches!(
        config.deployment.profile.as_str(),
        "local" | "staging" | "production"
    ) {
        report.push(
            ValidationLevel::Error,
            "deployment.profile",
            format!(
                "unsupported deployment profile '{}': expected local, staging, or production",
                config.deployment.profile
            ),
        );
    }

    if config.deployment.workspace_name.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "deployment.workspace_name",
            "deployment workspace_name must not be empty",
        );
    }

    for (field, value) in [
        (
            "auth.operator_token_env",
            config.auth.operator_token_env.as_deref(),
        ),
        (
            "auth.webchat_shared_secret_env",
            config.auth.webchat_shared_secret_env.as_deref(),
        ),
        (
            "auth.telegram_secret_token_env",
            config.auth.telegram_secret_token_env.as_deref(),
        ),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            report.push(
                ValidationLevel::Error,
                field,
                "environment variable name must not be empty when provided",
            );
        }
    }

    if !config.profiles.contains_key(&config.active_profile) {
        report.push(
            ValidationLevel::Error,
            "active_profile",
            format!(
                "active_profile '{}' does not match any configured profile",
                config.active_profile
            ),
        );
    }

    for (name, profile) in &config.profiles {
        let field_prefix = format!("profiles.{name}");

        if profile.provider_type.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.type"),
                "provider type must not be empty",
            );
            continue;
        }

        let provider_type = match parse_provider_type(&profile.provider_type) {
            Some(provider_type) => provider_type,
            None => {
                report.push(
                    ValidationLevel::Error,
                    format!("{field_prefix}.type"),
                    format!(
                        "unsupported provider type '{}': expected one of {}",
                        profile.provider_type,
                        supported_provider_types().join(", ")
                    ),
                );
                continue;
            }
        };

        if profile.model.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.model"),
                "model must not be empty",
            );
        }

        if profile
            .base_url
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.base_url"),
                "base_url must not be empty when provided",
            );
        }

        if profile
            .api_key_env
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.api_key_env"),
                "environment variable name must not be empty when provided",
            );
        }

        if provider_type.requires_explicit_base_url()
            && profile
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.base_url"),
                format!("{} profiles require base_url", provider_type),
            );
        }

        if provider_type.requires_api_key()
            && profile
                .api_key_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.api_key_env"),
                format!("{} profiles require api_key_env", provider_type),
            );
        }
    }

    if config.session_store.root_dir.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "session_store.root_dir",
            "session store root directory must not be empty",
        );
    }

    if config.inspect.runs_dir.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "inspect.runs_dir",
            "inspect runs directory must not be empty",
        );
    }

    if config.audit.root_dir.trim().is_empty() {
        report.push(
            ValidationLevel::Error,
            "audit.root_dir",
            "audit root directory must not be empty",
        );
    }

    if config.audit.retention_days == 0 {
        report.push(
            ValidationLevel::Error,
            "audit.retention_days",
            "audit retention_days must be greater than zero",
        );
    }

    if config.audit.event_replay_window == 0 {
        report.push(
            ValidationLevel::Error,
            "audit.event_replay_window",
            "audit event_replay_window must be greater than zero",
        );
    }

    if config.observability.slow_consumer_lag_threshold == 0 {
        report.push(
            ValidationLevel::Error,
            "observability.slow_consumer_lag_threshold",
            "observability slow_consumer_lag_threshold must be greater than zero",
        );
    }

    if config.deployment.profile == "production"
        && config
            .auth
            .operator_token_env
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        report.push(
            ValidationLevel::Error,
            "auth.operator_token_env",
            "production deployment requires auth.operator_token_env",
        );
    }

    if config.deployment.profile == "production" && !config.audit.redact_inputs {
        report.push(
            ValidationLevel::Warning,
            "audit.redact_inputs",
            "production deployment should keep audit.redact_inputs enabled",
        );
    }

    for (idx, manifest) in config.extensions.manifests.iter().enumerate() {
        if manifest.path.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("extensions.manifests.{idx}.path"),
                "extension manifest path must not be empty",
            );
        }

        if manifest
            .version_pin
            .as_deref()
            .is_some_and(|version| version.trim().is_empty())
        {
            report.push(
                ValidationLevel::Error,
                format!("extensions.manifests.{idx}.version_pin"),
                "extension manifest version_pin must not be empty when provided",
            );
        }
    }

    report
}

pub fn doctor_mosaic_config(config: &MosaicConfig, cwd: impl AsRef<Path>) -> DoctorReport {
    let cwd = cwd.as_ref();
    let validation = validate_mosaic_config(config);
    let mut checks = Vec::new();

    let session_root = cwd.join(&config.session_store.root_dir);
    checks.push(path_check(
        &session_root,
        DoctorCategory::Storage,
        "session store directory",
        true,
    ));

    let runs_root = cwd.join(&config.inspect.runs_dir);
    checks.push(path_check(
        &runs_root,
        DoctorCategory::Storage,
        "run trace directory",
        true,
    ));

    let audit_root = cwd.join(&config.audit.root_dir);
    checks.push(path_check(
        &audit_root,
        DoctorCategory::Storage,
        "audit directory",
        true,
    ));

    checks.push(secret_env_check(
        DoctorCategory::Auth,
        config.auth.operator_token_env.as_deref(),
        "operator auth token",
        config.deployment.profile == "production",
    ));
    checks.push(secret_env_check(
        DoctorCategory::Auth,
        config.auth.webchat_shared_secret_env.as_deref(),
        "webchat ingress shared secret",
        false,
    ));
    checks.push(secret_env_check(
        DoctorCategory::Auth,
        config.auth.telegram_secret_token_env.as_deref(),
        "telegram ingress secret token",
        false,
    ));

    for manifest in &config.extensions.manifests {
        let manifest_path = cwd.join(&manifest.path);
        checks.push(path_check(
            &manifest_path,
            DoctorCategory::Extensions,
            "extension manifest",
            false,
        ));
    }

    for (name, profile) in &config.profiles {
        let Some(provider_type) = parse_provider_type(&profile.provider_type) else {
            checks.push(DoctorCheck {
                status: DoctorStatus::Error,
                category: DoctorCategory::Providers,
                message: format!(
                    "profile '{}' uses unsupported provider type '{}'",
                    name, profile.provider_type
                ),
            });
            continue;
        };

        match provider_type {
            ProviderType::Mock => checks.push(DoctorCheck {
                status: DoctorStatus::Ok,
                category: DoctorCategory::Providers,
                message: format!(
                    "profile '{}' uses the mock provider and does not require API credentials",
                    name
                ),
            }),
            _ => {
                let configured_base_url = profile
                    .base_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                match configured_base_url.or_else(|| provider_type.default_base_url()) {
                    Some(base_url) => checks.push(DoctorCheck {
                        status: DoctorStatus::Ok,
                        category: DoctorCategory::Providers,
                        message: if configured_base_url.is_some() {
                            format!(
                                "profile '{}' uses {} base URL {}",
                                name, provider_type, base_url
                            )
                        } else {
                            format!(
                                "profile '{}' defaults to {} base URL {}",
                                name, provider_type, base_url
                            )
                        },
                    }),
                    None => checks.push(DoctorCheck {
                        status: DoctorStatus::Error,
                        category: DoctorCategory::Providers,
                        message: format!(
                            "profile '{}' requires an explicit {} base_url",
                            name, provider_type
                        ),
                    }),
                }

                if provider_type.requires_api_key() {
                    match profile
                        .api_key_env
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        Some(api_key_env) if env::var(api_key_env).is_ok() => {
                            checks.push(DoctorCheck {
                                status: DoctorStatus::Ok,
                                category: DoctorCategory::Providers,
                                message: format!(
                                    "profile '{}' has {} available in the environment",
                                    name, api_key_env
                                ),
                            })
                        }
                        Some(api_key_env) => checks.push(DoctorCheck {
                            status: if name == &config.active_profile {
                                DoctorStatus::Error
                            } else {
                                DoctorStatus::Warning
                            },
                            category: DoctorCategory::Providers,
                            message: format!(
                                "profile '{}' expects environment variable {} to be set",
                                name, api_key_env
                            ),
                        }),
                        None => checks.push(DoctorCheck {
                            status: DoctorStatus::Error,
                            category: DoctorCategory::Providers,
                            message: format!(
                                "profile '{}' is missing api_key_env for {}",
                                name, provider_type
                            ),
                        }),
                    }
                } else {
                    checks.push(DoctorCheck {
                        status: DoctorStatus::Ok,
                        category: DoctorCategory::Providers,
                        message: format!("profile '{}' does not require API credentials", name),
                    });
                }
            }
        }
    }

    DoctorReport { validation, checks }
}

pub fn redact_mosaic_config(config: &MosaicConfig) -> RedactedMosaicConfig {
    let profiles = config
        .profiles
        .iter()
        .map(|(name, profile)| RedactedProfileView {
            name: name.clone(),
            provider_type: profile.provider_type.clone(),
            model: profile.model.clone(),
            base_url: profile.base_url.clone(),
            api_key_env: profile.api_key_env.clone(),
            api_key_present: profile
                .api_key_env
                .as_deref()
                .is_some_and(|env_var| env::var(env_var).is_ok()),
        })
        .collect();

    RedactedMosaicConfig {
        schema_version: config.schema_version,
        active_profile: config.active_profile.clone(),
        profiles,
        deployment: RedactedDeploymentView {
            profile: config.deployment.profile.clone(),
            workspace_name: config.deployment.workspace_name.clone(),
        },
        auth: RedactedAuthView {
            operator_token_env: config.auth.operator_token_env.clone(),
            operator_token_present: env_var_present(config.auth.operator_token_env.as_deref()),
            webchat_shared_secret_env: config.auth.webchat_shared_secret_env.clone(),
            webchat_shared_secret_present: env_var_present(
                config.auth.webchat_shared_secret_env.as_deref(),
            ),
            telegram_secret_token_env: config.auth.telegram_secret_token_env.clone(),
            telegram_secret_token_present: env_var_present(
                config.auth.telegram_secret_token_env.as_deref(),
            ),
        },
        session_store_root_dir: config.session_store.root_dir.clone(),
        inspect_runs_dir: config.inspect.runs_dir.clone(),
        audit: RedactedAuditView {
            root_dir: config.audit.root_dir.clone(),
            retention_days: config.audit.retention_days,
            event_replay_window: config.audit.event_replay_window,
            redact_inputs: config.audit.redact_inputs,
        },
        observability: RedactedObservabilityView {
            enable_metrics: config.observability.enable_metrics,
            enable_readiness: config.observability.enable_readiness,
            slow_consumer_lag_threshold: config.observability.slow_consumer_lag_threshold,
        },
        extension_manifest_count: config.extensions.manifests.len(),
        policies: RedactedPolicyView {
            allow_exec: config.policies.allow_exec,
            allow_webhook: config.policies.allow_webhook,
            allow_cron: config.policies.allow_cron,
            allow_mcp: config.policies.allow_mcp,
            hot_reload_enabled: config.policies.hot_reload_enabled,
        },
    }
}

fn load_config_patch(path: &Path) -> Result<MosaicConfigPatch> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read config layer {}", path.display()))?;
    let patch = serde_yaml::from_str::<MosaicConfigPatch>(&content)
        .with_context(|| format!("failed to parse config layer {}", path.display()))?;
    Ok(patch)
}

fn merge_patch(config: &mut MosaicConfig, patch: MosaicConfigPatch) {
    if let Some(schema_version) = patch.schema_version {
        config.schema_version = schema_version;
    }

    if let Some(active_profile) = patch.active_profile {
        config.active_profile = active_profile;
    }

    if !patch.profiles.is_empty() {
        config.profiles.extend(patch.profiles);
    }

    if let Some(deployment) = patch.deployment {
        config.deployment = deployment;
    }

    if let Some(auth) = patch.auth {
        config.auth = auth;
    }

    if let Some(session_store) = patch.session_store {
        config.session_store = session_store;
    }

    if let Some(inspect) = patch.inspect {
        config.inspect = inspect;
    }

    if let Some(audit) = patch.audit {
        config.audit = audit;
    }

    if let Some(observability) = patch.observability {
        config.observability = observability;
    }

    if let Some(extensions) = patch.extensions {
        config.extensions = extensions;
    }

    if let Some(policies) = patch.policies {
        config.policies = policies;
    }
}

fn env_var_present(name: Option<&str>) -> bool {
    name.map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| env::var(value).is_ok())
}

fn secret_env_check(
    category: DoctorCategory,
    env_name: Option<&str>,
    label: &str,
    required: bool,
) -> DoctorCheck {
    match env_name.map(str::trim).filter(|value| !value.is_empty()) {
        Some(name) => {
            if env::var(name).is_ok() {
                DoctorCheck {
                    status: DoctorStatus::Ok,
                    category,
                    message: format!("{label} is configured via {name}"),
                }
            } else {
                DoctorCheck {
                    status: if required {
                        DoctorStatus::Error
                    } else {
                        DoctorStatus::Warning
                    },
                    category,
                    message: format!("{label} expects environment variable {name} to be set"),
                }
            }
        }
        None => DoctorCheck {
            status: if required {
                DoctorStatus::Error
            } else {
                DoctorStatus::Warning
            },
            category,
            message: if required {
                format!("{label} is not configured")
            } else {
                format!("{label} is not configured; this surface is currently unauthenticated")
            },
        },
    }
}

fn path_check(
    path: &Path,
    category: DoctorCategory,
    label: &str,
    create_if_missing: bool,
) -> DoctorCheck {
    if path.exists() {
        if path.is_dir() {
            DoctorCheck {
                status: DoctorStatus::Ok,
                category,
                message: format!("{label} is ready at {}", path.display()),
            }
        } else {
            DoctorCheck {
                status: DoctorStatus::Error,
                category,
                message: format!(
                    "{label} path {} exists but is not a directory",
                    path.display()
                ),
            }
        }
    } else {
        DoctorCheck {
            status: DoctorStatus::Warning,
            category,
            message: if create_if_missing {
                format!(
                    "{label} does not exist yet at {} and will be created on demand",
                    path.display()
                )
            } else {
                format!("{label} does not exist at {}", path.display())
            },
        }
    }
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn default_active_profile() -> String {
    "mock".to_owned()
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
            },
        ),
        (
            "azure-gpt-5.4".to_owned(),
            ProviderProfileConfig {
                provider_type: "azure".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: Some("https://your-resource.openai.azure.com".to_owned()),
                api_key_env: Some("AZURE_OPENAI_API_KEY".to_owned()),
            },
        ),
        (
            "gpt-5.4".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
            },
        ),
        (
            "gpt-5.4-mini".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai".to_owned(),
                model: "gpt-5.4-mini".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
            },
        ),
        (
            "mock".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
            },
        ),
        (
            "ollama-qwen3".to_owned(),
            ProviderProfileConfig {
                provider_type: "ollama".to_owned(),
                model: "qwen3:14b".to_owned(),
                base_url: Some("http://127.0.0.1:11434".to_owned()),
                api_key_env: None,
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

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);

        let path = std::env::temp_dir().join(format!(
            "mosaic-config-{label}-{}-{nanos}-{count}",
            process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("config crate should live under crates/")
            .parent()
            .expect("repo root should exist")
            .to_path_buf()
    }

    #[test]
    fn provider_and_deployment_example_patches_validate_against_defaults() {
        let root = repo_root();
        for rel in [
            "examples/providers/openai.yaml",
            "examples/providers/azure.yaml",
            "examples/providers/ollama.yaml",
            "examples/providers/anthropic.yaml",
            "examples/deployment/production.config.yaml",
        ] {
            let patch = load_config_patch(&root.join(rel)).expect("example patch should load");
            let mut config = MosaicConfig::default();
            merge_patch(&mut config, patch);
            let report = validate_mosaic_config(&config);
            assert!(
                !report.has_errors(),
                "{rel} should validate without errors: {:?}",
                report.issues
            );
        }
    }

    #[test]
    fn workflow_and_gateway_examples_parse_from_disk() {
        let root = repo_root();
        let app = load_from_file(root.join("examples/workflows/research-brief.yaml"))
            .expect("workflow example should load");
        assert_eq!(
            app.app.and_then(|app| app.name).as_deref(),
            Some("research-brief")
        );

        let payload = fs::read_to_string(root.join("examples/gateway/webchat-message.json"))
            .expect("gateway payload should load");
        let parsed: serde_json::Value =
            serde_json::from_str(&payload).expect("gateway payload should parse as JSON");
        assert_eq!(parsed["session_id"], "docs-webchat");
    }

    #[test]
    fn loads_yaml_app_config_from_disk() {
        let dir = temp_dir("app-load");
        let path = dir.join("app.yaml");

        fs::write(
            &path,
            r#"
app:
  name: basic-demo
provider:
  type: openai-compatible
  model: mock
  api_key_env: OPENAI_API_KEY
tools:
  - type: builtin
    name: echo
skills:
  - type: builtin
    name: summarize
agent:
  system: You are helpful.
task:
  input: Explain Mosaic.
"#,
        )
        .expect("fixture should be written");

        let cfg = load_from_file(&path).expect("config should load");

        assert_eq!(cfg.provider.provider_type, "openai-compatible");
        assert_eq!(cfg.tools.len(), 1);
        assert_eq!(cfg.skills.len(), 1);
        assert_eq!(cfg.task.input, "Explain Mosaic.");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn loads_extension_manifest_and_policy_config() {
        let dir = temp_dir("extension-load");
        let workspace = dir.join("workspace.yaml");
        let manifest = dir.join("demo-extension.yaml");

        fs::write(
            &workspace,
            r#"
extensions:
  manifests:
    - path: demo-extension.yaml
      version_pin: 0.2.0
policies:
  allow_exec: false
  allow_webhook: true
  allow_cron: true
  allow_mcp: false
  hot_reload_enabled: true
"#,
        )
        .expect("workspace config should be written");

        fs::write(
            &manifest,
            r#"
name: demo.extension
version: 0.2.0
description: demo manifest
tools:
  - type: builtin
    name: time_now
skills:
  - type: builtin
    name: summarize
workflows: []
"#,
        )
        .expect("extension manifest should be written");

        let loaded = load_mosaic_config(&LoadConfigOptions {
            cwd: dir.clone(),
            user_config_path: None,
            workspace_config_path: Some(workspace.clone()),
            overrides: ConfigOverrides::default(),
        })
        .expect("workspace config should load");
        let manifest =
            load_extension_manifest_from_file(&manifest).expect("extension manifest should parse");

        assert_eq!(loaded.config.extensions.manifests.len(), 1);
        assert!(!loaded.config.policies.allow_exec);
        assert!(!loaded.config.policies.allow_mcp);
        assert_eq!(manifest.name, "demo.extension");
        assert_eq!(manifest.version, "0.2.0");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn layered_mosaic_config_prefers_workspace_over_user_and_cli_over_env() {
        let dir = temp_dir("layered");
        let user = dir.join("user.yaml");
        let workspace = dir.join("workspace.yaml");

        fs::write(
            &user,
            r#"
active_profile: gpt-5.4-mini
profiles:
  custom-user:
    type: mock
    model: mock-user
"#,
        )
        .expect("user config should be written");

        fs::write(
            &workspace,
            r#"
active_profile: gpt-5.4
profiles:
  custom-workspace:
    type: mock
    model: mock-workspace
"#,
        )
        .expect("workspace config should be written");

        // SAFETY: tests in this crate do not spawn threads that read process env.
        unsafe {
            env::set_var(ACTIVE_PROFILE_ENV, "mock");
        }

        let loaded = load_mosaic_config(&LoadConfigOptions {
            cwd: dir.clone(),
            user_config_path: Some(user.clone()),
            workspace_config_path: Some(workspace.clone()),
            overrides: ConfigOverrides {
                active_profile: Some("custom-workspace".to_owned()),
            },
        })
        .expect("layered config should load");

        assert_eq!(loaded.config.active_profile, "custom-workspace");
        assert!(loaded.config.profiles.contains_key("custom-user"));
        assert!(loaded.config.profiles.contains_key("custom-workspace"));
        assert_eq!(loaded.sources.len(), 5);

        // SAFETY: tests in this crate do not spawn threads that read process env.
        unsafe {
            env::remove_var(ACTIVE_PROFILE_ENV);
        }

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn validate_reports_invalid_active_profile_and_missing_api_key_env() {
        let mut config = MosaicConfig::default();
        config.active_profile = "missing".to_owned();
        config.profiles.insert(
            "broken".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai-compatible".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: None,
            },
        );

        let report = validate_mosaic_config(&config);

        assert!(report.has_errors());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.field == "active_profile")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.field == "profiles.broken.api_key_env")
        );
    }

    #[test]
    fn validate_reports_missing_azure_base_url() {
        let mut config = MosaicConfig::default();
        config.profiles.insert(
            "azure-broken".to_owned(),
            ProviderProfileConfig {
                provider_type: "azure".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: None,
                api_key_env: Some("AZURE_OPENAI_API_KEY".to_owned()),
            },
        );

        let report = validate_mosaic_config(&config);

        assert!(report.has_errors());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.field == "profiles.azure-broken.base_url")
        );
    }

    #[test]
    fn doctor_redacts_profiles_and_reports_missing_active_api_key() {
        let mut config = MosaicConfig::default();
        config.active_profile = "gpt-5.4".to_owned();
        let dir = temp_dir("doctor");

        let doctor = doctor_mosaic_config(&config, &dir);
        let redacted = redact_mosaic_config(&config);

        assert!(doctor.has_errors());
        assert!(doctor.checks.iter().any(|check| {
            check.message.contains("OPENAI_API_KEY") && matches!(check.status, DoctorStatus::Error)
        }));
        assert_eq!(redacted.active_profile, "gpt-5.4");
        assert!(
            redacted
                .profiles
                .iter()
                .any(|profile| profile.name == "gpt-5.4" && !profile.api_key_present)
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn init_workspace_config_writes_template_and_directories() {
        let dir = temp_dir("init");
        let path = init_workspace_config(&dir, false).expect("workspace init should succeed");
        let content = fs::read_to_string(&path).expect("config should be readable");

        assert!(content.contains("schema_version: 1"));
        assert!(dir.join(".mosaic/sessions").is_dir());
        assert!(dir.join(".mosaic/runs").is_dir());
        assert!(dir.join(".mosaic/extensions").is_dir());

        fs::remove_dir_all(dir).ok();
    }
}
