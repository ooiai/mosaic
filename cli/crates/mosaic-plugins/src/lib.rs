use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use mosaic_core::error::{MosaicError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSource {
    Project,
    CodexHome,
    UserHome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub source: ExtensionSource,
    pub path: String,
    pub manifest_path: String,
    pub manifest_exists: bool,
    pub manifest_valid: bool,
    pub manifest_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub source: ExtensionSource,
    pub path: String,
    pub skill_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCheckItem {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCheckResult {
    pub id: String,
    pub kind: String,
    pub source: ExtensionSource,
    pub path: String,
    pub ok: bool,
    pub checks: Vec<ExtensionCheckItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCheckReport {
    pub kind: String,
    pub target: Option<String>,
    pub ok: bool,
    pub checked: usize,
    pub failed: usize,
    pub results: Vec<ExtensionCheckResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallOutcome {
    pub kind: String,
    pub id: String,
    pub source_path: String,
    pub installed_path: String,
    pub replaced: bool,
}

#[derive(Debug, Clone)]
pub struct RegistryRoots {
    pub state_root: PathBuf,
    pub codex_home: Option<PathBuf>,
    pub user_home: Option<PathBuf>,
}

impl RegistryRoots {
    pub fn from_state_root(state_root: PathBuf) -> Self {
        Self {
            state_root,
            codex_home: std::env::var_os("CODEX_HOME").map(PathBuf::from),
            user_home: dirs::home_dir(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionRegistry {
    roots: RegistryRoots,
}

impl ExtensionRegistry {
    pub fn new(roots: RegistryRoots) -> Self {
        Self { roots }
    }

    pub fn list_plugins(&self) -> Result<Vec<PluginEntry>> {
        let mut items = BTreeMap::<String, PluginEntry>::new();
        for (source, root) in self.plugin_roots() {
            if !root.is_dir() {
                continue;
            }
            let entries = match std::fs::read_dir(&root) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let discovered = discover_plugin_entry(source, &path);
                items.entry(discovered.id.clone()).or_insert(discovered);
            }
        }
        Ok(items.into_values().collect())
    }

    pub fn plugin_info(&self, plugin_id: &str) -> Result<PluginEntry> {
        self.list_plugins()?
            .into_iter()
            .find(|item| item.id == plugin_id)
            .ok_or_else(|| MosaicError::Validation(format!("plugin '{plugin_id}' not found")))
    }

    pub fn check_plugins(&self, plugin_id: Option<&str>) -> Result<ExtensionCheckReport> {
        let plugins = if let Some(plugin_id) = plugin_id {
            vec![self.plugin_info(plugin_id)?]
        } else {
            self.list_plugins()?
        };

        let mut failed = 0usize;
        let mut results = Vec::with_capacity(plugins.len());
        for plugin in plugins {
            let mut checks = Vec::new();
            let path = PathBuf::from(&plugin.path);
            checks.push(check(
                "plugin_dir_exists",
                path.is_dir(),
                plugin.path.clone(),
            ));
            checks.push(check(
                "plugin_manifest_exists",
                plugin.manifest_exists,
                plugin.manifest_path.clone(),
            ));
            checks.push(check(
                "plugin_manifest_valid",
                plugin.manifest_valid,
                plugin
                    .manifest_error
                    .clone()
                    .unwrap_or_else(|| "manifest parsed".to_string()),
            ));
            let ok = checks.iter().all(|item| item.ok);
            if !ok {
                failed += 1;
            }
            results.push(ExtensionCheckResult {
                id: plugin.id,
                kind: "plugin".to_string(),
                source: plugin.source,
                path: plugin.path,
                ok,
                checks,
            });
        }

        Ok(ExtensionCheckReport {
            kind: "plugin".to_string(),
            target: plugin_id.map(ToString::to_string),
            ok: failed == 0,
            checked: results.len(),
            failed,
            results,
        })
    }

    pub fn install_plugin_from_path(
        &self,
        source_path: &Path,
        force: bool,
    ) -> Result<InstallOutcome> {
        let source_dir = canonicalize_existing_dir(source_path, "plugin source path")?;
        let discovered = discover_plugin_entry(ExtensionSource::Project, &source_dir);
        if !discovered.manifest_exists {
            return Err(MosaicError::Validation(format!(
                "plugin source {} is missing plugin.toml",
                source_dir.display()
            )));
        }
        if !discovered.manifest_valid {
            return Err(MosaicError::Validation(
                discovered
                    .manifest_error
                    .unwrap_or_else(|| "plugin manifest is invalid".to_string()),
            ));
        }
        let plugin_id = normalize_package_id(&discovered.id, "plugin id")?;
        let destination_root = self.roots.state_root.join("plugins");
        let destination = destination_root.join(&plugin_id);
        let source_real = std::fs::canonicalize(&source_dir).map_err(|err| {
            MosaicError::Io(format!(
                "failed to canonicalize plugin source {}: {err}",
                source_dir.display()
            ))
        })?;
        let destination_real = std::fs::canonicalize(&destination).ok();
        if destination_real.as_ref() == Some(&source_real) {
            return Ok(InstallOutcome {
                kind: "plugin".to_string(),
                id: plugin_id,
                source_path: source_real.display().to_string(),
                installed_path: source_real.display().to_string(),
                replaced: false,
            });
        }

        let existed = destination.exists();
        if existed && !force {
            return Err(MosaicError::Validation(format!(
                "plugin '{}' already exists at {} (use --force to replace)",
                plugin_id,
                destination.display()
            )));
        }
        if existed {
            std::fs::remove_dir_all(&destination)?;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_dir_recursive(&source_dir, &destination)?;
        let installed = discover_plugin_entry(ExtensionSource::Project, &destination);
        if !installed.manifest_valid {
            return Err(MosaicError::Validation(
                installed
                    .manifest_error
                    .unwrap_or_else(|| "installed plugin manifest is invalid".to_string()),
            ));
        }
        Ok(InstallOutcome {
            kind: "plugin".to_string(),
            id: installed.id,
            source_path: source_dir.display().to_string(),
            installed_path: destination.display().to_string(),
            replaced: existed,
        })
    }

    pub fn remove_project_plugin(&self, plugin_id: &str) -> Result<bool> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Err(MosaicError::Validation(
                "plugin id cannot be empty".to_string(),
            ));
        }
        let target = self
            .list_plugins()?
            .into_iter()
            .find(|item| item.id == plugin_id && item.source == ExtensionSource::Project);
        let Some(target) = target else {
            return Ok(false);
        };
        let path = PathBuf::from(target.path);
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_dir_all(path)?;
        Ok(true)
    }

    pub fn list_skills(&self) -> Result<Vec<SkillEntry>> {
        let mut items = BTreeMap::<String, SkillEntry>::new();
        for (source, root) in self.skill_roots() {
            if !root.is_dir() {
                continue;
            }
            for entry in WalkDir::new(&root).min_depth(1).max_depth(5) {
                let entry = match entry {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                if entry.file_name().to_string_lossy() != "SKILL.md" {
                    continue;
                }
                let skill_file = entry.path();
                let Some(skill_dir) = skill_file.parent() else {
                    continue;
                };
                let relative = skill_dir.strip_prefix(&root).ok();
                let id = relative
                    .map(path_to_id)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| {
                        skill_dir
                            .file_name()
                            .map(|value| value.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    });
                if items.contains_key(&id) {
                    continue;
                }

                let fallback_title = skill_dir
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| id.clone());
                let (title, description) = match std::fs::read_to_string(skill_file) {
                    Ok(content) => extract_skill_summary(&content, &fallback_title),
                    Err(_) => (fallback_title, None),
                };

                let discovered = SkillEntry {
                    id: id.clone(),
                    title,
                    description,
                    source,
                    path: skill_dir.display().to_string(),
                    skill_file: skill_file.display().to_string(),
                };
                items.insert(id, discovered);
            }
        }
        Ok(items.into_values().collect())
    }

    pub fn skill_info(&self, skill_id: &str) -> Result<SkillEntry> {
        self.list_skills()?
            .into_iter()
            .find(|item| item.id == skill_id)
            .ok_or_else(|| MosaicError::Validation(format!("skill '{skill_id}' not found")))
    }

    pub fn check_skills(&self, skill_id: Option<&str>) -> Result<ExtensionCheckReport> {
        let skills = if let Some(skill_id) = skill_id {
            vec![self.skill_info(skill_id)?]
        } else {
            self.list_skills()?
        };

        let mut failed = 0usize;
        let mut results = Vec::with_capacity(skills.len());
        for skill in skills {
            let mut checks = Vec::new();
            let skill_dir = PathBuf::from(&skill.path);
            let skill_file = PathBuf::from(&skill.skill_file);
            checks.push(check(
                "skill_dir_exists",
                skill_dir.is_dir(),
                skill.path.clone(),
            ));
            checks.push(check(
                "skill_file_exists",
                skill_file.is_file(),
                skill.skill_file.clone(),
            ));
            match std::fs::read_to_string(&skill_file) {
                Ok(content) => {
                    checks.push(check(
                        "skill_file_non_empty",
                        !content.trim().is_empty(),
                        "trimmed content",
                    ));
                    checks.push(check(
                        "skill_heading_present",
                        content
                            .lines()
                            .any(|line| line.trim_start().starts_with("# ")),
                        "expected markdown heading (# ...)",
                    ));
                }
                Err(err) => {
                    checks.push(check(
                        "skill_file_non_empty",
                        false,
                        format!("failed to read skill file: {err}"),
                    ));
                    checks.push(check(
                        "skill_heading_present",
                        false,
                        "skipped due to read failure",
                    ));
                }
            }
            let ok = checks.iter().all(|item| item.ok);
            if !ok {
                failed += 1;
            }
            results.push(ExtensionCheckResult {
                id: skill.id,
                kind: "skill".to_string(),
                source: skill.source,
                path: skill.path,
                ok,
                checks,
            });
        }

        Ok(ExtensionCheckReport {
            kind: "skill".to_string(),
            target: skill_id.map(ToString::to_string),
            ok: failed == 0,
            checked: results.len(),
            failed,
            results,
        })
    }

    pub fn install_skill_from_path(
        &self,
        source_path: &Path,
        force: bool,
    ) -> Result<InstallOutcome> {
        let source_dir = canonicalize_existing_dir(source_path, "skill source path")?;
        let skill_file = source_dir.join("SKILL.md");
        if !skill_file.is_file() {
            return Err(MosaicError::Validation(format!(
                "skill source {} is missing SKILL.md",
                source_dir.display()
            )));
        }
        let skill_id = source_dir
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .ok_or_else(|| {
                MosaicError::Validation(format!(
                    "unable to derive skill id from {}",
                    source_dir.display()
                ))
            })?;
        let skill_id = normalize_package_id(&skill_id, "skill id")?;
        let destination_root = self.roots.state_root.join("skills");
        let destination = destination_root.join(&skill_id);
        let source_real = std::fs::canonicalize(&source_dir).map_err(|err| {
            MosaicError::Io(format!(
                "failed to canonicalize skill source {}: {err}",
                source_dir.display()
            ))
        })?;
        let destination_real = std::fs::canonicalize(&destination).ok();
        if destination_real.as_ref() == Some(&source_real) {
            return Ok(InstallOutcome {
                kind: "skill".to_string(),
                id: skill_id,
                source_path: source_real.display().to_string(),
                installed_path: source_real.display().to_string(),
                replaced: false,
            });
        }

        let existed = destination.exists();
        if existed && !force {
            return Err(MosaicError::Validation(format!(
                "skill '{}' already exists at {} (use --force to replace)",
                skill_id,
                destination.display()
            )));
        }
        if existed {
            std::fs::remove_dir_all(&destination)?;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_dir_recursive(&source_dir, &destination)?;
        Ok(InstallOutcome {
            kind: "skill".to_string(),
            id: skill_id,
            source_path: source_dir.display().to_string(),
            installed_path: destination.display().to_string(),
            replaced: existed,
        })
    }

    pub fn remove_project_skill(&self, skill_id: &str) -> Result<bool> {
        let skill_id = skill_id.trim();
        if skill_id.is_empty() {
            return Err(MosaicError::Validation(
                "skill id cannot be empty".to_string(),
            ));
        }
        let target = self
            .list_skills()?
            .into_iter()
            .find(|item| item.id == skill_id && item.source == ExtensionSource::Project);
        let Some(target) = target else {
            return Ok(false);
        };
        let path = PathBuf::from(target.path);
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_dir_all(path)?;
        Ok(true)
    }

    fn plugin_roots(&self) -> Vec<(ExtensionSource, PathBuf)> {
        let mut raw = vec![(
            ExtensionSource::Project,
            self.roots.state_root.join("plugins"),
        )];
        if let Some(codex_home) = &self.roots.codex_home {
            raw.push((ExtensionSource::CodexHome, codex_home.join("plugins")));
        }
        if let Some(user_home) = &self.roots.user_home {
            raw.push((
                ExtensionSource::UserHome,
                user_home.join(".codex").join("plugins"),
            ));
        }
        dedupe_roots(raw)
    }

    fn skill_roots(&self) -> Vec<(ExtensionSource, PathBuf)> {
        let mut raw = vec![(
            ExtensionSource::Project,
            self.roots.state_root.join("skills"),
        )];
        if let Some(codex_home) = &self.roots.codex_home {
            raw.push((ExtensionSource::CodexHome, codex_home.join("skills")));
        }
        if let Some(user_home) = &self.roots.user_home {
            raw.push((
                ExtensionSource::UserHome,
                user_home.join(".codex").join("skills"),
            ));
        }
        dedupe_roots(raw)
    }
}

fn check(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> ExtensionCheckItem {
    ExtensionCheckItem {
        name: name.into(),
        ok,
        detail: detail.into(),
    }
}

fn path_to_id(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn dedupe_roots(roots: Vec<(ExtensionSource, PathBuf)>) -> Vec<(ExtensionSource, PathBuf)> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for (source, root) in roots {
        let key = root.to_string_lossy().to_string();
        if seen.insert(key) {
            unique.push((source, root));
        }
    }
    unique
}

fn normalize_package_id(raw: &str, field_name: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(MosaicError::Validation(format!(
            "{field_name} cannot be empty"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(MosaicError::Validation(format!(
            "{field_name} '{value}' contains invalid characters"
        )));
    }
    Ok(value.to_string())
}

fn canonicalize_existing_dir(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.exists() {
        return Err(MosaicError::Validation(format!(
            "{label} '{}' does not exist",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(MosaicError::Validation(format!(
            "{label} '{}' must be a directory",
            path.display()
        )));
    }
    std::fs::canonicalize(path).map_err(|err| {
        MosaicError::Io(format!(
            "failed to canonicalize {label} '{}': {err}",
            path.display()
        ))
    })
}

fn copy_dir_recursive(source_dir: &Path, destination_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(source_dir) {
        let entry = match entry {
            Ok(value) => value,
            Err(err) => {
                return Err(MosaicError::Io(format!(
                    "failed to walk {}: {err}",
                    source_dir.display()
                )));
            }
        };
        let path = entry.path();
        let relative = path.strip_prefix(source_dir).map_err(|err| {
            MosaicError::Io(format!(
                "failed to compute relative path for {}: {err}",
                path.display()
            ))
        })?;
        let target_path = destination_dir.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path)?;
            continue;
        }
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, &target_path).map_err(|err| {
            MosaicError::Io(format!(
                "failed to copy {} -> {}: {err}",
                path.display(),
                target_path.display()
            ))
        })?;
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize)]
struct PluginManifest {
    plugin: Option<PluginManifestPlugin>,
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PluginManifestPlugin {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
}

impl PluginManifest {
    fn plugin_id(&self) -> Option<String> {
        self.plugin
            .as_ref()
            .and_then(|plugin| plugin.id.clone())
            .or_else(|| self.id.clone())
    }

    fn plugin_name(&self) -> Option<String> {
        self.plugin
            .as_ref()
            .and_then(|plugin| plugin.name.clone())
            .or_else(|| self.name.clone())
    }

    fn plugin_version(&self) -> Option<String> {
        self.plugin
            .as_ref()
            .and_then(|plugin| plugin.version.clone())
            .or_else(|| self.version.clone())
    }

    fn plugin_description(&self) -> Option<String> {
        self.plugin
            .as_ref()
            .and_then(|plugin| plugin.description.clone())
            .or_else(|| self.description.clone())
    }
}

fn discover_plugin_entry(source: ExtensionSource, plugin_dir: &Path) -> PluginEntry {
    let fallback_id = plugin_dir
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let manifest_path = plugin_dir.join("plugin.toml");
    let mut entry = PluginEntry {
        id: fallback_id.clone(),
        name: fallback_id,
        version: None,
        description: None,
        source,
        path: plugin_dir.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        manifest_exists: manifest_path.is_file(),
        manifest_valid: false,
        manifest_error: None,
    };

    if !entry.manifest_exists {
        return entry;
    }

    let raw = match std::fs::read_to_string(&manifest_path) {
        Ok(value) => value,
        Err(err) => {
            entry.manifest_error = Some(format!("failed to read manifest: {err}"));
            return entry;
        }
    };
    let parsed = match toml::from_str::<PluginManifest>(&raw) {
        Ok(value) => value,
        Err(err) => {
            entry.manifest_error = Some(format!("failed to parse manifest: {err}"));
            return entry;
        }
    };

    if let Some(id) = parsed.plugin_id() {
        if !id.trim().is_empty() {
            entry.id = id.trim().to_string();
        }
    }
    if let Some(name) = parsed.plugin_name() {
        if !name.trim().is_empty() {
            entry.name = name.trim().to_string();
        }
    }
    entry.version = parsed
        .plugin_version()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    entry.description = parsed
        .plugin_description()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    entry.manifest_valid = true;
    entry.manifest_error = None;
    entry
}

fn extract_skill_summary(content: &str, fallback_title: &str) -> (String, Option<String>) {
    let mut title = None;
    let mut description = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if title.is_none() && trimmed.starts_with("# ") {
            title = Some(trimmed.trim_start_matches("# ").trim().to_string());
            continue;
        }
        if description.is_none() && !trimmed.starts_with('#') {
            description = Some(trimmed.to_string());
        }
        if title.is_some() && description.is_some() {
            break;
        }
    }

    (
        title
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback_title.to_string()),
        description.filter(|value| !value.is_empty()),
    )
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn plugins_prefer_project_root() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join(".mosaic");
        let codex_home = temp.path().join("codex");

        let project_plugin_dir = state_root.join("plugins").join("demo");
        std::fs::create_dir_all(&project_plugin_dir).expect("create project plugin");
        std::fs::write(
            project_plugin_dir.join("plugin.toml"),
            "[plugin]\nid = \"demo\"\nname = \"Demo Project Plugin\"\nversion = \"1.0.0\"\n",
        )
        .expect("write project plugin manifest");

        let codex_plugin_dir = codex_home.join("plugins").join("demo");
        std::fs::create_dir_all(&codex_plugin_dir).expect("create codex plugin");
        std::fs::write(
            codex_plugin_dir.join("plugin.toml"),
            "[plugin]\nid = \"demo\"\nname = \"Demo Codex Plugin\"\nversion = \"9.9.9\"\n",
        )
        .expect("write codex plugin manifest");

        let registry = ExtensionRegistry::new(RegistryRoots {
            state_root,
            codex_home: Some(codex_home),
            user_home: None,
        });
        let plugins = registry.list_plugins().expect("list plugins");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "demo");
        assert_eq!(plugins[0].name, "Demo Project Plugin");
        assert_eq!(plugins[0].source, ExtensionSource::Project);
    }

    #[test]
    fn skill_discovery_and_check() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join(".mosaic");
        let skill_dir = state_root.join("skills").join("writer");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Writer\nGenerate concise release notes.\n",
        )
        .expect("write skill markdown");

        let registry = ExtensionRegistry::new(RegistryRoots {
            state_root,
            codex_home: None,
            user_home: None,
        });
        let skills = registry.list_skills().expect("list skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "writer");
        assert_eq!(skills[0].title, "Writer");

        let report = registry.check_skills(None).expect("check skills");
        assert!(report.ok);
        assert_eq!(report.checked, 1);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn plugin_check_fails_without_manifest() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join(".mosaic");
        let plugin_dir = state_root.join("plugins").join("no-manifest");
        std::fs::create_dir_all(&plugin_dir).expect("create plugin dir");

        let registry = ExtensionRegistry::new(RegistryRoots {
            state_root,
            codex_home: None,
            user_home: None,
        });
        let report = registry.check_plugins(None).expect("check plugins");
        assert!(!report.ok);
        assert_eq!(report.checked, 1);
        assert_eq!(report.failed, 1);
    }

    #[test]
    fn plugin_install_and_remove_flow() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join(".mosaic");
        let registry = ExtensionRegistry::new(RegistryRoots {
            state_root: state_root.clone(),
            codex_home: None,
            user_home: None,
        });

        let source_dir = temp.path().join("plugin-source");
        std::fs::create_dir_all(&source_dir).expect("create source");
        std::fs::write(
            source_dir.join("plugin.toml"),
            "[plugin]\nid = \"hello_plugin\"\nname = \"Hello\"\nversion = \"0.1.0\"\n",
        )
        .expect("write manifest");
        std::fs::write(source_dir.join("README.md"), "hello plugin").expect("write readme");

        let installed = registry
            .install_plugin_from_path(&source_dir, false)
            .expect("install plugin");
        assert_eq!(installed.kind, "plugin");
        assert_eq!(installed.id, "hello_plugin");
        assert!(PathBuf::from(installed.installed_path).exists());

        let removed = registry
            .remove_project_plugin("hello_plugin")
            .expect("remove plugin");
        assert!(removed);
        let after = registry.list_plugins().expect("list after remove");
        assert!(after.is_empty());
    }

    #[test]
    fn skill_install_and_remove_flow() {
        let temp = tempdir().expect("tempdir");
        let state_root = temp.path().join(".mosaic");
        let registry = ExtensionRegistry::new(RegistryRoots {
            state_root,
            codex_home: None,
            user_home: None,
        });

        let source_dir = temp.path().join("writer");
        std::fs::create_dir_all(&source_dir).expect("create source");
        std::fs::write(
            source_dir.join("SKILL.md"),
            "# Writer\nGenerate short release notes.\n",
        )
        .expect("write skill file");
        std::fs::write(source_dir.join("template.md"), "template").expect("write asset");

        let installed = registry
            .install_skill_from_path(&source_dir, false)
            .expect("install skill");
        assert_eq!(installed.kind, "skill");
        assert_eq!(installed.id, "writer");
        assert!(PathBuf::from(installed.installed_path).exists());

        let removed = registry
            .remove_project_skill("writer")
            .expect("remove skill");
        assert!(removed);
        let after = registry.list_skills().expect("list skills");
        assert!(after.is_empty());
    }
}
