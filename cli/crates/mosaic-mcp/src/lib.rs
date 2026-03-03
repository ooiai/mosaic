use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mosaic_core::error::{MosaicError, Result};

const MCP_SERVERS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_check_at: Option<DateTime<Utc>>,
    pub last_check_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddMcpServerInput {
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerCheck {
    pub server_id: String,
    pub checked_at: DateTime<Utc>,
    pub healthy: bool,
    pub enabled: bool,
    pub executable_resolved: Option<String>,
    pub cwd_exists: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerCheckResult {
    pub server: McpServer,
    pub check: McpServerCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpServersFile {
    #[serde(default = "default_mcp_servers_version")]
    version: u32,
    #[serde(default)]
    servers: Vec<McpServer>,
}

#[derive(Debug, Clone)]
pub struct McpStore {
    path: PathBuf,
}

impl McpStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<McpServer>> {
        let mut servers = self.load_file()?.servers;
        servers.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
        Ok(servers)
    }

    pub fn add(&self, input: AddMcpServerInput) -> Result<McpServer> {
        self.ensure_dirs()?;
        let name = normalize_non_empty("mcp server name", &input.name)?;
        let command = normalize_non_empty("mcp server command", &input.command)?;
        let cwd = normalize_optional(&input.cwd);

        if let Some(path) = cwd.as_deref()
            && path.is_empty()
        {
            return Err(MosaicError::Validation(
                "mcp server cwd cannot be empty".to_string(),
            ));
        }

        for key in input.env.keys() {
            validate_env_key(key)?;
        }

        let mut file = self.load_file()?;
        let id = input
            .id
            .as_deref()
            .map(normalize_server_id)
            .transpose()?
            .unwrap_or_else(|| format!("mcp-{}", Uuid::new_v4().simple()));
        if file.servers.iter().any(|server| server.id == id) {
            return Err(MosaicError::Validation(format!(
                "mcp server '{id}' already exists"
            )));
        }

        let now = Utc::now();
        let server = McpServer {
            id,
            name,
            command,
            args: input
                .args
                .into_iter()
                .map(|value| value.trim().to_string())
                .collect(),
            env: input.env,
            cwd,
            enabled: input.enabled,
            created_at: now,
            updated_at: now,
            last_check_at: None,
            last_check_error: None,
        };
        file.servers.push(server.clone());
        self.save_file(&file)?;
        Ok(server)
    }

    pub fn remove(&self, server_id: &str) -> Result<bool> {
        let server_id = normalize_server_id(server_id)?;
        let mut file = self.load_file()?;
        let before = file.servers.len();
        file.servers.retain(|server| server.id != server_id);
        if file.servers.len() == before {
            return Ok(false);
        }
        self.save_file(&file)?;
        Ok(true)
    }

    pub fn set_enabled(&self, server_id: &str, enabled: bool) -> Result<McpServer> {
        let server_id = normalize_server_id(server_id)?;
        let mut file = self.load_file()?;
        let server = file
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .ok_or_else(|| {
                MosaicError::Validation(format!("mcp server '{server_id}' not found"))
            })?;
        server.enabled = enabled;
        server.updated_at = Utc::now();
        let updated = server.clone();
        self.save_file(&file)?;
        Ok(updated)
    }

    pub fn check(&self, server_id: &str) -> Result<McpServerCheckResult> {
        let server_id = normalize_server_id(server_id)?;
        let mut file = self.load_file()?;
        let server = file
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .ok_or_else(|| {
                MosaicError::Validation(format!("mcp server '{server_id}' not found"))
            })?;

        let checked_at = Utc::now();
        let mut issues = Vec::new();
        let resolved = resolve_executable(&server.command);
        if !server.enabled {
            issues.push("server is disabled".to_string());
        }
        if resolved.is_none() {
            issues.push(format!("command '{}' not found in PATH", server.command));
        }

        let cwd_exists = match server.cwd.as_deref() {
            Some(cwd) => Path::new(cwd).is_dir(),
            None => true,
        };
        if !cwd_exists {
            issues.push(
                server
                    .cwd
                    .as_deref()
                    .map(|cwd| format!("cwd '{cwd}' does not exist or is not a directory"))
                    .unwrap_or_else(|| "cwd does not exist or is not a directory".to_string()),
            );
        }

        let healthy = issues.is_empty();
        server.last_check_at = Some(checked_at);
        server.last_check_error = if healthy {
            None
        } else {
            Some(issues.join("; "))
        };
        server.updated_at = checked_at;

        let check = McpServerCheck {
            server_id: server.id.clone(),
            checked_at,
            healthy,
            enabled: server.enabled,
            executable_resolved: resolved.map(|path| path.display().to_string()),
            cwd_exists,
            issues,
        };
        let record = server.clone();
        self.save_file(&file)?;

        Ok(McpServerCheckResult {
            server: record,
            check,
        })
    }

    fn load_file(&self) -> Result<McpServersFile> {
        if !self.path.exists() {
            return Ok(McpServersFile::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let mut file = serde_json::from_str::<McpServersFile>(&raw).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid mcp servers JSON {}: {err}",
                self.path.display()
            ))
        })?;
        if file.version == 0 {
            file.version = MCP_SERVERS_VERSION;
        }
        Ok(file)
    }

    fn save_file(&self, file: &McpServersFile) -> Result<()> {
        self.ensure_dirs()?;
        let payload = serde_json::to_string_pretty(file).map_err(|err| {
            MosaicError::Validation(format!("failed to encode mcp servers JSON: {err}"))
        })?;
        std::fs::write(&self.path, payload)?;
        Ok(())
    }
}

pub fn mcp_servers_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("mcp-servers.json")
}

fn default_mcp_servers_version() -> u32 {
    MCP_SERVERS_VERSION
}

impl Default for McpServersFile {
    fn default() -> Self {
        Self {
            version: MCP_SERVERS_VERSION,
            servers: Vec::new(),
        }
    }
}

fn normalize_server_id(value: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(MosaicError::Validation(
            "mcp server id cannot be empty".to_string(),
        ));
    }
    if normalized
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
    {
        return Err(MosaicError::Validation(format!(
            "invalid mcp server id '{normalized}'"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_non_empty(field: &str, value: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(MosaicError::Validation(format!("{field} cannot be empty")));
    }
    Ok(normalized.to_string())
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn validate_env_key(key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(MosaicError::Validation(
            "environment key cannot be empty".to_string(),
        ));
    }
    if key
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
    {
        return Err(MosaicError::Validation(format!(
            "invalid environment key '{key}'"
        )));
    }
    Ok(())
}

fn resolve_executable(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let candidate = Path::new(command);
    if candidate.is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
        return executable_if_valid(candidate);
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let joined = dir.join(command);
        if let Some(path) = executable_if_valid(&joined) {
            return Some(path);
        }
        #[cfg(windows)]
        {
            let with_exe = dir.join(format!("{command}.exe"));
            if let Some(path) = executable_if_valid(&with_exe) {
                return Some(path);
            }
        }
    }
    None
}

fn executable_if_valid(path: &Path) -> Option<PathBuf> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn store_add_list_toggle_remove_and_check() {
        let temp = tempdir().expect("tempdir");
        let data_dir = temp.path().join("data");
        let path = mcp_servers_file_path(&data_dir);
        let store = McpStore::new(path);

        let mut env = BTreeMap::new();
        env.insert("MCP_TOKEN".to_string(), "token".to_string());

        let created = store
            .add(AddMcpServerInput {
                id: Some("local-mcp".to_string()),
                name: "Local MCP".to_string(),
                command: std::env::current_exe()
                    .expect("current exe")
                    .display()
                    .to_string(),
                args: vec!["--version".to_string()],
                env,
                cwd: Some(temp.path().display().to_string()),
                enabled: true,
            })
            .expect("add");

        assert_eq!(created.id, "local-mcp");
        assert_eq!(store.list().expect("list").len(), 1);

        let checked = store.check("local-mcp").expect("check");
        assert!(checked.check.healthy);
        assert!(checked.check.executable_resolved.is_some());

        let disabled = store.set_enabled("local-mcp", false).expect("disable");
        assert!(!disabled.enabled);

        let checked_disabled = store.check("local-mcp").expect("check disabled");
        assert!(!checked_disabled.check.healthy);
        assert!(
            checked_disabled
                .check
                .issues
                .iter()
                .any(|issue| issue.contains("disabled"))
        );

        let enabled = store.set_enabled("local-mcp", true).expect("enable");
        assert!(enabled.enabled);

        assert!(store.remove("local-mcp").expect("remove"));
        assert!(store.list().expect("list after remove").is_empty());
    }

    #[test]
    fn check_reports_missing_command() {
        let temp = tempdir().expect("tempdir");
        let path = mcp_servers_file_path(&temp.path().join("data"));
        let store = McpStore::new(path);

        store
            .add(AddMcpServerInput {
                id: Some("missing".to_string()),
                name: "Missing".to_string(),
                command: "__missing_command__".to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: None,
                enabled: true,
            })
            .expect("add");

        let checked = store.check("missing").expect("check");
        assert!(!checked.check.healthy);
        assert!(
            checked
                .check
                .issues
                .iter()
                .any(|issue| issue.contains("not found"))
        );
    }
}
