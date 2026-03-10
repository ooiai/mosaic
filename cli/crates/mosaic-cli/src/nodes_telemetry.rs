use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use mosaic_core::privacy::append_sanitized_jsonl;

pub(super) fn nodes_events_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("nodes-events.jsonl")
}

#[derive(Debug, Clone)]
pub(super) struct NodeTelemetryEventInput {
    pub scope: &'static str,
    pub action: &'static str,
    pub target_type: &'static str,
    pub target_id: String,
    pub success: bool,
    pub detail: String,
    pub node_id: Option<String>,
    pub device_id: Option<String>,
    pub pairing_id: Option<String>,
    pub repair: Option<bool>,
    pub issues_total: Option<usize>,
    pub actions_applied: Option<usize>,
}

#[derive(Debug, Serialize)]
struct NodeTelemetryEventRecord {
    id: String,
    ts: String,
    scope: String,
    action: String,
    target_type: String,
    target_id: String,
    success: bool,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pairing_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repair: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issues_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions_applied: Option<usize>,
}

pub(super) fn write_nodes_event(path: &Path, event: NodeTelemetryEventInput) {
    let record = NodeTelemetryEventRecord {
        id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        scope: event.scope.to_string(),
        action: event.action.to_string(),
        target_type: event.target_type.to_string(),
        target_id: event.target_id,
        success: event.success,
        detail: event.detail,
        node_id: event.node_id,
        device_id: event.device_id,
        pairing_id: event.pairing_id,
        repair: event.repair,
        issues_total: event.issues_total,
        actions_applied: event.actions_applied,
    };
    let _ = append_sanitized_jsonl(path, &record, "nodes telemetry event");
}
