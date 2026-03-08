use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

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

#[derive(Debug, Clone, Copy)]
pub struct McpDiagnoseOptions {
    pub timeout_ms: u64,
}

impl Default for McpDiagnoseOptions {
    fn default() -> Self {
        Self { timeout_ms: 2_000 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct McpProtocolProbe {
    pub attempted: bool,
    pub timeout_ms: u64,
    pub duration_ms: u128,
    pub handshake_ok: bool,
    pub response_kind: Option<String>,
    pub response_preview: Option<String>,
    pub stderr_preview: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerDiagnoseResult {
    pub server: McpServer,
    pub check: McpServerCheck,
    pub protocol_probe: McpProtocolProbe,
    pub healthy: bool,
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

    pub fn get(&self, server_id: &str) -> Result<Option<McpServer>> {
        let server_id = normalize_server_id(server_id)?;
        Ok(self
            .load_file()?
            .servers
            .into_iter()
            .find(|server| server.id == server_id))
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

    pub fn set_cwd(&self, server_id: &str, cwd: Option<String>) -> Result<McpServer> {
        let server_id = normalize_server_id(server_id)?;
        let mut file = self.load_file()?;
        let server = file
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .ok_or_else(|| {
                MosaicError::Validation(format!("mcp server '{server_id}' not found"))
            })?;
        server.cwd = normalize_optional(&cwd);
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

        let check = evaluate_server_check(server);
        apply_server_check(server, &check);
        let record = server.clone();
        self.save_file(&file)?;

        Ok(McpServerCheckResult {
            server: record,
            check,
        })
    }

    pub fn check_all(&self) -> Result<Vec<McpServerCheckResult>> {
        let mut file = self.load_file()?;
        let mut results = Vec::with_capacity(file.servers.len());
        for server in &mut file.servers {
            let check = evaluate_server_check(server);
            apply_server_check(server, &check);
            results.push(McpServerCheckResult {
                server: server.clone(),
                check,
            });
        }
        self.save_file(&file)?;
        results.sort_by(|lhs, rhs| lhs.server.id.cmp(&rhs.server.id));
        Ok(results)
    }

    pub fn diagnose(
        &self,
        server_id: &str,
        options: McpDiagnoseOptions,
    ) -> Result<McpServerDiagnoseResult> {
        let checked = self.check(server_id)?;
        let timeout_ms = options.timeout_ms.max(100);
        let probe = if checked.check.executable_resolved.is_none() || !checked.check.cwd_exists {
            skipped_probe(timeout_ms)
        } else {
            run_stdio_initialize_probe(&checked.server, timeout_ms)
        };

        let healthy = checked.check.healthy && probe.handshake_ok;
        Ok(McpServerDiagnoseResult {
            server: checked.server,
            check: checked.check,
            protocol_probe: probe,
            healthy,
        })
    }

    pub fn diagnose_all(
        &self,
        options: McpDiagnoseOptions,
    ) -> Result<Vec<McpServerDiagnoseResult>> {
        let checks = self.check_all()?;
        if checks.is_empty() {
            return Ok(Vec::new());
        }

        let timeout_ms = options.timeout_ms.max(100);
        let count = checks.len();
        let (tx, rx) = mpsc::channel::<(usize, McpServerDiagnoseResult)>();

        for (index, checked) in checks.into_iter().enumerate() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let probe =
                    if checked.check.executable_resolved.is_none() || !checked.check.cwd_exists {
                        skipped_probe(timeout_ms)
                    } else {
                        run_stdio_initialize_probe(&checked.server, timeout_ms)
                    };
                let result = McpServerDiagnoseResult {
                    healthy: checked.check.healthy && probe.handshake_ok,
                    server: checked.server,
                    check: checked.check,
                    protocol_probe: probe,
                };
                let _ = tx.send((index, result));
            });
        }
        drop(tx);

        let mut ordered = vec![None; count];
        for _ in 0..count {
            let (index, result) = rx.recv().map_err(|err| {
                MosaicError::Unknown(format!("failed to receive mcp diagnose result: {err}"))
            })?;
            if index < ordered.len() {
                ordered[index] = Some(result);
            }
        }

        let mut results = Vec::with_capacity(count);
        for entry in ordered {
            let Some(result) = entry else {
                return Err(MosaicError::Unknown(
                    "mcp diagnose batch returned incomplete results".to_string(),
                ));
            };
            results.push(result);
        }
        Ok(results)
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

fn evaluate_server_check(server: &McpServer) -> McpServerCheck {
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

    McpServerCheck {
        server_id: server.id.clone(),
        checked_at,
        healthy: issues.is_empty(),
        enabled: server.enabled,
        executable_resolved: resolved.map(|path| path.display().to_string()),
        cwd_exists,
        issues,
    }
}

fn apply_server_check(server: &mut McpServer, check: &McpServerCheck) {
    server.last_check_at = Some(check.checked_at);
    server.last_check_error = if check.healthy {
        None
    } else {
        Some(check.issues.join("; "))
    };
    server.updated_at = check.checked_at;
}

fn run_stdio_initialize_probe(server: &McpServer, timeout_ms: u64) -> McpProtocolProbe {
    let timeout_ms = timeout_ms.max(100);
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    let mut command = Command::new(&server.command);
    command.args(&server.args);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(cwd) = server.cwd.as_deref() {
        command.current_dir(cwd);
    }
    if !server.env.is_empty() {
        command.envs(&server.env);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return McpProtocolProbe {
                attempted: true,
                timeout_ms,
                duration_ms: started.elapsed().as_millis(),
                handshake_ok: false,
                response_kind: None,
                response_preview: None,
                stderr_preview: None,
                error: Some(format!("failed to spawn server process: {err}")),
            };
        }
    };

    let mut stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return McpProtocolProbe {
                attempted: true,
                timeout_ms,
                duration_ms: started.elapsed().as_millis(),
                handshake_ok: false,
                response_kind: None,
                response_preview: None,
                stderr_preview: None,
                error: Some("failed to open server stdin for protocol probe".to_string()),
            };
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return McpProtocolProbe {
                attempted: true,
                timeout_ms,
                duration_ms: started.elapsed().as_millis(),
                handshake_ok: false,
                response_kind: None,
                response_preview: None,
                stderr_preview: None,
                error: Some("failed to open server stdout for protocol probe".to_string()),
            };
        }
    };

    let (stdout_tx, stdout_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if stdout_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let (stderr_tx, stderr_rx) = mpsc::channel::<Option<String>>();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let mut data = Vec::new();
            let mut limited = stderr.take(4_096);
            let _ = limited.read_to_end(&mut data);
            let text = String::from_utf8_lossy(&data).trim().to_string();
            let _ = stderr_tx.send((!text.is_empty()).then_some(text));
        });
    } else {
        let _ = stderr_tx.send(None);
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {
                "name": "mosaic-cli",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {}
        }
    });
    let request_body = match serde_json::to_string(&request) {
        Ok(payload) => payload,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return McpProtocolProbe {
                attempted: true,
                timeout_ms,
                duration_ms: started.elapsed().as_millis(),
                handshake_ok: false,
                response_kind: None,
                response_preview: None,
                stderr_preview: stderr_rx
                    .recv_timeout(Duration::from_millis(50))
                    .ok()
                    .flatten(),
                error: Some(format!("failed to encode initialize request: {err}")),
            };
        }
    };
    let framed = format!(
        "Content-Length: {}\r\n\r\n{}",
        request_body.len(),
        request_body
    );
    if let Err(err) = stdin
        .write_all(framed.as_bytes())
        .and_then(|_| stdin.flush())
    {
        let _ = child.kill();
        let _ = child.wait();
        return McpProtocolProbe {
            attempted: true,
            timeout_ms,
            duration_ms: started.elapsed().as_millis(),
            handshake_ok: false,
            response_kind: None,
            response_preview: None,
            stderr_preview: stderr_rx
                .recv_timeout(Duration::from_millis(50))
                .ok()
                .flatten(),
            error: Some(format!("failed to write initialize request: {err}")),
        };
    }
    drop(stdin);

    let deadline = Instant::now() + timeout;
    let mut response_kind = None;
    let mut response_preview = None;
    let mut probe_error = None;
    let mut handshake_ok = false;

    loop {
        let now = Instant::now();
        if now >= deadline {
            probe_error = Some("timed out waiting for initialize response".to_string());
            break;
        }

        match stdout_rx.recv_timeout(deadline.saturating_duration_since(now)) {
            Ok(line) => {
                if response_preview.is_none() {
                    response_preview = Some(trim_preview(&line, 512));
                }
                match classify_initialize_response_line(&line) {
                    InitializeResponse::Result => {
                        response_kind = Some("result".to_string());
                        handshake_ok = true;
                        break;
                    }
                    InitializeResponse::Error(message) => {
                        response_kind = Some("error".to_string());
                        probe_error = Some(message);
                        handshake_ok = false;
                        break;
                    }
                    InitializeResponse::Ignore => {}
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                probe_error = Some("timed out waiting for initialize response".to_string());
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                if probe_error.is_none() {
                    probe_error = Some("server stdout closed before initialize response".into());
                }
                break;
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let stderr_preview = stderr_rx
        .recv_timeout(Duration::from_millis(50))
        .ok()
        .flatten();
    McpProtocolProbe {
        attempted: true,
        timeout_ms,
        duration_ms: started.elapsed().as_millis(),
        handshake_ok,
        response_kind,
        response_preview,
        stderr_preview,
        error: probe_error,
    }
}

fn skipped_probe(timeout_ms: u64) -> McpProtocolProbe {
    McpProtocolProbe {
        attempted: false,
        timeout_ms,
        duration_ms: 0,
        handshake_ok: false,
        response_kind: None,
        response_preview: None,
        stderr_preview: None,
        error: Some("protocol probe skipped because executable/cwd precheck failed".into()),
    }
}

enum InitializeResponse {
    Ignore,
    Result,
    Error(String),
}

fn classify_initialize_response_line(line: &str) -> InitializeResponse {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return InitializeResponse::Ignore;
    }

    let value = match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => value,
        Err(_) => return InitializeResponse::Ignore,
    };

    let Some(object) = value.as_object() else {
        return InitializeResponse::Ignore;
    };
    if object.get("jsonrpc").and_then(|value| value.as_str()) != Some("2.0") {
        return InitializeResponse::Ignore;
    }
    if !matches_initialize_id(object.get("id")) {
        return InitializeResponse::Ignore;
    }

    if object.contains_key("result") {
        return InitializeResponse::Result;
    }
    if let Some(error) = object.get("error") {
        return InitializeResponse::Error(format!(
            "initialize returned error: {}",
            trim_preview(&error.to_string(), 256)
        ));
    }

    InitializeResponse::Ignore
}

fn matches_initialize_id(id: Option<&serde_json::Value>) -> bool {
    match id {
        Some(serde_json::Value::Number(number)) => number.as_i64() == Some(1),
        Some(serde_json::Value::String(value)) => value == "1",
        _ => false,
    }
}

fn trim_preview(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut preview = trimmed.chars().take(max_chars).collect::<String>();
    preview.push_str("...");
    preview
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

        let missing_cwd = temp.path().join("missing-cwd");
        let patched = store
            .set_cwd("local-mcp", Some(missing_cwd.display().to_string()))
            .expect("set cwd");
        assert_eq!(
            patched.cwd.as_deref(),
            Some(missing_cwd.to_string_lossy().as_ref())
        );
        let checked_missing_cwd = store.check("local-mcp").expect("check missing cwd");
        assert!(!checked_missing_cwd.check.healthy);
        assert!(
            checked_missing_cwd
                .check
                .issues
                .iter()
                .any(|issue| issue.contains("cwd"))
        );

        let cleared = store.set_cwd("local-mcp", None).expect("clear cwd");
        assert!(cleared.cwd.is_none());

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

    #[test]
    fn get_and_check_all_work() {
        let temp = tempdir().expect("tempdir");
        let path = mcp_servers_file_path(&temp.path().join("data"));
        let store = McpStore::new(path);

        store
            .add(AddMcpServerInput {
                id: Some("first".to_string()),
                name: "First".to_string(),
                command: std::env::current_exe()
                    .expect("current exe")
                    .display()
                    .to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: None,
                enabled: true,
            })
            .expect("add first");
        store
            .add(AddMcpServerInput {
                id: Some("second".to_string()),
                name: "Second".to_string(),
                command: "__missing_cmd__".to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: None,
                enabled: true,
            })
            .expect("add second");

        let got = store.get("first").expect("get").expect("exists");
        assert_eq!(got.id, "first");

        let checks = store.check_all().expect("check all");
        assert_eq!(checks.len(), 2);
        assert!(checks.iter().any(|item| item.server.id == "first"));
        assert!(checks.iter().any(|item| item.server.id == "second"));
        assert!(checks.iter().any(|item| item.check.healthy));
        assert!(checks.iter().any(|item| !item.check.healthy));
    }

    #[test]
    fn diagnose_skips_protocol_probe_when_precheck_fails() {
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

        let diagnosed = store
            .diagnose("missing", McpDiagnoseOptions { timeout_ms: 500 })
            .expect("diagnose");
        assert!(!diagnosed.healthy);
        assert!(!diagnosed.protocol_probe.attempted);
        assert!(
            diagnosed
                .protocol_probe
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("precheck failed")
        );
    }

    #[cfg(unix)]
    #[test]
    fn diagnose_protocol_probe_success_with_mock_script() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = mcp_servers_file_path(&temp.path().join("data"));
        let store = McpStore::new(path);

        let script_path = temp.path().join("mock-mcp.sh");
        std::fs::write(
            &script_path,
            "#!/bin/sh\nprintf '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{}}}\\n'\nsleep 1\n",
        )
        .expect("write script");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).expect("set permissions");

        store
            .add(AddMcpServerInput {
                id: Some("mock".to_string()),
                name: "Mock".to_string(),
                command: script_path.display().to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: Some(temp.path().display().to_string()),
                enabled: true,
            })
            .expect("add");

        let diagnosed = store
            .diagnose("mock", McpDiagnoseOptions { timeout_ms: 3_000 })
            .expect("diagnose");
        assert!(diagnosed.check.healthy);
        assert!(diagnosed.protocol_probe.attempted);
        assert!(
            diagnosed.protocol_probe.handshake_ok,
            "probe={:?}",
            diagnosed.protocol_probe
        );
        assert_eq!(
            diagnosed.protocol_probe.response_kind.as_deref(),
            Some("result")
        );
        assert!(diagnosed.healthy);
    }

    #[cfg(unix)]
    #[test]
    fn diagnose_protocol_probe_timeout_for_silent_script() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir().expect("tempdir");
        let path = mcp_servers_file_path(&temp.path().join("data"));
        let store = McpStore::new(path);

        let script_path = temp.path().join("silent-mcp.sh");
        std::fs::write(&script_path, "#!/bin/sh\nsleep 2\n").expect("write script");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).expect("set permissions");

        store
            .add(AddMcpServerInput {
                id: Some("silent".to_string()),
                name: "Silent".to_string(),
                command: script_path.display().to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: Some(temp.path().display().to_string()),
                enabled: true,
            })
            .expect("add");

        let diagnosed = store
            .diagnose("silent", McpDiagnoseOptions { timeout_ms: 200 })
            .expect("diagnose");
        assert!(!diagnosed.protocol_probe.handshake_ok);
        assert!(
            diagnosed
                .protocol_probe
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("timed out")
        );
        assert!(!diagnosed.healthy);
    }

    #[test]
    fn diagnose_all_returns_batch_results() {
        let temp = tempdir().expect("tempdir");
        let path = mcp_servers_file_path(&temp.path().join("data"));
        let store = McpStore::new(path);

        store
            .add(AddMcpServerInput {
                id: Some("ok".to_string()),
                name: "Ok".to_string(),
                command: std::env::current_exe()
                    .expect("current exe")
                    .display()
                    .to_string(),
                args: vec!["--version".to_string()],
                env: BTreeMap::new(),
                cwd: Some(temp.path().display().to_string()),
                enabled: true,
            })
            .expect("add ok");
        store
            .add(AddMcpServerInput {
                id: Some("missing".to_string()),
                name: "Missing".to_string(),
                command: "__missing_command__".to_string(),
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: Some(temp.path().display().to_string()),
                enabled: true,
            })
            .expect("add missing");

        let results = store
            .diagnose_all(McpDiagnoseOptions { timeout_ms: 300 })
            .expect("diagnose all");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|item| item.server.id == "ok"));
        assert!(results.iter().any(|item| item.server.id == "missing"));
        assert!(
            results
                .iter()
                .any(|item| item.server.id == "missing" && !item.protocol_probe.attempted)
        );
    }
}
