use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxSettings {
    pub base_dir: PathBuf,
    pub python_strategy: PythonEnvStrategy,
    pub node_strategy: NodeEnvStrategy,
    pub cleanup: SandboxCleanupPolicy,
}

impl Default for SandboxSettings {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(".mosaic/sandbox"),
            python_strategy: PythonEnvStrategy::Venv,
            node_strategy: NodeEnvStrategy::Npm,
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
    Ready,
    LayoutOnly,
    MissingRuntime,
    Broken,
}

impl SandboxEnvStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::LayoutOnly => "layout_only",
            Self::MissingRuntime => "missing_runtime",
            Self::Broken => "broken",
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
    pub strategy: String,
    pub env_dir: PathBuf,
    pub cache_dir: PathBuf,
    #[serde(default)]
    pub runtime_dir: Option<PathBuf>,
    pub status: SandboxEnvStatus,
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

    pub fn ensure_env(&self, binding: &SandboxBinding) -> Result<SandboxEnvRecord> {
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

        let (status, runtime_dir, error, strategy) = match binding.kind {
            SandboxKind::Python => self.prepare_python_env(&env_dir, binding)?,
            SandboxKind::Node => self.prepare_node_env(&env_dir, binding)?,
            SandboxKind::Shell => self.prepare_shell_env(&env_dir, binding),
            SandboxKind::Processor => self.prepare_processor_env(&env_dir, binding),
        };

        let record = SandboxEnvRecord {
            env_id: binding.env_id(),
            kind: binding.kind,
            scope: binding.scope,
            env_name: binding.env_name.clone(),
            dependency_spec: binding.dependency_spec.clone(),
            strategy,
            env_dir,
            cache_dir,
            runtime_dir,
            status,
            error,
            created_at,
            updated_at: now,
        };
        self.write_env_record(&record)?;
        Ok(record)
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
        match self.settings.python_strategy {
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
        match self.settings.node_strategy {
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
        binding: &SandboxBinding,
    ) -> Result<(SandboxEnvStatus, Option<PathBuf>, Option<String>, String)> {
        let requirements = env_dir.join("requirements.txt");
        if !binding.dependency_spec.is_empty() {
            fs::write(&requirements, binding.dependency_spec.join("\n") + "\n")
                .with_context(|| format!("failed to write {}", requirements.display()))?;
        }
        match self.settings.python_strategy {
            PythonEnvStrategy::Disabled => Ok((
                SandboxEnvStatus::MissingRuntime,
                None,
                Some("python sandbox strategy is disabled".to_owned()),
                PythonEnvStrategy::Disabled.label().to_owned(),
            )),
            PythonEnvStrategy::Uv => Ok((
                if command_exists("uv") {
                    SandboxEnvStatus::LayoutOnly
                } else {
                    SandboxEnvStatus::MissingRuntime
                },
                Some(env_dir.join("uv")),
                (!command_exists("uv")).then_some("uv is not available on PATH".to_owned()),
                PythonEnvStrategy::Uv.label().to_owned(),
            )),
            PythonEnvStrategy::Venv => {
                let runtime_dir = env_dir.join("venv");
                if command_exists("python3") {
                    if !runtime_dir.exists() {
                        let status = Command::new("python3")
                            .args(["-m", "venv"])
                            .arg(&runtime_dir)
                            .status()
                            .with_context(|| "failed to invoke python3 -m venv")?;
                        if !status.success() {
                            return Ok((
                                SandboxEnvStatus::Broken,
                                Some(runtime_dir),
                                Some("python3 -m venv returned a non-zero exit code".to_owned()),
                                PythonEnvStrategy::Venv.label().to_owned(),
                            ));
                        }
                    }
                    Ok((
                        SandboxEnvStatus::Ready,
                        Some(runtime_dir),
                        None,
                        PythonEnvStrategy::Venv.label().to_owned(),
                    ))
                } else {
                    Ok((
                        SandboxEnvStatus::MissingRuntime,
                        Some(runtime_dir),
                        Some("python3 is not available on PATH".to_owned()),
                        PythonEnvStrategy::Venv.label().to_owned(),
                    ))
                }
            }
        }
    }

    fn prepare_node_env(
        &self,
        env_dir: &Path,
        binding: &SandboxBinding,
    ) -> Result<(SandboxEnvStatus, Option<PathBuf>, Option<String>, String)> {
        let package_json = env_dir.join("package.json");
        let mut dependencies = BTreeMap::new();
        for dependency in &binding.dependency_spec {
            let trimmed = dependency.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (name, version) = trimmed
                .split_once('@')
                .filter(|(name, _)| !name.is_empty())
                .map(|(name, version)| (name.to_owned(), version.to_owned()))
                .unwrap_or_else(|| (trimmed.to_owned(), "latest".to_owned()));
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

        let strategy = self.settings.node_strategy;
        let runtime_dir = env_dir.join("node_modules");
        let runtime_available = match strategy {
            NodeEnvStrategy::Disabled => false,
            NodeEnvStrategy::LayoutOnly => true,
            NodeEnvStrategy::Npm => command_exists("npm"),
            NodeEnvStrategy::Pnpm => command_exists("pnpm"),
        };

        let status = match strategy {
            NodeEnvStrategy::Disabled => SandboxEnvStatus::MissingRuntime,
            NodeEnvStrategy::LayoutOnly => SandboxEnvStatus::LayoutOnly,
            NodeEnvStrategy::Npm | NodeEnvStrategy::Pnpm if runtime_available => {
                SandboxEnvStatus::LayoutOnly
            }
            NodeEnvStrategy::Npm | NodeEnvStrategy::Pnpm => SandboxEnvStatus::MissingRuntime,
        };
        let error = if runtime_available {
            None
        } else {
            Some(format!(
                "{} is not available on PATH",
                match strategy {
                    NodeEnvStrategy::Npm => "npm",
                    NodeEnvStrategy::Pnpm => "pnpm",
                    NodeEnvStrategy::Disabled => "node sandbox strategy",
                    NodeEnvStrategy::LayoutOnly => "node runtime",
                }
            ))
        };
        Ok((
            status,
            Some(runtime_dir),
            error,
            strategy.label().to_owned(),
        ))
    }

    fn prepare_shell_env(
        &self,
        env_dir: &Path,
        binding: &SandboxBinding,
    ) -> (SandboxEnvStatus, Option<PathBuf>, Option<String>, String) {
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
        (
            if available {
                SandboxEnvStatus::LayoutOnly
            } else {
                SandboxEnvStatus::MissingRuntime
            },
            Some(env_dir.to_path_buf()),
            (!available).then_some("sh is not available on PATH".to_owned()),
            "sh".to_owned(),
        )
    }

    fn prepare_processor_env(
        &self,
        env_dir: &Path,
        binding: &SandboxBinding,
    ) -> (SandboxEnvStatus, Option<PathBuf>, Option<String>, String) {
        let manifest = env_dir.join("processor.json");
        let _ = fs::write(
            &manifest,
            serde_json::to_vec_pretty(&serde_json::json!({
                "name": binding.env_name,
                "dependencies": binding.dependency_spec,
            }))
            .unwrap_or_default(),
        );
        (
            SandboxEnvStatus::LayoutOnly,
            Some(env_dir.to_path_buf()),
            None,
            "processor".to_owned(),
        )
    }
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
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let record = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Python,
                "operator-notes",
                SandboxScope::Capability,
                vec!["requests==2.32.0".to_owned()],
            ))
            .expect("python env should be prepared");
        assert!(record.env_dir.starts_with(root.join(".mosaic/sandbox")));
        assert!(record.env_dir.join("requirements.txt").is_file());
        assert_eq!(record.kind, SandboxKind::Python);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn node_envs_are_workspace_local_and_listable() {
        let root = temp_dir("node-env");
        let manager = SandboxManager::new(&root, SandboxSettings::default());
        let record = manager
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Node,
                "image-processor",
                SandboxScope::Capability,
                vec!["sharp@1.0.0".to_owned()],
            ))
            .expect("node env should be prepared");
        assert!(record.env_dir.join("package.json").is_file());
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
                vec!["python".to_owned()],
            ))
            .expect("workspace a env should prepare");
        let env_b = manager_b
            .ensure_env(&SandboxBinding::new(
                SandboxKind::Python,
                "shared-name",
                SandboxScope::Capability,
                vec!["python".to_owned()],
            ))
            .expect("workspace b env should prepare");

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
            .expect("processor env should prepare");
        let rebuilt = manager
            .rebuild_env(&record.env_id)
            .expect("processor env should rebuild");
        assert_eq!(rebuilt.env_id, record.env_id);
        assert!(rebuilt.env_dir.is_dir());
        fs::remove_dir_all(root).ok();
    }
}
