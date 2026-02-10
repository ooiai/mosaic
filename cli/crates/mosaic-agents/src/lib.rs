use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mosaic_core::config::{ConfigFile, ProfileConfig, RunGuardMode};
use mosaic_core::error::{MosaicError, Result};

const CURRENT_AGENTS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub profile: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_turns: Option<u32>,
    pub tools_enabled: Option<bool>,
    pub guard_mode: Option<RunGuardMode>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AddAgentInput {
    pub id: Option<String>,
    pub name: String,
    pub profile: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_turns: Option<u32>,
    pub tools_enabled: Option<bool>,
    pub guard_mode: Option<RunGuardMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentRoutes {
    pub default_agent_id: Option<String>,
    pub routes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentsFile {
    version: u32,
    agents: Vec<AgentDefinition>,
}

#[derive(Debug, Clone)]
pub struct AgentStore {
    agents_path: PathBuf,
    routes_path: PathBuf,
}

impl AgentStore {
    pub fn new(agents_path: PathBuf, routes_path: PathBuf) -> Self {
        Self {
            agents_path,
            routes_path,
        }
    }

    pub fn agents_path(&self) -> &Path {
        &self.agents_path
    }

    pub fn routes_path(&self) -> &Path {
        &self.routes_path
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        if let Some(parent) = self.agents_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.routes_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<AgentDefinition>> {
        let mut agents = self.load_agents()?.agents;
        agents.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
        Ok(agents)
    }

    pub fn get(&self, agent_id: &str) -> Result<Option<AgentDefinition>> {
        let agent_id = normalize_agent_id(agent_id)?;
        Ok(self
            .load_agents()?
            .agents
            .into_iter()
            .find(|item| item.id == agent_id))
    }

    pub fn add(&self, input: AddAgentInput) -> Result<AgentDefinition> {
        self.ensure_dirs()?;
        validate_agent_fields(
            input.id.as_deref(),
            &input.name,
            &input.profile,
            input.temperature,
            input.max_turns,
        )?;

        let mut file = self.load_agents()?;
        let id = match input.id {
            Some(value) => normalize_agent_id(&value)?,
            None => generate_agent_id(&input.name),
        };
        if file.agents.iter().any(|agent| agent.id == id) {
            return Err(MosaicError::Validation(format!(
                "agent '{id}' already exists"
            )));
        }

        let now = Utc::now();
        let agent = AgentDefinition {
            id,
            name: input.name.trim().to_string(),
            profile: input.profile.trim().to_string(),
            model: input.model.map(|value| value.trim().to_string()),
            temperature: input.temperature,
            max_turns: input.max_turns,
            tools_enabled: input.tools_enabled,
            guard_mode: input.guard_mode,
            created_at: now,
            updated_at: now,
        };
        file.agents.push(agent.clone());
        self.save_agents(&file)?;
        Ok(agent)
    }

    pub fn remove(&self, agent_id: &str) -> Result<bool> {
        let agent_id = normalize_agent_id(agent_id)?;
        let mut file = self.load_agents()?;
        let before = file.agents.len();
        file.agents.retain(|agent| agent.id != agent_id);
        if file.agents.len() == before {
            return Ok(false);
        }
        self.save_agents(&file)?;

        let mut routes = self.load_routes()?;
        if routes.default_agent_id.as_deref() == Some(agent_id.as_str()) {
            routes.default_agent_id = None;
        }
        routes.routes.retain(|_, value| value != &agent_id);
        self.save_routes(&routes)?;
        Ok(true)
    }

    pub fn load_routes(&self) -> Result<AgentRoutes> {
        if !self.routes_path.exists() {
            return Ok(AgentRoutes::default());
        }
        let raw = std::fs::read_to_string(&self.routes_path)?;
        serde_json::from_str::<AgentRoutes>(&raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid routes JSON {}: {err}",
                self.routes_path.display()
            ))
        })
    }

    pub fn set_default(&self, agent_id: &str) -> Result<AgentRoutes> {
        let agent_id = normalize_agent_id(agent_id)?;
        if self.get(&agent_id)?.is_none() {
            return Err(MosaicError::Validation(format!(
                "agent '{agent_id}' not found"
            )));
        }
        let mut routes = self.load_routes()?;
        routes.default_agent_id = Some(agent_id);
        self.save_routes(&routes)?;
        Ok(routes)
    }

    pub fn set_route(&self, route_key: &str, agent_id: &str) -> Result<AgentRoutes> {
        let route_key = normalize_route_key(route_key)?;
        let agent_id = normalize_agent_id(agent_id)?;
        if self.get(&agent_id)?.is_none() {
            return Err(MosaicError::Validation(format!(
                "agent '{agent_id}' not found"
            )));
        }
        let mut routes = self.load_routes()?;
        routes.routes.insert(route_key, agent_id);
        self.save_routes(&routes)?;
        Ok(routes)
    }

    pub fn remove_route(&self, route_key: &str) -> Result<(AgentRoutes, bool)> {
        let route_key = normalize_route_key(route_key)?;
        let mut routes = self.load_routes()?;
        let removed = routes.routes.remove(&route_key).is_some();
        self.save_routes(&routes)?;
        Ok((routes, removed))
    }

    pub fn resolve_for_runtime(
        &self,
        explicit_agent_id: Option<&str>,
        route_hint: Option<&str>,
    ) -> Result<Option<String>> {
        if let Some(agent_id) = explicit_agent_id {
            let agent_id = normalize_agent_id(agent_id)?;
            if self.get(&agent_id)?.is_none() {
                return Err(MosaicError::Validation(format!(
                    "agent '{agent_id}' not found"
                )));
            }
            return Ok(Some(agent_id));
        }

        let routes = self.load_routes()?;
        if let Some(route_key) = route_hint {
            let route_key = normalize_route_key(route_key)?;
            if let Some(agent_id) = routes.routes.get(&route_key) {
                if self.get(agent_id)?.is_none() {
                    return Err(MosaicError::Validation(format!(
                        "route '{route_key}' points to missing agent '{agent_id}'"
                    )));
                }
                return Ok(Some(agent_id.clone()));
            }
        }

        if let Some(default_agent_id) = routes.default_agent_id {
            if self.get(&default_agent_id)?.is_none() {
                return Err(MosaicError::Validation(format!(
                    "default agent '{}' not found",
                    default_agent_id
                )));
            }
            return Ok(Some(default_agent_id));
        }
        Ok(None)
    }

    pub fn resolve_effective_profile(
        &self,
        config: &ConfigFile,
        cli_profile: &str,
        explicit_agent_id: Option<&str>,
        route_hint: Option<&str>,
    ) -> Result<ResolvedAgentProfile> {
        let selected_agent_id = self.resolve_for_runtime(explicit_agent_id, route_hint)?;
        let Some(selected_agent_id) = selected_agent_id else {
            let resolved = config.resolve_profile(Some(cli_profile))?;
            return Ok(ResolvedAgentProfile {
                agent_id: None,
                profile_name: resolved.profile_name,
                profile: resolved.profile,
            });
        };

        let agent = self.get(&selected_agent_id)?.ok_or_else(|| {
            MosaicError::Validation(format!(
                "agent '{}' disappeared during resolution",
                selected_agent_id
            ))
        })?;
        let base = config.resolve_profile(Some(&agent.profile))?;
        let merged = apply_overrides(base.profile, &agent)?;
        Ok(ResolvedAgentProfile {
            agent_id: Some(agent.id),
            profile_name: base.profile_name,
            profile: merged,
        })
    }

    pub fn check_integrity(&self) -> Result<AgentIntegrityReport> {
        let agents = self.list()?;
        let routes = self.load_routes()?;
        let mut issues = Vec::new();

        for agent in &agents {
            if agent.name.trim().is_empty() {
                issues.push(format!("agent '{}' has empty name", agent.id));
            }
            if agent.profile.trim().is_empty() {
                issues.push(format!("agent '{}' has empty profile", agent.id));
            }
        }

        if let Some(default_agent_id) = &routes.default_agent_id {
            if !agents.iter().any(|agent| &agent.id == default_agent_id) {
                issues.push(format!(
                    "default_agent_id '{}' does not exist",
                    default_agent_id
                ));
            }
        }
        for (route, agent_id) in &routes.routes {
            if !agents.iter().any(|agent| &agent.id == agent_id) {
                issues.push(format!(
                    "route '{route}' points to missing agent '{agent_id}'"
                ));
            }
        }

        Ok(AgentIntegrityReport {
            agents_count: agents.len(),
            routes_count: routes.routes.len(),
            default_agent_id: routes.default_agent_id,
            ok: issues.is_empty(),
            issues,
        })
    }

    fn load_agents(&self) -> Result<AgentsFile> {
        if !self.agents_path.exists() {
            return Ok(AgentsFile {
                version: CURRENT_AGENTS_VERSION,
                agents: Vec::new(),
            });
        }
        let raw = std::fs::read_to_string(&self.agents_path)?;
        let file = serde_json::from_str::<AgentsFile>(&raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid agents JSON {}: {err}",
                self.agents_path.display()
            ))
        })?;
        if file.version != CURRENT_AGENTS_VERSION {
            return Err(MosaicError::Validation(format!(
                "unsupported agents file version {} expected {}",
                file.version, CURRENT_AGENTS_VERSION
            )));
        }
        Ok(file)
    }

    fn save_agents(&self, file: &AgentsFile) -> Result<()> {
        if let Some(parent) = self.agents_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(file).map_err(|err| {
            MosaicError::Validation(format!(
                "failed to encode agents JSON {}: {err}",
                self.agents_path.display()
            ))
        })?;
        std::fs::write(&self.agents_path, raw)?;
        Ok(())
    }

    fn save_routes(&self, routes: &AgentRoutes) -> Result<()> {
        if let Some(parent) = self.routes_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(routes).map_err(|err| {
            MosaicError::Validation(format!(
                "failed to encode routes JSON {}: {err}",
                self.routes_path.display()
            ))
        })?;
        std::fs::write(&self.routes_path, raw)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIntegrityReport {
    pub agents_count: usize,
    pub routes_count: usize,
    pub default_agent_id: Option<String>,
    pub ok: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAgentProfile {
    pub agent_id: Option<String>,
    pub profile_name: String,
    pub profile: ProfileConfig,
}

pub fn agents_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("agents.json")
}

pub fn agent_routes_path(data_dir: &Path) -> PathBuf {
    data_dir.join("agent-routes.json")
}

fn apply_overrides(mut base: ProfileConfig, agent: &AgentDefinition) -> Result<ProfileConfig> {
    if let Some(model) = &agent.model {
        if model.trim().is_empty() {
            return Err(MosaicError::Validation(format!(
                "agent '{}' has empty model override",
                agent.id
            )));
        }
        base.provider.model = model.to_string();
    }
    if let Some(temperature) = agent.temperature {
        if !(0.0..=2.0).contains(&temperature) {
            return Err(MosaicError::Validation(format!(
                "agent '{}' temperature must be in [0.0, 2.0]",
                agent.id
            )));
        }
        base.agent.temperature = temperature;
    }
    if let Some(max_turns) = agent.max_turns {
        if max_turns == 0 {
            return Err(MosaicError::Validation(format!(
                "agent '{}' max_turns must be greater than 0",
                agent.id
            )));
        }
        base.agent.max_turns = max_turns;
    }
    if let Some(tools_enabled) = agent.tools_enabled {
        base.tools.enabled = tools_enabled;
    }
    if let Some(guard_mode) = &agent.guard_mode {
        base.tools.run.guard_mode = guard_mode.clone();
    }
    base.validate()?;
    Ok(base)
}

fn normalize_agent_id(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(MosaicError::Validation(
            "agent id cannot be empty".to_string(),
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(MosaicError::Validation(format!(
            "agent id '{value}' contains invalid characters"
        )));
    }
    Ok(value.to_string())
}

fn normalize_route_key(raw: &str) -> Result<String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(MosaicError::Validation(
            "route key cannot be empty".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn generate_agent_id(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if (ch == ' ' || ch == '-' || ch == '_') && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    if slug.is_empty() {
        slug = "agent".to_string();
    }
    let suffix = &Uuid::new_v4().simple().to_string()[..8];
    format!("{slug}-{suffix}")
}

fn validate_agent_fields(
    id: Option<&str>,
    name: &str,
    profile: &str,
    temperature: Option<f32>,
    max_turns: Option<u32>,
) -> Result<()> {
    if let Some(id) = id {
        let _ = normalize_agent_id(id)?;
    }
    if name.trim().is_empty() {
        return Err(MosaicError::Validation(
            "agent name cannot be empty".to_string(),
        ));
    }
    if profile.trim().is_empty() {
        return Err(MosaicError::Validation(
            "agent profile cannot be empty".to_string(),
        ));
    }
    if let Some(value) = temperature {
        if !(0.0..=2.0).contains(&value) {
            return Err(MosaicError::Validation(
                "temperature must be in [0.0, 2.0]".to_string(),
            ));
        }
    }
    if let Some(value) = max_turns {
        if value == 0 {
            return Err(MosaicError::Validation(
                "max_turns must be greater than 0".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mosaic_core::config::{ConfigFile, StateConfig};
    use mosaic_core::state::StateMode;
    use tempfile::tempdir;

    use super::*;

    fn build_store(temp: &tempfile::TempDir) -> AgentStore {
        let data_dir = temp.path().join("data");
        AgentStore::new(agents_file_path(&data_dir), agent_routes_path(&data_dir))
    }

    #[test]
    fn add_list_set_default_and_resolve() {
        let temp = tempdir().expect("tempdir");
        let store = build_store(&temp);
        let agent = store
            .add(AddAgentInput {
                id: Some("writer".to_string()),
                name: "Writer".to_string(),
                profile: "default".to_string(),
                model: Some("mock-model".to_string()),
                temperature: Some(0.3),
                max_turns: Some(4),
                tools_enabled: Some(true),
                guard_mode: None,
            })
            .expect("add agent");
        assert_eq!(agent.id, "writer");
        assert_eq!(store.list().expect("list").len(), 1);

        store.set_default("writer").expect("set default");
        let resolved = store
            .resolve_for_runtime(None, Some("ask"))
            .expect("resolve default");
        assert_eq!(resolved.as_deref(), Some("writer"));
    }

    #[test]
    fn resolve_uses_route_before_default() {
        let temp = tempdir().expect("tempdir");
        let store = build_store(&temp);
        store
            .add(AddAgentInput {
                id: Some("planner".to_string()),
                name: "Planner".to_string(),
                profile: "default".to_string(),
                model: None,
                temperature: None,
                max_turns: None,
                tools_enabled: None,
                guard_mode: None,
            })
            .expect("add planner");
        store
            .add(AddAgentInput {
                id: Some("writer".to_string()),
                name: "Writer".to_string(),
                profile: "default".to_string(),
                model: None,
                temperature: None,
                max_turns: None,
                tools_enabled: None,
                guard_mode: None,
            })
            .expect("add writer");
        store.set_default("writer").expect("set default");
        store.set_route("ask", "planner").expect("set ask route");
        let resolved = store
            .resolve_for_runtime(None, Some("ask"))
            .expect("resolve");
        assert_eq!(resolved.as_deref(), Some("planner"));
    }

    #[test]
    fn resolve_effective_profile_applies_overrides() {
        let temp = tempdir().expect("tempdir");
        let store = build_store(&temp);
        store
            .add(AddAgentInput {
                id: Some("writer".to_string()),
                name: "Writer".to_string(),
                profile: "default".to_string(),
                model: Some("gpt-4o-mini".to_string()),
                temperature: Some(0.9),
                max_turns: Some(12),
                tools_enabled: Some(false),
                guard_mode: Some(RunGuardMode::AllConfirm),
            })
            .expect("add writer");

        let mut config = ConfigFile::default_for_mode(StateMode::Project);
        config.state = StateConfig {
            mode: StateMode::Project,
            project_dir: ".mosaic".to_string(),
        };
        config.profiles = BTreeMap::from([("default".to_string(), ProfileConfig::default())]);
        config.active_profile = "default".to_string();

        let resolved = store
            .resolve_effective_profile(&config, "default", Some("writer"), Some("ask"))
            .expect("resolve profile");
        assert_eq!(resolved.agent_id.as_deref(), Some("writer"));
        assert_eq!(resolved.profile.provider.model, "gpt-4o-mini");
        assert_eq!(resolved.profile.agent.temperature, 0.9);
        assert_eq!(resolved.profile.agent.max_turns, 12);
        assert!(!resolved.profile.tools.enabled);
        assert!(matches!(
            resolved.profile.tools.run.guard_mode,
            RunGuardMode::AllConfirm
        ));
    }
}
