use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const ACTIVE_PROFILE_ENV: &str = "MOSAIC_ACTIVE_PROFILE";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub app: Option<AppSection>,
    pub provider: ProviderConfig,
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub skills: Vec<SkillConfig>,
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
    pub session_store: SessionStoreConfig,
    #[serde(default)]
    pub inspect: InspectConfig,
}

impl Default for MosaicConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            active_profile: default_active_profile(),
            profiles: default_profiles(),
            session_store: SessionStoreConfig::default(),
            inspect: InspectConfig::default(),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorCheck {
    pub status: DoctorStatus,
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
pub struct RedactedMosaicConfig {
    pub schema_version: u32,
    pub active_profile: String,
    pub profiles: Vec<RedactedProfileView>,
    pub session_store_root_dir: String,
    pub inspect_runs_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
struct MosaicConfigPatch {
    pub schema_version: Option<u32>,
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProviderProfileConfig>,
    pub session_store: Option<SessionStoreConfig>,
    pub inspect: Option<InspectConfig>,
}

pub fn load_from_file(path: impl AsRef<Path>) -> Result<AppConfig> {
    let content = fs::read_to_string(path)?;
    let cfg = serde_yaml::from_str::<AppConfig>(&content)?;
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
        }

        if !matches!(profile.provider_type.as_str(), "mock" | "openai-compatible") {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.type"),
                format!("unsupported provider type '{}': expected mock or openai-compatible", profile.provider_type),
            );
        }

        if profile.model.trim().is_empty() {
            report.push(
                ValidationLevel::Error,
                format!("{field_prefix}.model"),
                "model must not be empty",
            );
        }

        if profile.provider_type == "openai-compatible" {
            match profile.api_key_env.as_deref().map(str::trim) {
                Some("") | None => report.push(
                    ValidationLevel::Error,
                    format!("{field_prefix}.api_key_env"),
                    "openai-compatible profiles require api_key_env",
                ),
                _ => {}
            }
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

    report
}

pub fn doctor_mosaic_config(config: &MosaicConfig, cwd: impl AsRef<Path>) -> DoctorReport {
    let cwd = cwd.as_ref();
    let validation = validate_mosaic_config(config);
    let mut checks = Vec::new();

    let session_root = cwd.join(&config.session_store.root_dir);
    checks.push(path_check(
        &session_root,
        "session store directory",
        true,
    ));

    let runs_root = cwd.join(&config.inspect.runs_dir);
    checks.push(path_check(&runs_root, "run trace directory", true));

    if let Some(active_profile) = config.profiles.get(&config.active_profile) {
        if active_profile.provider_type == "mock" {
            checks.push(DoctorCheck {
                status: DoctorStatus::Ok,
                message: format!(
                    "active profile '{}' uses the mock provider and does not require API credentials",
                    config.active_profile
                ),
            });
        } else if let Some(api_key_env) = active_profile.api_key_env.as_deref() {
            let status = if env::var(api_key_env).is_ok() {
                DoctorStatus::Ok
            } else {
                DoctorStatus::Error
            };
            let is_ok = matches!(status, DoctorStatus::Ok);

            checks.push(DoctorCheck {
                status,
                message: if is_ok {
                    format!(
                        "active profile '{}' has {} available in the environment",
                        config.active_profile, api_key_env
                    )
                } else {
                    format!(
                        "active profile '{}' expects environment variable {} to be set",
                        config.active_profile, api_key_env
                    )
                },
            });
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
        session_store_root_dir: config.session_store.root_dir.clone(),
        inspect_runs_dir: config.inspect.runs_dir.clone(),
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

    if let Some(session_store) = patch.session_store {
        config.session_store = session_store;
    }

    if let Some(inspect) = patch.inspect {
        config.inspect = inspect;
    }
}

fn path_check(path: &Path, label: &str, create_if_missing: bool) -> DoctorCheck {
    if path.exists() {
        if path.is_dir() {
            DoctorCheck {
                status: DoctorStatus::Ok,
                message: format!("{label} is ready at {}", path.display()),
            }
        } else {
            DoctorCheck {
                status: DoctorStatus::Error,
                message: format!("{label} path {} exists but is not a directory", path.display()),
            }
        }
    } else {
        DoctorCheck {
            status: DoctorStatus::Warning,
            message: if create_if_missing {
                format!("{label} does not exist yet at {} and will be created on demand", path.display())
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

fn default_profiles() -> BTreeMap<String, ProviderProfileConfig> {
    BTreeMap::from([
        (
            "gpt-5.4".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai-compatible".to_owned(),
                model: "gpt-5.4".to_owned(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                api_key_env: Some("OPENAI_API_KEY".to_owned()),
            },
        ),
        (
            "gpt-5.4-mini".to_owned(),
            ProviderProfileConfig {
                provider_type: "openai-compatible".to_owned(),
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
    ])
}

fn default_session_store_root_dir() -> String {
    ".mosaic/sessions".to_owned()
}

fn default_runs_root_dir() -> String {
    ".mosaic/runs".to_owned()
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
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.field == "active_profile"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.field == "profiles.broken.api_key_env"));
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
            check.message.contains("OPENAI_API_KEY")
                && matches!(check.status, DoctorStatus::Error)
        }));
        assert_eq!(redacted.active_profile, "gpt-5.4");
        assert!(redacted
            .profiles
            .iter()
            .any(|profile| profile.name == "gpt-5.4" && !profile.api_key_present));

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

        fs::remove_dir_all(dir).ok();
    }
}
