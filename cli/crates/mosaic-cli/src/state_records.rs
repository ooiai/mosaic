use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use chrono::Utc;

use crate::utils::{load_json_file_opt, save_state_json_file};

use super::{
    BROWSER_SEQ, BrowserRuntimeState, BrowserVisitRecord, CRON_SEQ, CronJobRecord, DeviceRecord,
    HOOK_SEQ, HookRecord, NodeRecord, NodeRuntimeStatus, PAIRING_REQUEST_SEQ, PairingRequestRecord,
    Result, WEBHOOK_SEQ, WebhookRecord,
};

pub(super) fn nodes_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("nodes.json")
}

pub(super) fn devices_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("devices.json")
}

pub(super) fn pairing_requests_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("pairing-requests.json")
}

pub(super) fn hooks_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("hooks.json")
}

pub(super) fn hook_events_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("hook-events")
}

pub(super) fn hook_events_file_path(data_dir: &Path, hook_id: &str) -> PathBuf {
    hook_events_dir(data_dir).join(format!("{hook_id}.jsonl"))
}

pub(super) fn webhooks_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("webhooks.json")
}

pub(super) fn webhook_events_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("webhook-events")
}

pub(super) fn webhook_events_file_path(data_dir: &Path, webhook_id: &str) -> PathBuf {
    webhook_events_dir(data_dir).join(format!("{webhook_id}.jsonl"))
}

pub(super) fn browser_history_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("browser-history.json")
}

pub(super) fn browser_state_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("browser-state.json")
}

pub(super) fn cron_jobs_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cron-jobs.json")
}

pub(super) fn cron_events_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cron-events")
}

pub(super) fn cron_events_file_path(data_dir: &Path, job_id: &str) -> PathBuf {
    cron_events_dir(data_dir).join(format!("{job_id}.jsonl"))
}

pub(super) fn load_nodes_or_default(path: &Path) -> Result<Vec<NodeRecord>> {
    let nodes =
        load_json_file_opt::<Vec<NodeRecord>>(path)?.unwrap_or_else(|| vec![default_local_node()]);
    if nodes.is_empty() {
        return Ok(vec![default_local_node()]);
    }
    Ok(nodes)
}

pub(super) fn save_nodes(path: &Path, nodes: &[NodeRecord]) -> Result<()> {
    save_state_json_file(path, &nodes.to_vec(), "nodes state")
}

pub(super) fn load_devices_or_default(path: &Path) -> Result<Vec<DeviceRecord>> {
    Ok(load_json_file_opt::<Vec<DeviceRecord>>(path)?.unwrap_or_default())
}

pub(super) fn save_devices(path: &Path, devices: &[DeviceRecord]) -> Result<()> {
    save_state_json_file(path, &devices.to_vec(), "devices state")
}

pub(super) fn load_pairing_requests_or_default(path: &Path) -> Result<Vec<PairingRequestRecord>> {
    Ok(load_json_file_opt::<Vec<PairingRequestRecord>>(path)?.unwrap_or_default())
}

pub(super) fn save_pairing_requests(path: &Path, requests: &[PairingRequestRecord]) -> Result<()> {
    save_state_json_file(path, &requests.to_vec(), "pairing requests state")
}

pub(super) fn load_hooks_or_default(path: &Path) -> Result<Vec<HookRecord>> {
    Ok(load_json_file_opt::<Vec<HookRecord>>(path)?.unwrap_or_default())
}

pub(super) fn save_hooks(path: &Path, hooks: &[HookRecord]) -> Result<()> {
    save_state_json_file(path, &hooks.to_vec(), "hooks state")
}

pub(super) fn load_webhooks_or_default(path: &Path) -> Result<Vec<WebhookRecord>> {
    Ok(load_json_file_opt::<Vec<WebhookRecord>>(path)?.unwrap_or_default())
}

pub(super) fn save_webhooks(path: &Path, webhooks: &[WebhookRecord]) -> Result<()> {
    save_state_json_file(path, &webhooks.to_vec(), "webhooks state")
}

pub(super) fn load_browser_history_or_default(path: &Path) -> Result<Vec<BrowserVisitRecord>> {
    Ok(load_json_file_opt::<Vec<BrowserVisitRecord>>(path)?.unwrap_or_default())
}

pub(super) fn save_browser_history(path: &Path, visits: &[BrowserVisitRecord]) -> Result<()> {
    save_state_json_file(path, &visits.to_vec(), "browser history state")
}

pub(super) fn load_browser_state_or_default(path: &Path) -> Result<BrowserRuntimeState> {
    Ok(load_json_file_opt::<BrowserRuntimeState>(path)?.unwrap_or_else(default_browser_state))
}

pub(super) fn save_browser_state(path: &Path, state: &BrowserRuntimeState) -> Result<()> {
    save_state_json_file(path, state, "browser runtime state")
}

pub(super) fn load_cron_jobs_or_default(path: &Path) -> Result<Vec<CronJobRecord>> {
    Ok(load_json_file_opt::<Vec<CronJobRecord>>(path)?.unwrap_or_default())
}

pub(super) fn save_cron_jobs(path: &Path, jobs: &[CronJobRecord]) -> Result<()> {
    save_state_json_file(path, &jobs.to_vec(), "cron jobs state")
}

fn default_local_node() -> NodeRecord {
    let now = Utc::now();
    NodeRecord {
        id: "local".to_string(),
        name: "Local Node".to_string(),
        status: NodeRuntimeStatus::Online,
        capabilities: vec![
            "invoke".to_string(),
            "run".to_string(),
            "status".to_string(),
        ],
        last_seen_at: now,
        updated_at: now,
    }
}

fn default_browser_state() -> BrowserRuntimeState {
    BrowserRuntimeState {
        running: false,
        started_at: None,
        stopped_at: None,
        active_visit_id: None,
    }
}

pub(super) fn generate_pairing_request_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("pr-{ts}-{}", next_pairing_seq())
}

pub(super) fn generate_hook_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("hk-{ts}-{}", next_hook_seq())
}

pub(super) fn generate_cron_job_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("cj-{ts}-{}", next_cron_seq())
}

pub(super) fn generate_webhook_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("wh-{ts}-{}", next_webhook_seq())
}

pub(super) fn generate_browser_visit_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("bv-{ts}-{}", next_browser_seq())
}

pub(super) fn next_pairing_seq() -> u64 {
    PAIRING_REQUEST_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn next_hook_seq() -> u64 {
    HOOK_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn next_cron_seq() -> u64 {
    CRON_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn next_webhook_seq() -> u64 {
    WEBHOOK_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn next_browser_seq() -> u64 {
    BROWSER_SEQ.fetch_add(1, Ordering::Relaxed)
}
