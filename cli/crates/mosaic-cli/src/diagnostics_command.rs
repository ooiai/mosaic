use std::collections::BTreeMap;

use serde_json::{Value, json};

use mosaic_agents::{AgentStore, agent_routes_path, agents_file_path};
use mosaic_channels::{ChannelRepository, channels_events_dir, channels_file_path};
use mosaic_core::config::ConfigManager;
use mosaic_core::error::Result;
use mosaic_core::provider::Provider;
use mosaic_core::session::SessionStore;
use mosaic_memory::{MemoryStore, memory_index_path, memory_status_path};
use mosaic_ops::{ApprovalStore, SandboxStore};
use mosaic_plugins::{ExtensionRegistry, RegistryRoots};
use mosaic_provider_openai::OpenAiCompatibleProvider;
use mosaic_security::{SecurityAuditOptions, SecurityAuditor, SecurityBaselineConfig};

use super::{Cli, binary_in_path, print_json, resolve_state_paths};

pub(super) fn run_check(
    name: impl Into<String>,
    ok: bool,
    detail: impl Into<String>,
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    map.insert("name".to_string(), Value::String(name.into()));
    map.insert(
        "status".to_string(),
        Value::String(if ok { "ok" } else { "warn" }.to_string()),
    );
    map.insert("detail".to_string(), Value::String(detail.into()));
    map
}

pub(super) fn emit_checks(
    json_mode: bool,
    kind: &str,
    checks: Vec<BTreeMap<String, Value>>,
) -> Result<()> {
    if json_mode {
        print_json(&json!({
            "ok": true,
            "type": kind,
            "checks": checks,
        }));
    } else {
        println!("{kind}:");
        for check in checks {
            let status = check
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("warn")
                .to_uppercase();
            let name = check.get("name").and_then(Value::as_str).unwrap_or("-");
            let detail = check.get("detail").and_then(Value::as_str).unwrap_or("-");
            println!("[{status}] {name}: {detail}");
        }
    }
    Ok(())
}

pub(super) fn handle_status(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let store = SessionStore::new(paths.sessions_dir.clone());
    let agent_store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    let latest_session = store.latest_session_id()?;
    let (agents_count, default_agent_id) = match (agent_store.list(), agent_store.load_routes()) {
        (Ok(agents), Ok(routes)) => (agents.len(), routes.default_agent_id),
        _ => (0, None),
    };
    if !manager.exists() {
        if cli.json {
            print_json(&json!({
                "ok": true,
                "configured": false,
                "state_mode": paths.mode,
                "config_path": manager.path().display().to_string(),
                "latest_session": latest_session,
                "agents_count": agents_count,
                "default_agent_id": default_agent_id,
            }));
        } else {
            println!("configured: no");
            println!("config path: {}", manager.path().display());
            println!("state mode: {:?}", paths.mode);
            println!("agents: {}", agents_count);
        }
        return Ok(());
    }

    let config = manager.load()?;
    let resolved = config.resolve_profile(Some(&cli.profile))?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "configured": true,
            "profile": resolved.profile_name,
            "provider": resolved.profile.provider,
            "tools": resolved.profile.tools,
            "state_mode": paths.mode,
            "config_path": manager.path().display().to_string(),
            "latest_session": latest_session,
            "agents_count": agents_count,
            "default_agent_id": default_agent_id,
        }));
    } else {
        println!("configured: yes");
        println!("profile: {}", resolved.profile_name);
        println!("provider: {:?}", resolved.profile.provider.kind);
        println!("base url: {}", resolved.profile.provider.base_url);
        println!("model: {}", resolved.profile.provider.model);
        println!("state mode: {:?}", paths.mode);
        println!("agents: {}", agents_count);
        if let Some(default_agent_id) = default_agent_id {
            println!("default agent: {default_agent_id}");
        }
        if let Some(latest) = latest_session {
            println!("latest session: {latest}");
        }
    }
    Ok(())
}

pub(super) async fn handle_health(cli: &Cli) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut checks = vec![];
    checks.push(run_check(
        "state_dirs",
        paths.ensure_dirs().is_ok(),
        "state paths ready",
    ));
    checks.push(run_check(
        "state_writable",
        paths.is_writable().is_ok(),
        "state paths writable",
    ));

    if manager.exists() {
        let config = manager.load()?;
        checks.push(run_check("config", true, "config valid"));
        let resolved = config.resolve_profile(Some(&cli.profile))?;
        let provider = OpenAiCompatibleProvider::from_profile(&resolved.profile)?;
        let health = provider.health().await?;
        checks.push(run_check(
            "provider",
            health.ok,
            format!(
                "{} (latency={}ms)",
                health.detail,
                health.latency_ms.unwrap_or(0)
            ),
        ));
    } else {
        checks.push(run_check("config", false, "run `mosaic setup` first"));
    }

    emit_checks(cli.json, "health", checks)
}

pub(super) async fn collect_doctor_checks(cli: &Cli) -> Result<Vec<BTreeMap<String, Value>>> {
    let paths = resolve_state_paths(cli.project_state)?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let channels_repo = ChannelRepository::new(
        channels_file_path(&paths.data_dir),
        channels_events_dir(&paths.data_dir),
    );
    let mut checks = vec![];

    checks.push(run_check(
        "config_exists",
        manager.exists(),
        "config file presence",
    ));
    checks.push(run_check(
        "state_writable",
        paths.is_writable().is_ok(),
        "state directories writable",
    ));
    checks.push(run_check(
        "rg_binary",
        binary_in_path("rg"),
        "ripgrep available for search_text tool",
    ));

    if manager.exists() {
        let config = manager.load()?;
        let resolved = config.resolve_profile(Some(&cli.profile))?;
        let api_key_exists = std::env::var(&resolved.profile.provider.api_key_env).is_ok();
        checks.push(run_check(
            "api_key_env",
            api_key_exists,
            format!(
                "environment variable {} {}",
                resolved.profile.provider.api_key_env,
                if api_key_exists { "found" } else { "missing" }
            ),
        ));

        if api_key_exists {
            let provider = OpenAiCompatibleProvider::from_profile(&resolved.profile)?;
            let provider_health = provider.health().await?;
            checks.push(run_check(
                "provider_connectivity",
                provider_health.ok,
                provider_health.detail,
            ));
        } else {
            checks.push(run_check(
                "provider_connectivity",
                false,
                "skipped because API key env is missing",
            ));
        }
    }

    match channels_repo.doctor_checks() {
        Ok(channel_checks) => {
            for check in channel_checks {
                checks.push(run_check(check.name, check.ok, check.detail));
            }
        }
        Err(err) => {
            checks.push(run_check(
                "channels_file",
                false,
                format!("failed to inspect channels: {err}"),
            ));
        }
    }

    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    match approval_store.load_or_default() {
        Ok(policy) => {
            checks.push(run_check(
                "approvals_policy",
                true,
                format!(
                    "mode={:?} allowlist_size={} path={}",
                    policy.mode,
                    policy.allowlist.len(),
                    approval_store.path().display()
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "approvals_policy",
                false,
                format!("failed to load approvals policy: {err}"),
            ));
        }
    }

    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    match sandbox_store.load_or_default() {
        Ok(policy) => {
            checks.push(run_check(
                "sandbox_policy",
                true,
                format!(
                    "profile={:?} path={}",
                    policy.profile,
                    sandbox_store.path().display()
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "sandbox_policy",
                false,
                format!("failed to load sandbox policy: {err}"),
            ));
        }
    }

    let memory_store = MemoryStore::new(
        memory_index_path(&paths.data_dir),
        memory_status_path(&paths.data_dir),
    );
    match memory_store.status() {
        Ok(status) => {
            checks.push(run_check(
                "memory_index",
                true,
                format!(
                    "indexed_documents={} index_path={}",
                    status.indexed_documents, status.index_path
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "memory_index",
                false,
                format!("failed to load memory status: {err}"),
            ));
        }
    }

    let agent_store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    match agent_store.check_integrity() {
        Ok(report) => {
            checks.push(run_check(
                "agents_integrity",
                report.ok,
                format!(
                    "agents={} routes={} default={}",
                    report.agents_count,
                    report.routes_count,
                    report.default_agent_id.unwrap_or_else(|| "-".to_string())
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "agents_integrity",
                false,
                format!("failed to inspect agents: {err}"),
            ));
        }
    }

    let extension_registry =
        ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));
    match extension_registry.check_plugins(None) {
        Ok(report) => {
            checks.push(run_check(
                "plugins_check",
                report.ok,
                format!("checked={} failed={}", report.checked, report.failed),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "plugins_check",
                false,
                format!("failed to run plugin checks: {err}"),
            ));
        }
    }
    match extension_registry.check_skills(None) {
        Ok(report) => {
            checks.push(run_check(
                "skills_check",
                report.ok,
                format!("checked={} failed={}", report.checked, report.failed),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "skills_check",
                false,
                format!("failed to run skill checks: {err}"),
            ));
        }
    }

    let security_root = std::env::current_dir()
        .map_err(|err| mosaic_core::error::MosaicError::Io(err.to_string()))?;
    let baseline_path = paths.root_dir.join("security").join("baseline.toml");
    match SecurityBaselineConfig::load_optional(&baseline_path) {
        Ok(Some(baseline)) => {
            checks.push(run_check(
                "security_baseline",
                true,
                format!(
                    "path={} fingerprints={} categories={} paths={}",
                    baseline_path.display(),
                    baseline.ignored_fingerprints.len(),
                    baseline.ignored_categories.len(),
                    baseline.ignored_paths.len(),
                ),
            ));
        }
        Ok(None) => {
            checks.push(run_check(
                "security_baseline",
                true,
                format!("path={} (not configured)", baseline_path.display()),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "security_baseline",
                false,
                format!("failed to load security baseline: {err}"),
            ));
        }
    }
    let security_report = SecurityAuditor::new().audit(SecurityAuditOptions {
        root: security_root,
        deep: false,
        max_files: 200,
        max_file_size: 131_072,
    });
    match security_report {
        Ok(report) => {
            checks.push(run_check(
                "security_audit",
                report.summary.high == 0,
                format!(
                    "findings={} high={} medium={} low={} scanned={}",
                    report.summary.findings,
                    report.summary.high,
                    report.summary.medium,
                    report.summary.low,
                    report.summary.scanned_files
                ),
            ));
        }
        Err(err) => {
            checks.push(run_check(
                "security_audit",
                false,
                format!("failed to run security audit: {err}"),
            ));
        }
    }

    Ok(checks)
}

pub(super) async fn handle_doctor(cli: &Cli) -> Result<()> {
    let checks = collect_doctor_checks(cli).await?;
    emit_checks(cli.json, "doctor", checks)
}
