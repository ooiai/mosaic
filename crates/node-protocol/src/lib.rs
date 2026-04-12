use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mosaic_tool_core::{
    CapabilityAudit, CapabilityKind, PermissionScope, ToolResult, ToolRiskLevel,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_STALE_AFTER_SECS: i64 = 15;
pub const DEFAULT_AFFINITY_KEY: &str = "__default__";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeHealth {
    Online,
    Stale,
    Offline,
}

impl NodeHealth {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Stale => "stale",
            Self::Offline => "offline",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCapabilityDeclaration {
    pub name: String,
    pub kind: CapabilityKind,
    #[serde(default)]
    pub permission_scopes: Vec<PermissionScope>,
    pub risk: ToolRiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRegistration {
    pub node_id: String,
    pub label: String,
    pub transport: String,
    pub platform: String,
    #[serde(default)]
    pub capabilities: Vec<NodeCapabilityDeclaration>,
    pub registered_at: DateTime<Utc>,
    pub last_heartbeat_at: DateTime<Utc>,
    #[serde(default = "default_true")]
    pub online: bool,
    pub last_disconnect_reason: Option<String>,
}

impl NodeRegistration {
    pub fn new(
        node_id: impl Into<String>,
        label: impl Into<String>,
        transport: impl Into<String>,
        platform: impl Into<String>,
        capabilities: Vec<NodeCapabilityDeclaration>,
    ) -> Self {
        let now = Utc::now();
        Self {
            node_id: node_id.into(),
            label: label.into(),
            transport: transport.into(),
            platform: platform.into(),
            capabilities,
            registered_at: now,
            last_heartbeat_at: now,
            online: true,
            last_disconnect_reason: None,
        }
    }

    pub fn heartbeat(&mut self) {
        self.last_heartbeat_at = Utc::now();
        self.online = true;
        self.last_disconnect_reason = None;
    }

    pub fn disconnect(&mut self, reason: impl Into<String>) {
        self.online = false;
        self.last_disconnect_reason = Some(reason.into());
    }

    pub fn health(&self, now: DateTime<Utc>, stale_after_secs: i64) -> NodeHealth {
        if !self.online {
            return NodeHealth::Offline;
        }

        if now
            .signed_duration_since(self.last_heartbeat_at)
            .num_seconds()
            > stale_after_secs
        {
            NodeHealth::Stale
        } else {
            NodeHealth::Online
        }
    }

    pub fn supports_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|decl| decl.name == capability)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeCommandDispatch {
    pub command_id: String,
    pub node_id: String,
    pub session_id: Option<String>,
    pub capability: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub dispatched_at: DateTime<Utc>,
}

impl NodeCommandDispatch {
    pub fn new(
        node_id: impl Into<String>,
        session_id: Option<String>,
        capability: impl Into<String>,
        tool_name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self {
            command_id: Uuid::new_v4().to_string(),
            node_id: node_id.into(),
            session_id,
            capability: capability.into(),
            tool_name: tool_name.into(),
            input,
            dispatched_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeCommandResultEnvelope {
    pub command_id: String,
    pub node_id: String,
    pub capability: String,
    pub tool_name: String,
    pub status: String,
    pub output: String,
    pub structured: Option<serde_json::Value>,
    pub error: Option<String>,
    pub disconnect_context: Option<String>,
    pub audit: Option<CapabilityAudit>,
    pub completed_at: DateTime<Utc>,
}

impl NodeCommandResultEnvelope {
    pub fn success(dispatch: &NodeCommandDispatch, result: ToolResult) -> Self {
        Self {
            command_id: dispatch.command_id.clone(),
            node_id: dispatch.node_id.clone(),
            capability: dispatch.capability.clone(),
            tool_name: dispatch.tool_name.clone(),
            status: if result.is_error {
                "failed".to_owned()
            } else {
                "success".to_owned()
            },
            output: result.content,
            structured: result.structured,
            error: None,
            disconnect_context: None,
            audit: result.audit,
            completed_at: Utc::now(),
        }
    }

    pub fn failure(
        dispatch: &NodeCommandDispatch,
        status: impl Into<String>,
        error: impl Into<String>,
        disconnect_context: Option<String>,
    ) -> Self {
        let error = error.into();
        Self {
            command_id: dispatch.command_id.clone(),
            node_id: dispatch.node_id.clone(),
            capability: dispatch.capability.clone(),
            tool_name: dispatch.tool_name.clone(),
            status: status.into(),
            output: error.clone(),
            structured: None,
            error: Some(error),
            disconnect_context,
            audit: None,
            completed_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeAffinityRecord {
    pub session_id: String,
    pub node_id: String,
    pub attached_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NodeSelection {
    pub registration: NodeRegistration,
    pub route: String,
}

#[derive(Debug, Clone)]
pub struct NodeToolExecutionRequest {
    pub session_id: Option<String>,
    pub tool_name: String,
    pub capability: String,
    pub input: serde_json::Value,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct NodeToolExecutionResult {
    pub node_id: String,
    pub route: String,
    pub disconnect_context: Option<String>,
    pub result: ToolResult,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeDispatchFailureClass {
    NoEligibleNode,
    Unavailable,
    Stale,
    Transport,
    PermissionDenied,
    RemoteExecutionFailed,
}

impl NodeDispatchFailureClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoEligibleNode => "no_eligible_node",
            Self::Unavailable => "node_unavailable",
            Self::Stale => "node_stale",
            Self::Transport => "node_transport_failed",
            Self::PermissionDenied => "node_permission_denied",
            Self::RemoteExecutionFailed => "node_remote_execution_failed",
        }
    }

    pub fn allows_local_fallback(self) -> bool {
        matches!(
            self,
            Self::NoEligibleNode | Self::Unavailable | Self::Stale | Self::Transport
        )
    }
}

#[derive(Debug, Clone)]
pub struct NodeToolExecutionError {
    pub node_id: Option<String>,
    pub route: Option<String>,
    pub disconnect_context: Option<String>,
    pub failure_class: NodeDispatchFailureClass,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum NodeToolDispatchOutcome {
    NotHandled,
    Completed(NodeToolExecutionResult),
    Failed(NodeToolExecutionError),
}

#[async_trait]
pub trait NodeRouter: Send + Sync {
    async fn dispatch(&self, request: NodeToolExecutionRequest) -> Result<NodeToolDispatchOutcome>;
}

#[derive(Debug, Clone)]
pub struct FileNodeStore {
    root: PathBuf,
    stale_after_secs: i64,
}

impl FileNodeStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::new_with_stale_after(root, DEFAULT_STALE_AFTER_SECS)
    }

    pub fn new_with_stale_after(root: impl Into<PathBuf>, stale_after_secs: i64) -> Self {
        Self {
            root: root.into(),
            stale_after_secs,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn register_node(&self, registration: &NodeRegistration) -> Result<()> {
        self.ensure_layout()?;
        self.write_json(&self.registry_path(&registration.node_id), registration)
    }

    pub fn heartbeat(&self, node_id: &str) -> Result<Option<NodeRegistration>> {
        let mut registration = match self.load_node(node_id)? {
            Some(registration) => registration,
            None => return Ok(None),
        };
        registration.heartbeat();
        self.register_node(&registration)?;
        Ok(Some(registration))
    }

    pub fn disconnect_node(&self, node_id: &str, reason: impl Into<String>) -> Result<()> {
        let mut registration = self
            .load_node(node_id)?
            .ok_or_else(|| anyhow!("node not found: {}", node_id))?;
        registration.disconnect(reason);
        self.register_node(&registration)
    }

    pub fn load_node(&self, node_id: &str) -> Result<Option<NodeRegistration>> {
        self.read_json_if_exists(&self.registry_path(node_id))
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeRegistration>> {
        self.ensure_layout()?;
        let mut nodes: Vec<NodeRegistration> = Vec::new();
        for entry in fs::read_dir(self.registry_dir())? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    nodes.push(read_json(&path)?);
                }
            }
        }
        nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        Ok(nodes)
    }

    pub fn attach_session(&self, session_id: &str, node_id: &str) -> Result<()> {
        self.ensure_layout()?;
        let record = NodeAffinityRecord {
            session_id: session_id.to_owned(),
            node_id: node_id.to_owned(),
            attached_at: Utc::now(),
        };
        self.write_affinity(session_id, &record)
    }

    pub fn attach_default(&self, node_id: &str) -> Result<()> {
        self.attach_session(DEFAULT_AFFINITY_KEY, node_id)
    }

    pub fn detach_session(&self, session_id: &str) -> Result<bool> {
        self.ensure_layout()?;
        let path = self.affinity_path(session_id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(path)?;
        Ok(true)
    }

    pub fn detach_default(&self) -> Result<bool> {
        self.detach_session(DEFAULT_AFFINITY_KEY)
    }

    pub fn list_affinities(&self) -> Result<Vec<NodeAffinityRecord>> {
        self.ensure_layout()?;
        let mut records: Vec<NodeAffinityRecord> = Vec::new();
        for entry in fs::read_dir(self.affinity_dir())? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    records.push(read_json(&path)?);
                }
            }
        }
        records.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(records)
    }

    pub fn affinity_for_session(
        &self,
        session_id: Option<&str>,
    ) -> Result<Option<NodeAffinityRecord>> {
        if let Some(session_id) = session_id {
            if let Some(record) = self.read_affinity(session_id)? {
                return Ok(Some(record));
            }
        }
        self.read_affinity(DEFAULT_AFFINITY_KEY)
    }

    pub fn prune_stale_nodes(&self) -> Result<Vec<NodeRegistration>> {
        self.ensure_layout()?;
        let now = Utc::now();
        let mut removed = Vec::new();
        for node in self.list_nodes()? {
            if node.health(now, self.stale_after_secs) == NodeHealth::Online {
                continue;
            }
            let path = self.registry_path(&node.node_id);
            if path.exists() {
                fs::remove_file(path)?;
            }
            removed.push(node);
        }
        Ok(removed)
    }

    pub fn node_capabilities(&self, node_id: &str) -> Result<Vec<NodeCapabilityDeclaration>> {
        Ok(self
            .load_node(node_id)?
            .ok_or_else(|| anyhow!("node not found: {}", node_id))?
            .capabilities)
    }

    pub fn select_node(
        &self,
        capability: &str,
        session_id: Option<&str>,
    ) -> Result<Option<NodeSelection>> {
        let now = Utc::now();
        let nodes = self.list_nodes()?;

        if let Some(affinity) = self.affinity_for_session(session_id)? {
            if let Some(node) = nodes.iter().find(|node| node.node_id == affinity.node_id) {
                if !node.supports_capability(capability) {
                    bail!(
                        "node permission denied: node '{}' does not declare capability '{}'",
                        node.node_id,
                        capability,
                    );
                }
                return Ok(Some(NodeSelection {
                    registration: node.clone(),
                    route: if affinity.session_id == DEFAULT_AFFINITY_KEY {
                        "default_affinity".to_owned()
                    } else {
                        "session_affinity".to_owned()
                    },
                }));
            }
        }

        if let Some(node) = nodes.iter().find(|node| {
            node.supports_capability(capability)
                && node.health(now, self.stale_after_secs) == NodeHealth::Online
        }) {
            return Ok(Some(NodeSelection {
                registration: node.clone(),
                route: "capability_match".to_owned(),
            }));
        }

        Ok(nodes
            .into_iter()
            .find(|node| node.supports_capability(capability))
            .map(|registration| NodeSelection {
                registration,
                route: "capability_match".to_owned(),
            }))
    }

    pub fn dispatch_command(&self, dispatch: &NodeCommandDispatch) -> Result<()> {
        self.ensure_layout()?;
        let node_dir = self.command_node_dir(&dispatch.node_id);
        fs::create_dir_all(&node_dir)?;
        self.write_json(
            &node_dir.join(format!("{}.json", dispatch.command_id)),
            dispatch,
        )
    }

    pub fn pending_commands(&self, node_id: &str) -> Result<Vec<NodeCommandDispatch>> {
        self.ensure_layout()?;
        let node_dir = self.command_node_dir(node_id);
        if !node_dir.exists() {
            return Ok(Vec::new());
        }
        let mut commands: Vec<NodeCommandDispatch> = Vec::new();
        for entry in fs::read_dir(node_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                commands.push(read_json(&entry.path())?);
            }
        }
        commands.sort_by(|left, right| left.dispatched_at.cmp(&right.dispatched_at));
        Ok(commands)
    }

    pub fn complete_command(&self, result: &NodeCommandResultEnvelope) -> Result<()> {
        self.ensure_layout()?;
        self.write_json(&self.result_path(&result.command_id), result)?;
        let command_path = self
            .command_node_dir(&result.node_id)
            .join(format!("{}.json", result.command_id));
        if command_path.exists() {
            fs::remove_file(command_path)?;
        }
        Ok(())
    }

    pub fn take_result(&self, command_id: &str) -> Result<Option<NodeCommandResultEnvelope>> {
        let path = self.result_path(command_id);
        let result = self.read_json_if_exists(&path)?;
        if result.is_some() {
            fs::remove_file(path)?;
        }
        Ok(result)
    }

    pub async fn await_result(
        &self,
        command_id: &str,
        timeout: Duration,
    ) -> Result<NodeCommandResultEnvelope> {
        let started = std::time::Instant::now();
        loop {
            if let Some(result) = self.take_result(command_id)? {
                return Ok(result);
            }

            if started.elapsed() >= timeout {
                bail!("timed out waiting for node result: {}", command_id);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    fn registry_dir(&self) -> PathBuf {
        self.root.join("registry")
    }

    fn commands_dir(&self) -> PathBuf {
        self.root.join("commands")
    }

    fn command_node_dir(&self, node_id: &str) -> PathBuf {
        self.commands_dir().join(node_id)
    }

    fn results_dir(&self) -> PathBuf {
        self.root.join("results")
    }

    fn affinity_dir(&self) -> PathBuf {
        self.root.join("affinity")
    }

    fn registry_path(&self, node_id: &str) -> PathBuf {
        self.registry_dir().join(format!("{}.json", node_id))
    }

    fn result_path(&self, command_id: &str) -> PathBuf {
        self.results_dir().join(format!("{}.json", command_id))
    }

    fn affinity_path(&self, session_id: &str) -> PathBuf {
        self.affinity_dir().join(format!("{}.json", session_id))
    }

    fn ensure_layout(&self) -> Result<()> {
        fs::create_dir_all(self.registry_dir())?;
        fs::create_dir_all(self.commands_dir())?;
        fs::create_dir_all(self.results_dir())?;
        fs::create_dir_all(self.affinity_dir())?;
        Ok(())
    }

    fn read_affinity(&self, session_id: &str) -> Result<Option<NodeAffinityRecord>> {
        self.read_json_if_exists(&self.affinity_path(session_id))
    }

    fn write_affinity(&self, session_id: &str, record: &NodeAffinityRecord) -> Result<()> {
        self.write_json(&self.affinity_path(session_id), record)
    }

    fn read_json_if_exists<T>(&self, path: &Path) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(read_json(path)?))
    }

    fn write_json<T>(&self, path: &Path, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(value)?)?;
        Ok(())
    }
}

#[async_trait]
impl NodeRouter for FileNodeStore {
    async fn dispatch(&self, request: NodeToolExecutionRequest) -> Result<NodeToolDispatchOutcome> {
        let selection = match self.select_node(&request.capability, request.session_id.as_deref()) {
            Ok(Some(selection)) => selection,
            Ok(None) => return Ok(NodeToolDispatchOutcome::NotHandled),
            Err(err) => {
                return Ok(NodeToolDispatchOutcome::Failed(NodeToolExecutionError {
                    node_id: None,
                    route: None,
                    disconnect_context: None,
                    failure_class: NodeDispatchFailureClass::PermissionDenied,
                    message: err.to_string(),
                }));
            }
        };
        let health = selection
            .registration
            .health(Utc::now(), self.stale_after_secs);
        match health {
            NodeHealth::Offline => {
                return Ok(NodeToolDispatchOutcome::Failed(NodeToolExecutionError {
                    node_id: Some(selection.registration.node_id.clone()),
                    route: Some(selection.route.clone()),
                    disconnect_context: selection.registration.last_disconnect_reason.clone(),
                    failure_class: NodeDispatchFailureClass::Unavailable,
                    message: format!("node unavailable: {}", selection.registration.node_id),
                }));
            }
            NodeHealth::Stale => {
                return Ok(NodeToolDispatchOutcome::Failed(NodeToolExecutionError {
                    node_id: Some(selection.registration.node_id.clone()),
                    route: Some(selection.route.clone()),
                    disconnect_context: selection.registration.last_disconnect_reason.clone(),
                    failure_class: NodeDispatchFailureClass::Stale,
                    message: format!("node stale: {}", selection.registration.node_id),
                }));
            }
            NodeHealth::Online => {}
        }

        let dispatch = NodeCommandDispatch::new(
            selection.registration.node_id.clone(),
            request.session_id,
            request.capability,
            request.tool_name,
            request.input,
        );
        self.dispatch_command(&dispatch)?;
        let result = match self
            .await_result(&dispatch.command_id, request.timeout)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                return Ok(NodeToolDispatchOutcome::Failed(NodeToolExecutionError {
                    node_id: Some(selection.registration.node_id.clone()),
                    route: Some(selection.route),
                    disconnect_context: selection.registration.last_disconnect_reason.clone(),
                    failure_class: NodeDispatchFailureClass::Transport,
                    message: err.to_string(),
                }));
            }
        };

        if result.status != "success" {
            let error = result
                .error
                .clone()
                .unwrap_or_else(|| result.output.clone());
            return Ok(NodeToolDispatchOutcome::Failed(NodeToolExecutionError {
                node_id: Some(result.node_id),
                route: Some(selection.route),
                disconnect_context: result.disconnect_context,
                failure_class: NodeDispatchFailureClass::RemoteExecutionFailed,
                message: error,
            }));
        }

        Ok(NodeToolDispatchOutcome::Completed(
            NodeToolExecutionResult {
                node_id: result.node_id,
                route: selection.route,
                disconnect_context: result.disconnect_context,
                result: ToolResult {
                    content: result.output,
                    structured: result.structured,
                    is_error: false,
                    audit: result.audit,
                },
            },
        ))
    }
}

fn default_true() -> bool {
    true
}

fn read_json<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> FileNodeStore {
        FileNodeStore::new(std::env::temp_dir().join(format!(
            "mosaic-node-protocol-tests-{}-{}",
            name,
            Uuid::new_v4()
        )))
    }

    fn exec_capability() -> NodeCapabilityDeclaration {
        NodeCapabilityDeclaration {
            name: "exec_command".to_owned(),
            kind: CapabilityKind::Exec,
            permission_scopes: vec![PermissionScope::LocalExec],
            risk: ToolRiskLevel::High,
        }
    }

    #[test]
    fn register_heartbeat_and_disconnect_roundtrip() {
        let store = temp_store("register");
        let registration = NodeRegistration::new(
            "node-a",
            "Headless Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        store
            .register_node(&registration)
            .expect("registration should persist");

        let listed = store.list_nodes().expect("nodes should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].node_id, "node-a");
        assert!(listed[0].supports_capability("exec_command"));

        let heartbeat = store
            .heartbeat("node-a")
            .expect("heartbeat should persist")
            .expect("node should exist");
        assert!(heartbeat.online);

        store
            .disconnect_node("node-a", "shutdown")
            .expect("disconnect should persist");
        let disconnected = store
            .load_node("node-a")
            .expect("node should load")
            .expect("node should exist");
        assert_eq!(
            disconnected.health(Utc::now(), DEFAULT_STALE_AFTER_SECS),
            NodeHealth::Offline
        );
        assert_eq!(
            disconnected.last_disconnect_reason.as_deref(),
            Some("shutdown")
        );
    }

    #[tokio::test]
    async fn dispatch_result_and_take_result_roundtrip() {
        let store = temp_store("dispatch");
        let registration = NodeRegistration::new(
            "node-a",
            "Headless Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        store
            .register_node(&registration)
            .expect("registration should persist");
        let dispatch = NodeCommandDispatch::new(
            "node-a",
            Some("demo".to_owned()),
            "exec_command",
            "exec_command",
            serde_json::json!({ "command": "/bin/echo", "args": ["hello"] }),
        );
        store
            .dispatch_command(&dispatch)
            .expect("command should persist");
        let pending = store
            .pending_commands("node-a")
            .expect("pending commands should load");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].command_id, dispatch.command_id);

        store
            .complete_command(&NodeCommandResultEnvelope::success(
                &dispatch,
                ToolResult::ok("hello"),
            ))
            .expect("result should persist");
        let result = store
            .await_result(&dispatch.command_id, Duration::from_secs(1))
            .await
            .expect("result should arrive");
        assert_eq!(result.status, "success");
        assert_eq!(result.output, "hello");
        assert!(
            store
                .pending_commands("node-a")
                .expect("pending commands should load")
                .is_empty()
        );
    }

    #[test]
    fn affinity_and_selection_prefer_attached_node() {
        let store = temp_store("affinity");
        let mut stale = NodeRegistration::new(
            "node-stale",
            "Stale Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        stale.last_heartbeat_at = Utc::now() - chrono::Duration::seconds(60);
        let fresh = NodeRegistration::new(
            "node-fresh",
            "Fresh Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        store
            .register_node(&stale)
            .expect("stale node should persist");
        store
            .register_node(&fresh)
            .expect("fresh node should persist");
        store
            .attach_session("demo", "node-stale")
            .expect("affinity should persist");

        let selection = store
            .select_node("exec_command", Some("demo"))
            .expect("selection should succeed")
            .expect("selection should exist");
        assert_eq!(selection.registration.node_id, "node-stale");
        assert_eq!(selection.route, "session_affinity");
        assert_eq!(
            selection
                .registration
                .health(Utc::now(), DEFAULT_STALE_AFTER_SECS),
            NodeHealth::Stale
        );
    }

    #[test]
    fn detach_and_list_affinities_roundtrip() {
        let store = temp_store("detach");
        let registration = NodeRegistration::new(
            "node-a",
            "Headless Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        store
            .register_node(&registration)
            .expect("registration should persist");
        store
            .attach_session("demo", "node-a")
            .expect("session affinity should persist");
        store
            .attach_default("node-a")
            .expect("default affinity should persist");

        let affinities = store.list_affinities().expect("affinities should list");
        assert_eq!(affinities.len(), 2);
        assert!(
            affinities
                .iter()
                .any(|record| record.session_id == DEFAULT_AFFINITY_KEY)
        );
        assert!(
            store
                .detach_session("demo")
                .expect("session affinity should detach")
        );
        assert!(
            store
                .detach_default()
                .expect("default affinity should detach")
        );
        assert!(
            !store
                .detach_session("missing")
                .expect("missing affinity should return false")
        );
        assert!(
            store
                .list_affinities()
                .expect("affinities should reload")
                .is_empty()
        );
    }

    #[test]
    fn prune_stale_nodes_removes_offline_and_stale_registrations() {
        let store = temp_store("prune");
        let online = NodeRegistration::new(
            "node-online",
            "Online Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        let mut stale = NodeRegistration::new(
            "node-stale",
            "Stale Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        stale.last_heartbeat_at = Utc::now() - chrono::Duration::seconds(60);
        let mut offline = NodeRegistration::new(
            "node-offline",
            "Offline Node",
            "file",
            "headless",
            vec![exec_capability()],
        );
        offline.disconnect("operator_shutdown");

        store.register_node(&online).expect("online should persist");
        store.register_node(&stale).expect("stale should persist");
        store
            .register_node(&offline)
            .expect("offline should persist");

        let removed = store.prune_stale_nodes().expect("prune should succeed");
        assert_eq!(removed.len(), 2);
        assert!(removed.iter().any(|node| node.node_id == "node-stale"));
        assert!(removed.iter().any(|node| node.node_id == "node-offline"));

        let remaining = store.list_nodes().expect("nodes should reload");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].node_id, "node-online");
    }
}
