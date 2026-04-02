use std::{
    collections::BTreeMap,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const ENV_RECORD_FILE: &str = ".mosaic-sandbox-env.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxKind {
    #[default]
    Python,
    Node,
    Shell,
    Processor,
}

impl SandboxKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Node => "node",
            Self::Shell => "shell",
            Self::Processor => "processor",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxScope {
    Workspace,
    #[default]
    Capability,
    Run,
}

impl SandboxScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Capability => "capability",
            Self::Run => "run",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonEnvStrategy {
    #[default]
    Venv,
    Uv,
    Disabled,
}

impl PythonEnvStrategy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Venv => "venv",
            Self::Uv => "uv",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeEnvStrategy {
    #[default]
    Npm,
    Pnpm,
    LayoutOnly,
    Disabled,
}

impl NodeEnvStrategy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::LayoutOnly => "layout_only",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxCleanupPolicy {
    pub run_workdirs_after_hours: u64,
    pub attachments_after_hours: u64,
}

impl Default for SandboxCleanupPolicy {
    fn default() -> Self {
        Self {
            run_workdirs_after_hours: 24,
            attachments_after_hours: 24,
        }
    }
}

fn default_install_enabled() -> bool {
    true
}

fn default_install_timeout_ms() -> u64 {
    120_000
}

fn default_install_sources() -> Vec<SandboxInstallSource> {
    vec![SandboxInstallSource::Registry, SandboxInstallSource::File]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxInstallSource {
    Registry,
    File,
}

impl SandboxInstallSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::File => "file",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxInstallPolicy {
    #[serde(default = "default_install_enabled")]
    pub enabled: bool,
    #[serde(default = "default_install_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub retry_limit: u8,
    #[serde(default = "default_install_sources")]
    pub allowed_sources: Vec<SandboxInstallSource>,
}

impl Default for SandboxInstallPolicy {
    fn default() -> Self {
        Self {
            enabled: default_install_enabled(),
            timeout_ms: default_install_timeout_ms(),
            retry_limit: 0,
            allowed_sources: default_install_sources(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonSandboxSettings {
    #[serde(default)]
    pub strategy: PythonEnvStrategy,
    #[serde(default)]
    pub install: SandboxInstallPolicy,
}

impl Default for PythonSandboxSettings {
    fn default() -> Self {
        Self {
            strategy: PythonEnvStrategy::Venv,
            install: SandboxInstallPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSandboxSettings {
    #[serde(default)]
    pub strategy: NodeEnvStrategy,
    #[serde(default)]
    pub install: SandboxInstallPolicy,
}

impl Default for NodeSandboxSettings {
    fn default() -> Self {
        Self {
            strategy: NodeEnvStrategy::Npm,
            install: SandboxInstallPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxSettings {
    pub base_dir: PathBuf,
    #[serde(default)]
    pub python: PythonSandboxSettings,
    #[serde(default)]
    pub node: NodeSandboxSettings,
    pub cleanup: SandboxCleanupPolicy,
}

impl Default for SandboxSettings {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(".mosaic/sandbox"),
            python: PythonSandboxSettings::default(),
            node: NodeSandboxSettings::default(),
            cleanup: SandboxCleanupPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SandboxBinding {
    #[serde(default)]
    pub kind: SandboxKind,
    pub env_name: String,
    #[serde(default)]
    pub dependency_spec: Vec<String>,
    #[serde(default)]
    pub scope: SandboxScope,
}

impl SandboxBinding {
    pub fn new(
        kind: SandboxKind,
        env_name: impl Into<String>,
        scope: SandboxScope,
        dependency_spec: Vec<String>,
    ) -> Self {
        Self {
            kind,
            env_name: env_name.into(),
            dependency_spec,
            scope,
        }
    }

    pub fn env_id(&self) -> String {
        format!(
            "{}-{}-{}",
            self.kind.label(),
            self.scope.label(),
            slugify(&self.env_name)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPaths {
    pub root: PathBuf,
    pub python_envs: PathBuf,
    pub python_cache: PathBuf,
    pub node_envs: PathBuf,
    pub node_cache: PathBuf,
    pub shell_envs: PathBuf,
    pub work_runs: PathBuf,
    pub attachments: PathBuf,
    pub processors: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxEnvStatus {
    Absent,
    Preparing,
    #[serde(alias = "layout_only")]
    Ready,
    Drifted,
    #[serde(alias = "missing_runtime", alias = "broken")]
    Failed,
    RebuildRequired,
}

impl SandboxEnvStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Drifted => "drifted",
            Self::Failed => "failed",
            Self::RebuildRequired => "rebuild_required",
        }
    }

    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxFailureStage {
    Runtime,
    Create,
    Install,
    HealthCheck,
    Policy,
}

impl SandboxFailureStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Create => "create",
            Self::Install => "install",
            Self::HealthCheck => "health_check",
            Self::Policy => "policy",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxEnvRecord {
    pub env_id: String,
    pub kind: SandboxKind,
    pub scope: SandboxScope,
    pub env_name: String,
    #[serde(default)]
    pub dependency_spec: Vec<String>,
    #[serde(default)]
    pub dependency_fingerprint: String,
    pub strategy: String,
    pub env_dir: PathBuf,
    pub cache_dir: PathBuf,
    #[serde(default)]
    pub runtime_dir: Option<PathBuf>,
    pub status: SandboxEnvStatus,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub failure_stage: Option<String>,
    #[serde(default = "default_install_enabled")]
    pub install_enabled: bool,
    #[serde(default = "default_install_timeout_ms")]
    pub install_timeout_ms: u64,
    #[serde(default)]
    pub install_retry_limit: u8,
    #[serde(default)]
    pub allowed_sources: Vec<SandboxInstallSource>,
    #[serde(default)]
    pub last_transition: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxEnvResolution {
    pub record: SandboxEnvRecord,
    pub prepared: bool,
    pub reused: bool,
    pub selection_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxRuntimeStatus {
    pub kind: SandboxKind,
    pub strategy: String,
    pub available: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SandboxCleanReport {
    pub removed_run_workdirs: usize,
    pub removed_attachment_workdirs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxRunAllocation {
    pub run_id: String,
    pub root: PathBuf,
    pub workdir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SandboxManager {
    workspace_root: PathBuf,
    settings: SandboxSettings,
}

impl SandboxManager {
    pub fn new(workspace_root: impl AsRef<Path>, settings: SandboxSettings) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            settings,
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn settings(&self) -> &SandboxSettings {
        &self.settings
    }

    pub fn paths(&self) -> SandboxPaths {
        let root = if self.settings.base_dir.is_absolute() {
            self.settings.base_dir.clone()
        } else {
            self.workspace_root.join(&self.settings.base_dir)
        };

        SandboxPaths {
            python_envs: root.join("python/envs"),
            python_cache: root.join("python/cache"),
            node_envs: root.join("node/envs"),
            node_cache: root.join("node/cache"),
            shell_envs: root.join("shell/envs"),
            work_runs: root.join("work/runs"),
            attachments: root.join("attachments"),
            processors: root.join("processors"),
            root,
        }
    }

    pub fn ensure_layout(&self) -> Result<SandboxPaths> {
        let paths = self.paths();
        for dir in [
            &paths.root,
            &paths.python_envs,
            &paths.python_cache,
            &paths.node_envs,
            &paths.node_cache,
            &paths.shell_envs,
            &paths.work_runs,
            &paths.attachments,
            &paths.processors,
        ] {
            fs::create_dir_all(dir)
                .with_context(|| format!("failed to create sandbox directory {}", dir.display()))?;
        }
        Ok(paths)
    }

    pub fn runtime_statuses(&self) -> Vec<SandboxRuntimeStatus> {
        vec![
            self.python_runtime_status(),
            self.node_runtime_status(),
            self.shell_runtime_status(),
        ]
    }

    pub fn create_run_workdir(&self, run_id: &str) -> Result<SandboxRunAllocation> {
        let paths = self.ensure_layout()?;
        let workdir = paths.work_runs.join(slugify(run_id));
        fs::create_dir_all(&workdir)
            .with_context(|| format!("failed to create run workdir {}", workdir.display()))?;
        Ok(SandboxRunAllocation {
            run_id: run_id.to_owned(),
            root: paths.root,
            workdir,
        })
    }

    pub fn ensure_env(&self, binding: &SandboxBinding) -> Result<SandboxEnvResolution> {
        self.ensure_layout()?;
        let env_dir = self.env_dir(binding);
        let cache_dir = self.cache_dir(binding.kind);
        fs::create_dir_all(&env_dir)
            .with_context(|| format!("failed to create sandbox env {}", env_dir.display()))?;
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("failed to create sandbox cache {}", cache_dir.display()))?;

        let now = Utc::now();
        let record_path = env_dir.join(ENV_RECORD_FILE);
        let existing = self.read_env_record_if_exists(&record_path)?;
        let created_at = existing
            .as_ref()
            .map(|record| record.created_at)
            .unwrap_or(now);
        let dependency_fingerprint = dependency_fingerprint(&binding.dependency_spec);
        let strategy = self.strategy_label(binding.kind).to_owned();
        let install_policy = self.install_policy(binding.kind).clone();

        if let Some(existing) = existing {
            if existing.dependency_fingerprint != dependency_fingerprint {
                let record = SandboxEnvRecord {
                    env_id: binding.env_id(),
                    kind: binding.kind,
                    scope: binding.scope,
                    env_name: binding.env_name.clone(),
                    dependency_spec: binding.dependency_spec.clone(),
                    dependency_fingerprint,
                    strategy,
                    env_dir,
                    cache_dir,
                    runtime_dir: existing.runtime_dir,
                    status: SandboxEnvStatus::RebuildRequired,
                    error: Some("dependency spec changed; rebuild required".to_owned()),
                    failure_stage: Some(SandboxFailureStage::Policy.label().to_owned()),
                    install_enabled: install_policy.enabled,
                    install_timeout_ms: install_policy.timeout_ms,
                    install_retry_limit: install_policy.retry_limit,
                    allowed_sources: install_policy.allowed_sources.clone(),
                    last_transition: "rebuild_required".to_owned(),
                    created_at,
                    updated_at: now,
                };
                self.write_env_record(&record)?;
                return Ok(SandboxEnvResolution {
                    record,
                    prepared: false,
                    reused: false,
                    selection_reason: "dependency fingerprint changed".to_owned(),
                });
            }

            if let Some(drift_reason) = self.detect_env_drift(binding.kind, &existing) {
                let record = SandboxEnvRecord {
                    env_id: binding.env_id(),
                    kind: binding.kind,
                    scope: binding.scope,
                    env_name: binding.env_name.clone(),
                    dependency_spec: binding.dependency_spec.clone(),
                    dependency_fingerprint,
                    strategy,
                    env_dir,
                    cache_dir,
                    runtime_dir: existing.runtime_dir,
                    status: SandboxEnvStatus::Drifted,
                    error: Some(drift_reason.clone()),
                    failure_stage: Some(SandboxFailureStage::HealthCheck.label().to_owned()),
                    install_enabled: install_policy.enabled,
                    install_timeout_ms: install_policy.timeout_ms,
                    install_retry_limit: install_policy.retry_limit,
                    allowed_sources: install_policy.allowed_sources.clone(),
                    last_transition: "drifted".to_owned(),
                    created_at,
                    updated_at: now,
                };
                self.write_env_record(&record)?;
                return Ok(SandboxEnvResolution {
                    record,
                    prepared: false,
                    reused: false,
                    selection_reason: "existing env drifted".to_owned(),
                });
            }

            if existing.status == SandboxEnvStatus::Ready {
                let record = SandboxEnvRecord {
                    env_id: binding.env_id(),
                    kind: binding.kind,
                    scope: binding.scope,
                    env_name: binding.env_name.clone(),
                    dependency_spec: binding.dependency_spec.clone(),
                    dependency_fingerprint,
                    strategy,
                    env_dir,
                    cache_dir,
                    runtime_dir: existing.runtime_dir,
                    status: SandboxEnvStatus::Ready,
                    error: None,
                    failure_stage: None,
                    install_enabled: install_policy.enabled,
                    install_timeout_ms: install_policy.timeout_ms,
                    install_retry_limit: install_policy.retry_limit,
                    allowed_sources: install_policy.allowed_sources.clone(),
                    last_transition: "reused".to_owned(),
                    created_at,
                    updated_at: now,
                };
                self.write_env_record(&record)?;
                return Ok(SandboxEnvResolution {
                    record,
                    prepared: false,
                    reused: true,
                    selection_reason: "reused existing ready env".to_owned(),
                });
            }
        }

        let preparing = SandboxEnvRecord {
            env_id: binding.env_id(),
            kind: binding.kind,
            scope: binding.scope,
            env_name: binding.env_name.clone(),
            dependency_spec: binding.dependency_spec.clone(),
            dependency_fingerprint: dependency_fingerprint.clone(),
            strategy: strategy.clone(),
            env_dir: env_dir.clone(),
            cache_dir: cache_dir.clone(),
            runtime_dir: None,
            status: SandboxEnvStatus::Preparing,
            error: None,
            failure_stage: None,
            install_enabled: install_policy.enabled,
            install_timeout_ms: install_policy.timeout_ms,
            install_retry_limit: install_policy.retry_limit,
            allowed_sources: install_policy.allowed_sources.clone(),
            last_transition: "preparing".to_owned(),
            created_at,
            updated_at: now,
        };
        self.write_env_record(&preparing)?;

        let prepared = match binding.kind {
            SandboxKind::Python => self.prepare_python_env(&env_dir, &cache_dir, binding)?,
            SandboxKind::Node => self.prepare_node_env(&env_dir, &cache_dir, binding)?,
            SandboxKind::Shell => self.prepare_shell_env(&env_dir, binding),
            SandboxKind::Processor => self.prepare_processor_env(&env_dir, binding),
        };

        let record = SandboxEnvRecord {
            env_id: binding.env_id(),
            kind: binding.kind,
            scope: binding.scope,
            env_name: binding.env_name.clone(),
            dependency_spec: binding.dependency_spec.clone(),
            dependency_fingerprint,
            strategy: prepared.strategy,
            env_dir,
            cache_dir,
            runtime_dir: prepared.runtime_dir,
            status: prepared.status,
            error: prepared.error,
            failure_stage: prepared.failure_stage.map(|stage| stage.label().to_owned()),
            install_enabled: install_policy.enabled,
            install_timeout_ms: install_policy.timeout_ms,
            install_retry_limit: install_policy.retry_limit,
            allowed_sources: install_policy.allowed_sources.clone(),
            last_transition: if prepared.status.is_ready() {
                "prepared".to_owned()
            } else {
                "failed".to_owned()
            },
            created_at,
            updated_at: Utc::now(),
        };
        self.write_env_record(&record)?;
        Ok(SandboxEnvResolution {
            record,
            prepared: true,
            reused: false,
            selection_reason: "prepared sandbox env for capability execution".to_owned(),
        })
    }

    pub fn list_envs(&self) -> Result<Vec<SandboxEnvRecord>> {
        self.ensure_layout()?;
        let mut records = Vec::new();
        let paths = self.paths();
        for root in [
            paths.python_envs,
            paths.node_envs,
            paths.shell_envs,
            paths.processors,
        ] {
            if !root.exists() {
                continue;
            }
            for entry in fs::read_dir(&root)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let record_path = path.join(ENV_RECORD_FILE);
                if let Some(record) = self.read_env_record_if_exists(&record_path)? {
                    records.push(record);
                }
            }
        }
        records.sort_by(|left, right| left.env_id.cmp(&right.env_id));
        Ok(records)
    }

    pub fn inspect_env(&self, env_id: &str) -> Result<SandboxEnvRecord> {
        self.list_envs()?
            .into_iter()
            .find(|record| record.env_id == env_id)
            .ok_or_else(|| anyhow!("sandbox env not found: {}", env_id))
    }

    pub fn rebuild_env(&self, env_id: &str) -> Result<SandboxEnvRecord> {
        let record = self.inspect_env(env_id)?;
        if record.env_dir.exists() {
            fs::remove_dir_all(&record.env_dir).with_context(|| {
                format!(
                    "failed to remove sandbox env before rebuild {}",
                    record.env_dir.display()
                )
            })?;
        }
        self.ensure_env(&SandboxBinding {
            kind: record.kind,
            env_name: record.env_name,
            dependency_spec: record.dependency_spec,
            scope: record.scope,
        })
        .map(|resolution| resolution.record)
    }

    pub fn clean(&self) -> Result<SandboxCleanReport> {
        let paths = self.ensure_layout()?;
        let removed_run_workdirs = remove_children(&paths.work_runs)?;
        let removed_attachment_workdirs = remove_children(&paths.attachments)?;
        Ok(SandboxCleanReport {
            removed_run_workdirs,
            removed_attachment_workdirs,
        })
    }

    fn env_dir(&self, binding: &SandboxBinding) -> PathBuf {
        match binding.kind {
            SandboxKind::Python => self.paths().python_envs.join(slugify(&binding.env_name)),
            SandboxKind::Node => self.paths().node_envs.join(slugify(&binding.env_name)),
            SandboxKind::Shell => self.paths().shell_envs.join(slugify(&binding.env_name)),
            SandboxKind::Processor => self.paths().processors.join(slugify(&binding.env_name)),
        }
    }

    fn cache_dir(&self, kind: SandboxKind) -> PathBuf {
        let paths = self.paths();
        match kind {
            SandboxKind::Python => paths.python_cache,
            SandboxKind::Node => paths.node_cache,
            SandboxKind::Shell => paths.work_runs.join("shell-cache"),
            SandboxKind::Processor => paths.processors.join("cache"),
        }
    }

    fn read_env_record_if_exists(&self, record_path: &Path) -> Result<Option<SandboxEnvRecord>> {
        if !record_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(record_path)
            .with_context(|| format!("failed to read sandbox record {}", record_path.display()))?;
        let record = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse sandbox record {}", record_path.display()))?;
        Ok(Some(record))
    }

    fn write_env_record(&self, record: &SandboxEnvRecord) -> Result<()> {
        let body = serde_json::to_string_pretty(record)?;
        fs::write(record.env_dir.join(ENV_RECORD_FILE), body).with_context(|| {
            format!(
                "failed to write sandbox record {}",
                record.env_dir.join(ENV_RECORD_FILE).display()
            )
        })?;
        Ok(())
    }

    fn python_runtime_status(&self) -> SandboxRuntimeStatus {
        match self.settings.python.strategy {
            PythonEnvStrategy::Disabled => SandboxRuntimeStatus {
                kind: SandboxKind::Python,
                strategy: PythonEnvStrategy::Disabled.label().to_owned(),
                available: false,
                detail: Some("python sandboxing disabled".to_owned()),
            },
            PythonEnvStrategy::Venv => command_status("python3", SandboxKind::Python, "venv"),
            PythonEnvStrategy::Uv => command_status("uv", SandboxKind::Python, "uv"),
        }
    }

    fn node_runtime_status(&self) -> SandboxRuntimeStatus {
        match self.settings.node.strategy {
            NodeEnvStrategy::Disabled => SandboxRuntimeStatus {
                kind: SandboxKind::Node,
                strategy: NodeEnvStrategy::Disabled.label().to_owned(),
                available: false,
                detail: Some("node sandboxing disabled".to_owned()),
            },
            NodeEnvStrategy::LayoutOnly => SandboxRuntimeStatus {
                kind: SandboxKind::Node,
                strategy: NodeEnvStrategy::LayoutOnly.label().to_owned(),
                available: true,
                detail: Some("layout-only node sandbox".to_owned()),
            },
            NodeEnvStrategy::Npm => command_status("npm", SandboxKind::Node, "npm"),
            NodeEnvStrategy::Pnpm => command_status("pnpm", SandboxKind::Node, "pnpm"),
        }
    }

    fn shell_runtime_status(&self) -> SandboxRuntimeStatus {
        command_status("sh", SandboxKind::Shell, "sh")
    }

    fn prepare_python_env(
        &self,
        env_dir: &Path,
        cache_dir: &Path,
        binding: &SandboxBinding,
    ) -> Result<PreparedEnv> {
        let requirements = env_dir.join("requirements.txt");
        if !binding.dependency_spec.is_empty() {
            fs::write(&requirements, binding.dependency_spec.join("\n") + "\n")
                .with_context(|| format!("failed to write {}", requirements.display()))?;
        }
        match self.settings.python.strategy {
            PythonEnvStrategy::Disabled => Ok(PreparedEnv::failed(
                PythonEnvStrategy::Disabled.label(),
                None,
                SandboxFailureStage::Runtime,
                "python sandbox strategy is disabled",
            )),
            PythonEnvStrategy::Uv => {
                let runtime_dir = env_dir.join("uv");
                if !command_exists("uv") {
                    return Ok(PreparedEnv::failed(
                        PythonEnvStrategy::Uv.label(),
                        Some(runtime_dir),
                        SandboxFailureStage::Runtime,
                        "uv is not available on PATH",
                    ));
                }
                if let Err(error) = run_command_with_timeout(
                    &CommandSpecBuilder::new("uv")
                        .args(["venv"])
                        .arg(&runtime_dir),
                    self.settings.python.install.timeout_ms,
                ) {
                    return Ok(PreparedEnv::failed(
                        PythonEnvStrategy::Uv.label(),
                        Some(runtime_dir),
                        SandboxFailureStage::Create,
                        error.to_string(),
                    ));
                }
                if !binding.dependency_spec.is_empty() {
                    if let Err(error) = self.validate_install_sources(
                        binding.kind,
                        &binding.dependency_spec,
                        &self.settings.python.install,
                    ) {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Uv.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Policy,
                            error.to_string(),
                        ));
                    }
                    if !self.settings.python.install.enabled {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Uv.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Policy,
                            "python dependency installs are disabled by sandbox policy",
                        ));
                    }
                    let python = python_binary(&runtime_dir);
                    if let Err(error) = run_command_with_retries(
                        CommandSpecBuilder::new("uv")
                            .args(["pip", "install", "--python"])
                            .arg(&python)
                            .args(["-r"])
                            .arg(&requirements)
                            .env("UV_CACHE_DIR", cache_dir),
                        &self.settings.python.install,
                    ) {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Uv.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Install,
                            error.to_string(),
                        ));
                    }
                }
                Ok(PreparedEnv::ready(
                    PythonEnvStrategy::Uv.label(),
                    Some(runtime_dir),
                ))
            }
            PythonEnvStrategy::Venv => {
                let runtime_dir = env_dir.join("venv");
                if !command_exists("python3") {
                    return Ok(PreparedEnv::failed(
                        PythonEnvStrategy::Venv.label(),
                        Some(runtime_dir),
                        SandboxFailureStage::Runtime,
                        "python3 is not available on PATH",
                    ));
                }
                if !runtime_dir.exists() {
                    if let Err(error) = run_command_with_timeout(
                        &CommandSpecBuilder::new("python3")
                            .args(["-m", "venv"])
                            .arg(&runtime_dir),
                        self.settings.python.install.timeout_ms,
                    ) {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Venv.label(),
                            Some(runtime_dir),
                            SandboxFailureStage::Create,
                            error.to_string(),
                        ));
                    }
                }
                if !binding.dependency_spec.is_empty() {
                    if let Err(error) = self.validate_install_sources(
                        binding.kind,
                        &binding.dependency_spec,
                        &self.settings.python.install,
                    ) {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Venv.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Policy,
                            error.to_string(),
                        ));
                    }
                    if !self.settings.python.install.enabled {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Venv.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Policy,
                            "python dependency installs are disabled by sandbox policy",
                        ));
                    }
                    let pip = pip_binary(&runtime_dir);
                    if let Err(error) = run_command_with_retries(
                        CommandSpecBuilder::new(pip.display().to_string())
                            .args(["install", "-r"])
                            .arg(&requirements)
                            .env("PIP_CACHE_DIR", cache_dir),
                        &self.settings.python.install,
                    ) {
                        return Ok(PreparedEnv::failed(
                            PythonEnvStrategy::Venv.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Install,
                            error.to_string(),
                        ));
                    }
                }
                Ok(PreparedEnv::ready(
                    PythonEnvStrategy::Venv.label(),
                    Some(runtime_dir),
                ))
            }
        }
    }

    fn prepare_node_env(
        &self,
        env_dir: &Path,
        cache_dir: &Path,
        binding: &SandboxBinding,
    ) -> Result<PreparedEnv> {
        let package_json = env_dir.join("package.json");
        let mut dependencies = BTreeMap::new();
        for dependency in &binding.dependency_spec {
            let trimmed = dependency.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (name, version) = parse_node_dependency(trimmed);
            dependencies.insert(name, version);
        }
        let package_body = serde_json::json!({
            "name": format!("mosaic-sandbox-{}", slugify(&binding.env_name)),
            "private": true,
            "version": "0.0.0",
            "dependencies": dependencies,
        });
        fs::write(&package_json, serde_json::to_vec_pretty(&package_body)?)
            .with_context(|| format!("failed to write {}", package_json.display()))?;

        let strategy = self.settings.node.strategy;
        let runtime_dir = env_dir.join("node_modules");
        match strategy {
            NodeEnvStrategy::Disabled => Ok(PreparedEnv::failed(
                strategy.label(),
                Some(runtime_dir),
                SandboxFailureStage::Runtime,
                "node sandbox strategy is disabled",
            )),
            NodeEnvStrategy::LayoutOnly => {
                if binding.dependency_spec.is_empty() {
                    Ok(PreparedEnv::ready(strategy.label(), Some(runtime_dir)))
                } else {
                    Ok(PreparedEnv {
                        status: SandboxEnvStatus::RebuildRequired,
                        runtime_dir: Some(runtime_dir),
                        error: Some(
                            "node layout_only strategy does not install dependencies; switch to npm/pnpm or rebuild with a richer strategy"
                                .to_owned(),
                        ),
                        failure_stage: Some(SandboxFailureStage::Policy),
                        strategy: strategy.label().to_owned(),
                    })
                }
            }
            NodeEnvStrategy::Npm | NodeEnvStrategy::Pnpm => {
                let command = if strategy == NodeEnvStrategy::Npm {
                    "npm"
                } else {
                    "pnpm"
                };
                if !command_exists(command) {
                    return Ok(PreparedEnv::failed(
                        strategy.label(),
                        Some(runtime_dir),
                        SandboxFailureStage::Runtime,
                        format!("{command} is not available on PATH"),
                    ));
                }
                if !binding.dependency_spec.is_empty() {
                    if let Err(error) = self.validate_install_sources(
                        binding.kind,
                        &binding.dependency_spec,
                        &self.settings.node.install,
                    ) {
                        return Ok(PreparedEnv::failed(
                            strategy.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Policy,
                            error.to_string(),
                        ));
                    }
                    if !self.settings.node.install.enabled {
                        return Ok(PreparedEnv::failed(
                            strategy.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Policy,
                            "node dependency installs are disabled by sandbox policy",
                        ));
                    }
                    let builder = if strategy == NodeEnvStrategy::Npm {
                        CommandSpecBuilder::new("npm")
                            .arg("install")
                            .arg("--no-package-lock")
                            .cwd(env_dir)
                            .env("npm_config_cache", cache_dir)
                    } else {
                        CommandSpecBuilder::new("pnpm")
                            .arg("install")
                            .cwd(env_dir)
                            .env("npm_config_cache", cache_dir)
                    };
                    if let Err(error) =
                        run_command_with_retries(builder, &self.settings.node.install)
                    {
                        return Ok(PreparedEnv::failed(
                            strategy.label(),
                            Some(runtime_dir.clone()),
                            SandboxFailureStage::Install,
                            error.to_string(),
                        ));
                    }
                }

                Ok(PreparedEnv::ready(strategy.label(), Some(runtime_dir)))
            }
        }
    }

    fn prepare_shell_env(&self, env_dir: &Path, binding: &SandboxBinding) -> PreparedEnv {
        let script = env_dir.join("ENVIRONMENT.txt");
        let _ = fs::write(
            &script,
            format!(
                "shell sandbox for {}\ndependencies: {}\n",
                binding.env_name,
                binding.dependency_spec.join(", ")
            ),
        );
        let available = command_exists("sh");
        if available {
            PreparedEnv::ready("sh", Some(env_dir.to_path_buf()))
        } else {
            PreparedEnv::failed(
                "sh",
                Some(env_dir.to_path_buf()),
                SandboxFailureStage::Runtime,
                "sh is not available on PATH",
            )
        }
    }

    fn prepare_processor_env(&self, env_dir: &Path, binding: &SandboxBinding) -> PreparedEnv {
        let manifest = env_dir.join("processor.json");
        let _ = fs::write(
            &manifest,
            serde_json::to_vec_pretty(&serde_json::json!({
                "name": binding.env_name,
                "dependencies": binding.dependency_spec,
            }))
            .unwrap_or_default(),
        );
        PreparedEnv::ready("processor", Some(env_dir.to_path_buf()))
    }

    fn strategy_label(&self, kind: SandboxKind) -> &'static str {
        match kind {
            SandboxKind::Python => self.settings.python.strategy.label(),
            SandboxKind::Node => self.settings.node.strategy.label(),
            SandboxKind::Shell => "sh",
            SandboxKind::Processor => "processor",
        }
    }

    fn install_policy(&self, kind: SandboxKind) -> &SandboxInstallPolicy {
        match kind {
            SandboxKind::Python => &self.settings.python.install,
            SandboxKind::Node => &self.settings.node.install,
            SandboxKind::Shell | SandboxKind::Processor => {
                static DEFAULT: std::sync::OnceLock<SandboxInstallPolicy> =
                    std::sync::OnceLock::new();
                DEFAULT.get_or_init(SandboxInstallPolicy::default)
            }
        }
    }

    fn detect_env_drift(&self, kind: SandboxKind, existing: &SandboxEnvRecord) -> Option<String> {
        let runtime_dir = existing.runtime_dir.as_ref()?;
        let healthy = match kind {
            SandboxKind::Python => python_healthcheck(runtime_dir),
            SandboxKind::Node => runtime_dir.is_dir(),
            SandboxKind::Shell | SandboxKind::Processor => existing.env_dir.is_dir(),
        };
        if healthy {
            None
        } else {
            Some(format!(
                "sandbox runtime directory is missing or unhealthy: {}",
                runtime_dir.display()
            ))
        }
    }

    fn validate_install_sources(
        &self,
        kind: SandboxKind,
        dependency_spec: &[String],
        policy: &SandboxInstallPolicy,
    ) -> Result<()> {
        for dependency in dependency_spec {
            let source = classify_install_source(kind, dependency);
            if !policy.allowed_sources.contains(&source) {
                bail!(
                    "sandbox policy rejected dependency '{}' because source '{}' is not allowed",
                    dependency,
                    source.label()
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PreparedEnv {
    status: SandboxEnvStatus,
    runtime_dir: Option<PathBuf>,
    error: Option<String>,
    failure_stage: Option<SandboxFailureStage>,
    strategy: String,
}

impl PreparedEnv {
    fn ready(strategy: impl Into<String>, runtime_dir: Option<PathBuf>) -> Self {
        Self {
            status: SandboxEnvStatus::Ready,
            runtime_dir,
            error: None,
            failure_stage: None,
            strategy: strategy.into(),
        }
    }

    fn failed(
        strategy: impl Into<String>,
        runtime_dir: Option<PathBuf>,
        failure_stage: SandboxFailureStage,
        error: impl Into<String>,
    ) -> Self {
        Self {
            status: SandboxEnvStatus::Failed,
            runtime_dir,
            error: Some(error.into()),
            failure_stage: Some(failure_stage),
            strategy: strategy.into(),
        }
    }
}

struct CommandSpecBuilder {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    env: BTreeMap<String, PathBuf>,
}

impl CommandSpecBuilder {
    fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        }
    }

    fn arg(mut self, arg: impl AsRef<std::ffi::OsStr>) -> Self {
        self.args.push(arg.as_ref().to_string_lossy().into_owned());
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        for arg in args {
            self.args.push(arg.as_ref().to_string_lossy().into_owned());
        }
        self
    }

    fn cwd(mut self, cwd: impl AsRef<Path>) -> Self {
        self.cwd = Some(cwd.as_ref().to_path_buf());
        self
    }

    fn env(mut self, key: impl Into<String>, value: impl AsRef<Path>) -> Self {
        self.env.insert(key.into(), value.as_ref().to_path_buf());
        self
    }
}

fn run_command_with_retries(
    builder: CommandSpecBuilder,
    policy: &SandboxInstallPolicy,
) -> Result<()> {
    let mut last_error = None;
    for _ in 0..=policy.retry_limit {
        match run_command_with_timeout(&builder, policy.timeout_ms) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("sandbox command failed without an error")))
}

fn run_command_with_timeout(builder: &CommandSpecBuilder, timeout_ms: u64) -> Result<()> {
    let mut command = Command::new(&builder.program);
    command.args(&builder.args);
    if let Some(cwd) = &builder.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &builder.env {
        command.env(key, value);
    }
    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to spawn sandbox command '{} {}'",
            builder.program,
            builder.args.join(" ")
        )
    })?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(anyhow!(
                "sandbox command '{} {}' exited with status {}",
                builder.program,
                builder.args.join(" "),
                status
            ));
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!(
                "sandbox command '{} {}' timed out after {}ms",
                builder.program,
                builder.args.join(" "),
                timeout_ms
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn dependency_fingerprint(spec: &[String]) -> String {
    let mut normalized = spec
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn parse_node_dependency(raw: &str) -> (String, String) {
    if let Some((name, source)) = raw.split_once('=') {
        if !name.trim().is_empty() && !source.trim().is_empty() {
            return (name.trim().to_owned(), source.trim().to_owned());
        }
    }
    raw.rsplit_once('@')
        .filter(|(name, version)| !name.is_empty() && !version.is_empty())
        .map(|(name, version)| (name.to_owned(), version.to_owned()))
        .unwrap_or_else(|| (raw.to_owned(), "latest".to_owned()))
}

fn classify_install_source(kind: SandboxKind, dependency: &str) -> SandboxInstallSource {
    let trimmed = dependency.trim();
    match kind {
        SandboxKind::Python => {
            if trimmed.starts_with("file:")
                || trimmed.starts_with('.')
                || trimmed.starts_with('/')
                || trimmed.contains(std::path::MAIN_SEPARATOR)
            {
                SandboxInstallSource::File
            } else {
                SandboxInstallSource::Registry
            }
        }
        SandboxKind::Node => {
            if trimmed.contains("file:") || trimmed.contains(std::path::MAIN_SEPARATOR) {
                SandboxInstallSource::File
            } else {
                SandboxInstallSource::Registry
            }
        }
        SandboxKind::Shell | SandboxKind::Processor => SandboxInstallSource::File,
    }
}

fn python_binary(runtime_dir: &Path) -> PathBuf {
    let unix = runtime_dir.join("bin/python");
    if unix.exists() {
        unix
    } else {
        runtime_dir.join("Scripts/python.exe")
    }
}

fn pip_binary(runtime_dir: &Path) -> PathBuf {
    let unix = runtime_dir.join("bin/pip");
    if unix.exists() {
        unix
    } else {
        runtime_dir.join("Scripts/pip.exe")
    }
}

fn python_healthcheck(runtime_dir: &Path) -> bool {
    python_binary(runtime_dir).exists()
}

fn remove_children(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        removed += 1;
    }
    Ok(removed)
}

fn command_status(command: &str, kind: SandboxKind, strategy: &str) -> SandboxRuntimeStatus {
    SandboxRuntimeStatus {
        kind,
        strategy: strategy.to_owned(),
        available: command_exists(command),
        detail: (!command_exists(command)).then_some(format!("{command} is not available on PATH")),
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "default".to_owned()
    } else {
        trimmed.to_owned()
    }
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
        let dir = std::env::temp_dir().join(format!(
            "mosaic-sandbox-core-{label}-{}-{nanos}-{count}",
            process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn ensure_layout_creates_workspace_local_directories() {
        let root = temp_dir("layout");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let paths = manager.ensure_layout().expect("layout should be created");
        assert!(paths.root.starts_with(&root));
        assert!(paths.python_envs.is_dir());
        assert!(paths.node_envs.is_dir());
        assert!(paths.work_runs.is_dir());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn create_run_workdir_uses_workspace_sandbox_root() {
        let root = temp_dir("run-workdir");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let allocation = manager
            .create_run_workdir("run-123")
            .expect("workdir should be created");
        assert!(allocation.workdir.is_dir());
        assert!(allocation.workdir.starts_with(root.join(".mosaic/sandbox")));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn python_envs_live_under_workspace_sandbox_and_write_requirements() {
        let root = temp_dir("python-env");
        let package_dir = root.join("note_dep");
        fs::create_dir_all(package_dir.join("note_dep")).expect("python package dir");
        fs::write(
            package_dir.join("pyproject.toml"),
            "[build-system]\nrequires = [\"setuptools>=61\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[project]\nname = \"note-dep\"\nversion = \"0.1.0\"\n",
        )
        .expect("pyproject");
        fs::write(package_dir.join("note_dep/__init__.py"), "VALUE = 'note'\n")
            .expect("python module");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let resolution = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Python,
                "operator-notes",
                SandboxScope::Capability,
                vec![package_dir.display().to_string()],
            ))
            .expect("python env should be prepared");
        let record = resolution.record;
        assert!(record.env_dir.starts_with(root.join(".mosaic/sandbox")));
        assert!(record.env_dir.join("requirements.txt").is_file());
        assert_eq!(record.kind, SandboxKind::Python);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn node_envs_are_workspace_local_and_listable() {
        let root = temp_dir("node-env");
        let package_dir = root.join("image-processor-package");
        fs::create_dir_all(&package_dir).expect("node package dir");
        fs::write(
            package_dir.join("package.json"),
            "{\n  \"name\": \"image-processor-package\",\n  \"version\": \"0.1.0\",\n  \"main\": \"index.js\"\n}\n",
        )
        .expect("package json");
        fs::write(
            package_dir.join("index.js"),
            "module.exports = { value: 'ok' };\n",
        )
        .expect("node index");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let resolution = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Node,
                "image-processor",
                SandboxScope::Capability,
                vec![format!(
                    "image-processor-package=file:{}",
                    package_dir.display()
                )],
            ))
            .expect("node env should be prepared");
        let record = resolution.record;
        assert!(record.env_dir.join("package.json").is_file());
        assert_eq!(record.status, SandboxEnvStatus::Ready);
        let listed = manager.list_envs().expect("envs should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].env_id, record.env_id);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn different_workspaces_do_not_share_env_directories() {
        let root_a = temp_dir("workspace-a");
        let root_b = temp_dir("workspace-b");
        let manager_a = SandboxManager::new(&root_a, SandboxSettings::default());
        let manager_b = SandboxManager::new(&root_b, SandboxSettings::default());

        let env_a = manager_a
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Python,
                "shared-name",
                SandboxScope::Capability,
                Vec::new(),
            ))
            .expect("workspace a env should prepare")
            .record;
        let env_b = manager_b
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Python,
                "shared-name",
                SandboxScope::Capability,
                Vec::new(),
            ))
            .expect("workspace b env should prepare")
            .record;

        assert_ne!(env_a.env_dir, env_b.env_dir);
        assert!(env_a.env_dir.starts_with(&root_a));
        assert!(env_b.env_dir.starts_with(&root_b));

        fs::remove_dir_all(root_a).ok();
        fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn rebuild_env_recreates_record() {
        let root = temp_dir("rebuild");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let record = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Processor,
                "doc-parser",
                SandboxScope::Capability,
                vec!["paddle-vl".to_owned()],
            ))
            .expect("processor env should prepare")
            .record;
        let rebuilt = manager
            .rebuild_env(&record.env_id)
            .expect("processor env should rebuild");
        assert_eq!(rebuilt.env_id, record.env_id);
        assert!(rebuilt.env_dir.is_dir());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn python_env_install_uses_workspace_local_venv_and_local_dependency() {
        if !command_exists("python3") {
            return;
        }

        let root = temp_dir("python-install");
        let package_dir = root.join("local_python_dep");
        fs::create_dir_all(package_dir.join("local_python_dep")).expect("python package dir");
        fs::write(
            package_dir.join("pyproject.toml"),
            "[build-system]\nrequires = [\"setuptools>=61\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[project]\nname = \"local-python-dep\"\nversion = \"0.1.0\"\n",
        )
        .expect("pyproject");
        fs::write(
            package_dir.join("local_python_dep/__init__.py"),
            "__all__ = ['VALUE']\nVALUE = 'ok'\n",
        )
        .expect("python module");

        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let resolution = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Python,
                "local-python-dep",
                SandboxScope::Capability,
                vec![package_dir.display().to_string()],
            ))
            .expect("python env should prepare");

        assert!(resolution.prepared);
        assert_eq!(resolution.record.status, SandboxEnvStatus::Ready);
        let site_packages = resolution.record.env_dir.join("venv/lib");
        assert!(site_packages.exists(), "venv lib dir should exist");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn node_env_install_uses_workspace_local_modules_and_local_dependency() {
        if !command_exists("npm") {
            return;
        }

        let root = temp_dir("node-install");
        let package_dir = root.join("local-node-dep");
        fs::create_dir_all(&package_dir).expect("node package dir");
        fs::write(
            package_dir.join("package.json"),
            "{\n  \"name\": \"local-node-dep\",\n  \"version\": \"0.1.0\",\n  \"main\": \"index.js\"\n}\n",
        )
        .expect("package json");
        fs::write(
            package_dir.join("index.js"),
            "module.exports = { value: 'ok' };\n",
        )
        .expect("node index");

        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let resolution = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Node,
                "local-node-dep",
                SandboxScope::Capability,
                vec![format!("local-node-dep=file:{}", package_dir.display())],
            ))
            .expect("node env should prepare");

        assert!(resolution.prepared);
        assert_eq!(resolution.record.status, SandboxEnvStatus::Ready);
        assert!(
            resolution
                .record
                .env_dir
                .join("node_modules/local-node-dep")
                .exists(),
            "local node dependency should be installed in sandbox node_modules"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn dependency_fingerprint_drift_requires_rebuild() {
        let root = temp_dir("rebuild-required");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let first = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Processor,
                "doc-parser",
                SandboxScope::Capability,
                vec!["paddle-vl".to_owned()],
            ))
            .expect("processor env should prepare");
        assert_eq!(first.record.status, SandboxEnvStatus::Ready);

        let second = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Processor,
                "doc-parser",
                SandboxScope::Capability,
                vec!["paddle-vl==2".to_owned()],
            ))
            .expect("processor env should resolve");
        assert_eq!(second.record.status, SandboxEnvStatus::RebuildRequired);
        assert!(!second.prepared);
        fs::remove_dir_all(root).ok();
    }
}
