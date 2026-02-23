use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tiny_http::{Method, Response, Server};

use mosaic_agents::{
    AddAgentInput, AgentStore, UpdateAgentInput, agent_routes_path, agents_file_path,
};
use mosaic_channels::{
    AddChannelInput, ChannelRepository, ChannelSendOptions, ChannelTemplateDefaults,
    RotateTokenEnvInput, UpdateChannelInput, channels_events_dir, channels_file_path,
    format_channel_for_output,
};
use mosaic_gateway::{GatewayClient, GatewayRequest, HttpGatewayClient};
use mosaic_memory::{MemoryIndexOptions, MemoryStore, memory_index_path, memory_status_path};
use mosaic_plugins::{ExtensionRegistry, RegistryRoots};

use mosaic_agent::{AgentRunOptions, AgentRunner};
use mosaic_core::audit::AuditStore;
use mosaic_core::config::{
    ConfigManager, DEFAULT_PROFILE, ProfileConfig, RunGuardMode, StateConfig,
};
use mosaic_core::error::{MosaicError, Result};
use mosaic_core::models::{ModelProfileConfig, ModelRoutingStore};
use mosaic_core::provider::{ChatRequest, ChatResponse, ModelInfo, Provider, ProviderHealth};
use mosaic_core::session::SessionStore;
use mosaic_core::state::{StateMode, StatePaths};
use mosaic_ops::{
    ApprovalDecision, ApprovalMode, ApprovalStore, RuntimePolicy, SandboxProfile, SandboxStore,
    SystemEvent, SystemEventStore, collect_logs, evaluate_approval, evaluate_sandbox,
    list_profiles, snapshot_presence, system_events_path,
};
use mosaic_provider_openai::OpenAiCompatibleProvider;
use mosaic_security::{
    SecurityAuditOptions, SecurityAuditor, SecurityBaselineConfig, apply_baseline, report_to_sarif,
};
use mosaic_tools::{RunCommandOutput, ToolContext, ToolExecutor};

const PROJECT_STATE_DIR: &str = ".mosaic";
static PAIRING_REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);
static HOOK_SEQ: AtomicU64 = AtomicU64::new(1);
static CRON_SEQ: AtomicU64 = AtomicU64::new(1);
static WEBHOOK_SEQ: AtomicU64 = AtomicU64::new(1);
static BROWSER_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Parser, Debug)]
#[command(name = "mosaic", version, about = "Mosaic local agent CLI")]
struct Cli {
    #[arg(long, default_value = DEFAULT_PROFILE)]
    profile: String,
    #[arg(long)]
    project_state: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long)]
    debug: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    #[command(visible_alias = "onboard")]
    Setup(SetupArgs),
    Configure(ConfigureArgs),
    Models(ModelsArgs),
    #[command(visible_alias = "message")]
    Ask(AskArgs),
    #[command(visible_alias = "agent")]
    Chat(ChatArgs),
    Session(SessionArgs),
    Gateway(GatewayArgs),
    Channels(ChannelsArgs),
    Nodes(NodesArgs),
    Devices(DevicesArgs),
    Pairing(PairingArgs),
    Hooks(HooksArgs),
    Cron(CronArgs),
    Webhooks(WebhooksArgs),
    Browser(BrowserArgs),
    Logs(LogsArgs),
    System(SystemArgs),
    Approvals(ApprovalsArgs),
    Sandbox(SandboxArgs),
    Memory(MemoryArgs),
    Security(SecurityArgs),
    Agents(AgentsArgs),
    Plugins(PluginsArgs),
    Skills(SkillsArgs),
    Status,
    Health,
    Doctor,
}

#[derive(Args, Debug, Clone)]
struct SetupArgs {
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    api_key_env: Option<String>,
    #[arg(long)]
    temperature: Option<f32>,
    #[arg(long)]
    max_turns: Option<u32>,
    #[arg(long)]
    tools_enabled: Option<bool>,
    #[arg(long, value_enum)]
    guard_mode: Option<GuardModeArg>,
}

#[derive(Args, Debug, Clone)]
struct ConfigureArgs {
    #[arg(long)]
    show: bool,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    api_key_env: Option<String>,
    #[arg(long)]
    temperature: Option<f32>,
    #[arg(long)]
    max_turns: Option<u32>,
    #[arg(long)]
    tools_enabled: Option<bool>,
    #[arg(long, value_enum)]
    guard_mode: Option<GuardModeArg>,
}

#[derive(Args, Debug, Clone)]
struct ModelsArgs {
    #[command(subcommand)]
    command: ModelsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ModelsCommand {
    List,
    Status,
    Set {
        model: String,
    },
    Aliases {
        #[command(subcommand)]
        command: ModelAliasesCommand,
    },
    Fallbacks {
        #[command(subcommand)]
        command: ModelFallbacksCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum ModelAliasesCommand {
    List,
    Set { alias: String, model: String },
    Remove { alias: String },
    Clear,
}

#[derive(Subcommand, Debug, Clone)]
enum ModelFallbacksCommand {
    List,
    Add { model: String },
    Remove { model: String },
    Clear,
}

#[derive(Args, Debug, Clone)]
struct AskArgs {
    prompt: String,
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    agent: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct ChatArgs {
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    prompt: Option<String>,
    #[arg(long)]
    agent: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct SessionArgs {
    #[command(subcommand)]
    command: SessionCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SessionCommand {
    List,
    Show {
        session_id: String,
    },
    Resume {
        session_id: String,
    },
    Clear {
        session_id: Option<String>,
        #[arg(long)]
        all: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct GatewayArgs {
    #[command(subcommand)]
    command: GatewayCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum GatewayCommand {
    Install {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    #[command(visible_alias = "run")]
    Start {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    Restart {
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    Status {
        #[arg(long)]
        deep: bool,
    },
    Health {
        #[arg(long)]
        verbose: bool,
    },
    Call {
        method: String,
        #[arg(long)]
        params: Option<String>,
    },
    Probe,
    Discover,
    Stop,
    Uninstall,
    #[command(hide = true)]
    Serve {
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
    },
}

#[derive(Args, Debug, Clone)]
struct NodesArgs {
    #[command(subcommand)]
    command: NodesCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum NodesCommand {
    List,
    Status {
        node_id: Option<String>,
    },
    Run {
        node_id: String,
        #[arg(long)]
        command: String,
    },
    Invoke {
        node_id: String,
        method: String,
        #[arg(long)]
        params: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct DevicesArgs {
    #[command(subcommand)]
    command: DevicesCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum DevicesCommand {
    List,
    Approve {
        device_id: String,
        #[arg(long)]
        name: Option<String>,
    },
    Reject {
        device_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Rotate {
        device_id: String,
    },
    Revoke {
        device_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct PairingArgs {
    #[command(subcommand)]
    command: PairingCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum PairingCommand {
    List {
        #[arg(long, value_enum)]
        status: Option<PairingStatusArg>,
    },
    Approve {
        request_id: String,
    },
    Request {
        #[arg(long)]
        device: String,
        #[arg(long, default_value = "local")]
        node: String,
        #[arg(long)]
        reason: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct HooksArgs {
    #[command(subcommand)]
    command: HooksCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum HooksCommand {
    List {
        #[arg(long)]
        event: Option<String>,
    },
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        event: String,
        #[arg(long)]
        command: String,
        #[arg(long)]
        disabled: bool,
    },
    Remove {
        hook_id: String,
    },
    Enable {
        hook_id: String,
    },
    Disable {
        hook_id: String,
    },
    Run {
        hook_id: String,
        #[arg(long)]
        data: Option<String>,
    },
    Logs {
        #[arg(long)]
        hook: Option<String>,
        #[arg(long, default_value_t = 50)]
        tail: usize,
    },
}

#[derive(Args, Debug, Clone)]
struct CronArgs {
    #[command(subcommand)]
    command: CronCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum CronCommand {
    List {
        #[arg(long)]
        event: Option<String>,
    },
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        event: String,
        #[arg(long)]
        every: u64,
        #[arg(long)]
        data: Option<String>,
        #[arg(long)]
        disabled: bool,
    },
    Remove {
        job_id: String,
    },
    Enable {
        job_id: String,
    },
    Disable {
        job_id: String,
    },
    Run {
        job_id: String,
        #[arg(long)]
        data: Option<String>,
    },
    Tick {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Logs {
        #[arg(long)]
        job: Option<String>,
        #[arg(long, default_value_t = 50)]
        tail: usize,
    },
}

#[derive(Args, Debug, Clone)]
struct WebhooksArgs {
    #[command(subcommand)]
    command: WebhooksCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum WebhooksCommand {
    List {
        #[arg(long)]
        event: Option<String>,
    },
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        event: String,
        #[arg(long)]
        path: String,
        #[arg(long, value_enum, default_value_t = WebhookMethodArg::Post)]
        method: WebhookMethodArg,
        #[arg(long)]
        secret_env: Option<String>,
        #[arg(long)]
        disabled: bool,
    },
    Remove {
        webhook_id: String,
    },
    Enable {
        webhook_id: String,
    },
    Disable {
        webhook_id: String,
    },
    Trigger {
        webhook_id: String,
        #[arg(long)]
        data: Option<String>,
        #[arg(long)]
        secret: Option<String>,
    },
    Resolve {
        #[arg(long)]
        path: String,
        #[arg(long, value_enum, default_value_t = WebhookMethodArg::Post)]
        method: WebhookMethodArg,
        #[arg(long)]
        data: Option<String>,
        #[arg(long)]
        secret: Option<String>,
    },
    Logs {
        #[arg(long)]
        webhook: Option<String>,
        #[arg(long, default_value_t = 50)]
        tail: usize,
    },
}

#[derive(Args, Debug, Clone)]
struct BrowserArgs {
    #[command(subcommand)]
    command: BrowserCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum BrowserCommand {
    #[command(visible_alias = "visit")]
    Open {
        #[arg(long)]
        url: String,
        #[arg(long, default_value_t = 10_000)]
        timeout_ms: u64,
    },
    History {
        #[arg(long, default_value_t = 20)]
        tail: usize,
    },
    Show {
        visit_id: String,
    },
    Clear {
        visit_id: Option<String>,
        #[arg(long)]
        all: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct ChannelsArgs {
    #[command(subcommand)]
    command: ChannelsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ChannelsCommand {
    List,
    Status,
    Add {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "slack_webhook")]
        kind: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        chat_id: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long)]
        default_parse_mode: Option<String>,
        #[arg(long)]
        default_title: Option<String>,
        #[arg(long)]
        default_block: Vec<String>,
        #[arg(long)]
        default_metadata: Option<String>,
    },
    Update {
        channel_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        chat_id: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long, conflicts_with = "token_env")]
        clear_token_env: bool,
        #[arg(long)]
        default_parse_mode: Option<String>,
        #[arg(long)]
        default_title: Option<String>,
        #[arg(long)]
        default_block: Vec<String>,
        #[arg(long)]
        default_metadata: Option<String>,
        #[arg(
            long,
            conflicts_with_all = [
                "default_parse_mode",
                "default_title",
                "default_block",
                "default_metadata"
            ]
        )]
        clear_defaults: bool,
    },
    Login {
        channel_id: String,
        #[arg(long)]
        token_env: Option<String>,
    },
    Send {
        channel_id: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        parse_mode: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        block: Vec<String>,
        #[arg(long)]
        metadata: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
    },
    Test {
        channel_id: String,
        #[arg(long)]
        token_env: Option<String>,
    },
    Logs {
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = 50)]
        tail: usize,
    },
    Capabilities {
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        target: Option<String>,
    },
    Resolve {
        #[arg(long)]
        channel: String,
        query: Vec<String>,
    },
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Import {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        replace: bool,
        #[arg(long, conflicts_with = "replace")]
        strict: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
    RotateTokenEnv {
        #[arg(long, conflicts_with = "all")]
        channel: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, requires = "all")]
        kind: Option<String>,
        #[arg(long = "from")]
        from_token_env: Option<String>,
        #[arg(long)]
        to: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
    Remove {
        channel_id: String,
    },
    Logout {
        channel_id: String,
    },
}

#[derive(Args, Debug, Clone)]
struct LogsArgs {
    #[arg(long)]
    follow: bool,
    #[arg(long, default_value_t = 100)]
    tail: usize,
}

#[derive(Args, Debug, Clone)]
struct SystemArgs {
    #[command(subcommand)]
    command: SystemCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SystemCommand {
    Event {
        name: String,
        #[arg(long)]
        data: Option<String>,
    },
    Presence,
}

#[derive(Args, Debug, Clone)]
struct ApprovalsArgs {
    #[command(subcommand)]
    command: ApprovalsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ApprovalsCommand {
    Get,
    Set {
        #[arg(value_enum)]
        mode: ApprovalModeArg,
    },
    Allowlist {
        #[command(subcommand)]
        command: AllowlistCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum AllowlistCommand {
    Add { prefix: String },
    Remove { prefix: String },
}

#[derive(Args, Debug, Clone)]
struct SandboxArgs {
    #[command(subcommand)]
    command: SandboxCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SandboxCommand {
    List,
    Explain {
        #[arg(long, value_enum)]
        profile: Option<SandboxProfileArg>,
    },
}

#[derive(Args, Debug, Clone)]
struct MemoryArgs {
    #[command(subcommand)]
    command: MemoryCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum MemoryCommand {
    Index {
        #[arg(long, default_value = ".")]
        path: String,
        #[arg(long, default_value_t = 500)]
        max_files: usize,
        #[arg(long, default_value_t = 262_144)]
        max_file_size: usize,
        #[arg(long, default_value_t = 16_384)]
        max_content_bytes: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Status,
}

#[derive(Args, Debug, Clone)]
struct SecurityArgs {
    #[command(subcommand)]
    command: SecurityCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SecurityCommand {
    Audit {
        #[arg(long, default_value = ".")]
        path: String,
        #[arg(long)]
        deep: bool,
        #[arg(long, default_value_t = 800)]
        max_files: usize,
        #[arg(long, default_value_t = 262_144)]
        max_file_size: usize,
        #[arg(long)]
        baseline: Option<String>,
        #[arg(long)]
        no_baseline: bool,
        #[arg(long)]
        update_baseline: bool,
        #[arg(long)]
        sarif: bool,
        #[arg(long)]
        sarif_output: Option<String>,
    },
    Baseline {
        #[command(subcommand)]
        command: SecurityBaselineCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum SecurityBaselineCommand {
    Show {
        #[arg(long)]
        path: Option<String>,
    },
    Add {
        #[arg(long)]
        path: Option<String>,
        #[arg(long = "fingerprint")]
        fingerprints: Vec<String>,
        #[arg(long = "category")]
        categories: Vec<String>,
        #[arg(long = "match-path")]
        match_paths: Vec<String>,
    },
    Remove {
        #[arg(long)]
        path: Option<String>,
        #[arg(long = "fingerprint")]
        fingerprints: Vec<String>,
        #[arg(long = "category")]
        categories: Vec<String>,
        #[arg(long = "match-path")]
        match_paths: Vec<String>,
    },
    Clear {
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct AgentsArgs {
    #[command(subcommand)]
    command: AgentsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum AgentsCommand {
    List,
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        max_turns: Option<u32>,
        #[arg(long)]
        tools_enabled: Option<bool>,
        #[arg(long, value_enum)]
        guard_mode: Option<GuardModeArg>,
        #[arg(long)]
        set_default: bool,
        #[arg(long = "route")]
        route_keys: Vec<String>,
    },
    Update {
        agent_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        clear_model: bool,
        #[arg(long)]
        temperature: Option<f32>,
        #[arg(long)]
        clear_temperature: bool,
        #[arg(long)]
        max_turns: Option<u32>,
        #[arg(long)]
        clear_max_turns: bool,
        #[arg(long)]
        tools_enabled: Option<bool>,
        #[arg(long)]
        clear_tools_enabled: bool,
        #[arg(long, value_enum)]
        guard_mode: Option<GuardModeArg>,
        #[arg(long)]
        clear_guard_mode: bool,
        #[arg(long)]
        set_default: bool,
        #[arg(long = "route")]
        route_keys: Vec<String>,
    },
    Show {
        agent_id: String,
    },
    Remove {
        agent_id: String,
    },
    Default {
        agent_id: Option<String>,
    },
    Route {
        #[command(subcommand)]
        command: AgentsRouteCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum AgentsRouteCommand {
    List,
    Set {
        route_key: String,
        agent_id: String,
    },
    Remove {
        route_key: String,
    },
    Resolve {
        #[arg(long)]
        route: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
struct PluginsArgs {
    #[command(subcommand)]
    command: PluginsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum PluginsCommand {
    List,
    Info {
        plugin_id: String,
    },
    Check {
        plugin_id: Option<String>,
    },
    Install {
        #[arg(long)]
        path: String,
        #[arg(long)]
        force: bool,
    },
    Remove {
        plugin_id: String,
    },
}

#[derive(Args, Debug, Clone)]
struct SkillsArgs {
    #[command(subcommand)]
    command: SkillsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SkillsCommand {
    List,
    Info {
        skill_id: String,
    },
    Check {
        skill_id: Option<String>,
    },
    Install {
        #[arg(long)]
        path: String,
        #[arg(long)]
        force: bool,
    },
    Remove {
        skill_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GatewayState {
    running: bool,
    host: String,
    port: u16,
    pid: u32,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GatewayServiceState {
    installed: bool,
    host: String,
    port: u16,
    installed_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct GatewayRuntimeStatus {
    running: bool,
    process_alive: bool,
    endpoint_healthy: bool,
    state: Option<GatewayState>,
    service: Option<GatewayServiceState>,
    target_host: String,
    target_port: u16,
}

#[derive(Debug, Clone)]
struct GatewayStartResult {
    state: GatewayState,
    already_running: bool,
}

#[derive(Debug, Clone)]
struct GatewayStopResult {
    was_running: bool,
    stopped: bool,
    state: Option<GatewayState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum NodeRuntimeStatus {
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeRecord {
    id: String,
    name: String,
    status: NodeRuntimeStatus,
    capabilities: Vec<String>,
    last_seen_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DeviceStatus {
    Pending,
    Approved,
    Rejected,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceRecord {
    id: String,
    name: String,
    fingerprint: String,
    status: DeviceStatus,
    token_version: u32,
    last_seen_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PairingStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairingRequestRecord {
    id: String,
    device_id: String,
    node_id: String,
    status: PairingStatus,
    reason: Option<String>,
    requested_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookRecord {
    id: String,
    name: String,
    event: String,
    command: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_triggered_at: Option<DateTime<Utc>>,
    last_result: Option<HookLastResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookLastResult {
    ok: bool,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    approved_by: Option<String>,
    error_code: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookEventRecord {
    ts: DateTime<Utc>,
    hook_id: String,
    hook_name: String,
    event: String,
    trigger: String,
    command: String,
    delivery_status: String,
    ok: bool,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    approved_by: Option<String>,
    error_code: Option<String>,
    error: Option<String>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    data: Value,
}

#[derive(Debug, Clone, Serialize)]
struct HookExecutionReport {
    hook_id: String,
    hook_name: String,
    event: String,
    trigger: String,
    command: String,
    ok: bool,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    approved_by: Option<String>,
    error_code: Option<String>,
    error: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CronJobRecord {
    id: String,
    name: String,
    event: String,
    every_seconds: u64,
    data: Value,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_run_at: Option<DateTime<Utc>>,
    next_run_at: DateTime<Utc>,
    run_count: u64,
    last_result: Option<CronLastResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CronLastResult {
    ok: bool,
    hooks_triggered: usize,
    hooks_ok: usize,
    hooks_failed: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CronEventRecord {
    ts: DateTime<Utc>,
    job_id: String,
    job_name: String,
    trigger: String,
    event: String,
    data: Value,
    ok: bool,
    hooks_triggered: usize,
    hooks_ok: usize,
    hooks_failed: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CronExecutionReport {
    job_id: String,
    job_name: String,
    trigger: String,
    event: String,
    ok: bool,
    hooks_triggered: usize,
    hooks_ok: usize,
    hooks_failed: usize,
    error: Option<String>,
    system_event: Option<SystemEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum WebhookMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl std::fmt::Display for WebhookMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebhookRecord {
    id: String,
    name: String,
    event: String,
    path: String,
    method: WebhookMethod,
    secret_env: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_triggered_at: Option<DateTime<Utc>>,
    last_result: Option<WebhookLastResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebhookLastResult {
    ok: bool,
    hooks_triggered: usize,
    hooks_ok: usize,
    hooks_failed: usize,
    error_code: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebhookEventRecord {
    ts: DateTime<Utc>,
    webhook_id: String,
    webhook_name: String,
    trigger: String,
    event: String,
    path: String,
    method: WebhookMethod,
    data: Value,
    ok: bool,
    hooks_triggered: usize,
    hooks_ok: usize,
    hooks_failed: usize,
    error_code: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct WebhookExecutionReport {
    webhook_id: String,
    webhook_name: String,
    trigger: String,
    event: String,
    path: String,
    method: WebhookMethod,
    ok: bool,
    hooks_triggered: usize,
    hooks_ok: usize,
    hooks_failed: usize,
    error_code: Option<String>,
    error: Option<String>,
    system_event: Option<SystemEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserVisitRecord {
    id: String,
    ts: DateTime<Utc>,
    url: String,
    ok: bool,
    http_status: Option<u16>,
    title: Option<String>,
    content_type: Option<String>,
    content_length: Option<usize>,
    preview: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct SystemEventDispatch {
    event: SystemEvent,
    hook_reports: Vec<HookExecutionReport>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum GuardModeArg {
    ConfirmDangerous,
    AllConfirm,
    Unrestricted,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum ApprovalModeArg {
    Deny,
    Confirm,
    Allowlist,
}

impl From<ApprovalModeArg> for ApprovalMode {
    fn from(value: ApprovalModeArg) -> Self {
        match value {
            ApprovalModeArg::Deny => Self::Deny,
            ApprovalModeArg::Confirm => Self::Confirm,
            ApprovalModeArg::Allowlist => Self::Allowlist,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum SandboxProfileArg {
    Restricted,
    Standard,
    Elevated,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
enum PairingStatusArg {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lower")]
enum WebhookMethodArg {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl From<PairingStatusArg> for PairingStatus {
    fn from(value: PairingStatusArg) -> Self {
        match value {
            PairingStatusArg::Pending => Self::Pending,
            PairingStatusArg::Approved => Self::Approved,
            PairingStatusArg::Rejected => Self::Rejected,
        }
    }
}

impl From<SandboxProfileArg> for SandboxProfile {
    fn from(value: SandboxProfileArg) -> Self {
        match value {
            SandboxProfileArg::Restricted => Self::Restricted,
            SandboxProfileArg::Standard => Self::Standard,
            SandboxProfileArg::Elevated => Self::Elevated,
        }
    }
}

impl From<WebhookMethodArg> for WebhookMethod {
    fn from(value: WebhookMethodArg) -> Self {
        match value {
            WebhookMethodArg::Get => Self::Get,
            WebhookMethodArg::Post => Self::Post,
            WebhookMethodArg::Put => Self::Put,
            WebhookMethodArg::Patch => Self::Patch,
            WebhookMethodArg::Delete => Self::Delete,
        }
    }
}

impl From<GuardModeArg> for RunGuardMode {
    fn from(value: GuardModeArg) -> Self {
        match value {
            GuardModeArg::ConfirmDangerous => Self::ConfirmDangerous,
            GuardModeArg::AllConfirm => Self::AllConfirm,
            GuardModeArg::Unrestricted => Self::Unrestricted,
        }
    }
}

struct RuntimeContext {
    provider: Arc<dyn Provider>,
    agent: AgentRunner,
    active_agent_id: Option<String>,
    active_profile_name: String,
}

struct ModelRoutingProvider {
    inner: Arc<dyn Provider>,
    fallback_models: Vec<String>,
}

impl ModelRoutingProvider {
    fn new(inner: Arc<dyn Provider>, fallback_models: Vec<String>) -> Self {
        Self {
            inner,
            fallback_models,
        }
    }
}

#[async_trait::async_trait]
impl Provider for ModelRoutingProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        self.inner.list_models().await
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if self.fallback_models.is_empty() {
            return self.inner.chat(request).await;
        }

        let mut attempts = vec![request.model.clone()];
        for model in &self.fallback_models {
            if !attempts.iter().any(|candidate| candidate == model) {
                attempts.push(model.clone());
            }
        }

        let mut last_error: Option<MosaicError> = None;
        for model in &attempts {
            let mut retry_request = request.clone();
            retry_request.model = model.clone();
            match self.inner.chat(retry_request).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if matches!(err, MosaicError::Auth(_)) {
                        return Err(err);
                    }
                    last_error = Some(err);
                }
            }
        }

        let chain = attempts.join(" -> ");
        Err(last_error
            .unwrap_or_else(|| MosaicError::Unknown("model fallback failed".to_string()))
            .with_context(format!("chat failed across model chain [{chain}]")))
    }

    async fn health(&self) -> Result<ProviderHealth> {
        self.inner.health().await
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    let result = run(cli).await;
    if let Err(err) = result {
        if json_mode {
            print_json(&json!({
                "ok": false,
                "error": {
                    "code": err.code(),
                    "message": err.to_string(),
                    "exit_code": err.exit_code(),
                }
            }));
        } else {
            eprintln!("error [{}]: {}", err.code(), err);
        }
        std::process::exit(err.exit_code());
    }
}

async fn run(cli: Cli) -> Result<()> {
    if cli.debug {
        eprintln!(
            "[debug] profile={} project_state={} json={}",
            cli.profile, cli.project_state, cli.json
        );
    }
    match cli.command.clone() {
        Commands::Setup(args) => handle_setup(&cli, args),
        Commands::Configure(args) => handle_configure(&cli, args),
        Commands::Models(args) => handle_models(&cli, args).await,
        Commands::Ask(args) => handle_ask(&cli, args).await,
        Commands::Chat(args) => handle_chat(&cli, args).await,
        Commands::Session(args) => handle_session(&cli, args).await,
        Commands::Gateway(args) => handle_gateway(&cli, args).await,
        Commands::Channels(args) => handle_channels(&cli, args).await,
        Commands::Nodes(args) => handle_nodes(&cli, args).await,
        Commands::Devices(args) => handle_devices(&cli, args),
        Commands::Pairing(args) => handle_pairing(&cli, args),
        Commands::Hooks(args) => handle_hooks(&cli, args),
        Commands::Cron(args) => handle_cron(&cli, args),
        Commands::Webhooks(args) => handle_webhooks(&cli, args),
        Commands::Browser(args) => handle_browser(&cli, args).await,
        Commands::Logs(args) => handle_logs(&cli, args).await,
        Commands::System(args) => handle_system(&cli, args),
        Commands::Approvals(args) => handle_approvals(&cli, args),
        Commands::Sandbox(args) => handle_sandbox(&cli, args),
        Commands::Memory(args) => handle_memory(&cli, args),
        Commands::Security(args) => handle_security(&cli, args),
        Commands::Agents(args) => handle_agents(&cli, args),
        Commands::Plugins(args) => handle_plugins(&cli, args),
        Commands::Skills(args) => handle_skills(&cli, args),
        Commands::Status => handle_status(&cli),
        Commands::Health => handle_health(&cli).await,
        Commands::Doctor => handle_doctor(&cli).await,
    }
}

fn handle_setup(cli: &Cli, args: SetupArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut config = manager.load_or_default(paths.mode)?;
    let profile = config
        .profiles
        .entry(cli.profile.clone())
        .or_insert_with(ProfileConfig::default);
    if let Some(base_url) = args.base_url {
        profile.provider.base_url = base_url;
    }
    if let Some(model) = args.model {
        profile.provider.model = model;
    }
    if let Some(api_key_env) = args.api_key_env {
        profile.provider.api_key_env = api_key_env;
    }
    if let Some(temperature) = args.temperature {
        profile.agent.temperature = temperature;
    }
    if let Some(max_turns) = args.max_turns {
        profile.agent.max_turns = max_turns;
    }
    if let Some(tools_enabled) = args.tools_enabled {
        profile.tools.enabled = tools_enabled;
    }
    if let Some(guard_mode) = args.guard_mode {
        profile.tools.run.guard_mode = guard_mode.into();
    }
    config.active_profile = cli.profile.clone();
    config.state = StateConfig {
        mode: paths.mode,
        project_dir: PROJECT_STATE_DIR.to_string(),
    };
    manager.save(&config)?;

    if cli.json {
        print_json(&json!({
            "ok": true,
            "config_path": manager.path().display().to_string(),
            "profile": cli.profile,
            "mode": paths.mode,
        }));
    } else {
        println!("Setup complete.");
        println!("Config: {}", manager.path().display());
        println!("Profile: {}", cli.profile);
        println!("Mode: {:?}", paths.mode);
    }
    Ok(())
}

fn handle_configure(cli: &Cli, args: ConfigureArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let mut config = manager.load()?;

    let mut changed = false;
    {
        let profile = config
            .profiles
            .entry(cli.profile.clone())
            .or_insert_with(ProfileConfig::default);
        if let Some(base_url) = args.base_url {
            profile.provider.base_url = base_url;
            changed = true;
        }
        if let Some(model) = args.model {
            profile.provider.model = model;
            changed = true;
        }
        if let Some(api_key_env) = args.api_key_env {
            profile.provider.api_key_env = api_key_env;
            changed = true;
        }
        if let Some(temperature) = args.temperature {
            profile.agent.temperature = temperature;
            changed = true;
        }
        if let Some(max_turns) = args.max_turns {
            profile.agent.max_turns = max_turns;
            changed = true;
        }
        if let Some(tools_enabled) = args.tools_enabled {
            profile.tools.enabled = tools_enabled;
            changed = true;
        }
        if let Some(guard_mode) = args.guard_mode {
            profile.tools.run.guard_mode = guard_mode.into();
            changed = true;
        }
    }

    config.active_profile = cli.profile.clone();
    if changed {
        manager.save(&config)?;
    }
    let resolved = config.resolve_profile(Some(&cli.profile))?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "changed": changed,
            "profile": resolved.profile_name,
            "config_path": manager.path().display().to_string(),
            "config": resolved,
        }));
    } else if args.show || !changed {
        println!("Config path: {}", manager.path().display());
        println!("Profile: {}", resolved.profile_name);
        println!("Provider base URL: {}", resolved.profile.provider.base_url);
        println!("Model: {}", resolved.profile.provider.model);
        println!("API key env: {}", resolved.profile.provider.api_key_env);
        println!("Tools enabled: {}", resolved.profile.tools.enabled);
        println!("Guard mode: {:?}", resolved.profile.tools.run.guard_mode);
    } else {
        println!(
            "Configuration updated for profile '{}'.",
            resolved.profile_name
        );
    }
    Ok(())
}

async fn handle_models(cli: &Cli, args: ModelsArgs) -> Result<()> {
    match args.command {
        ModelsCommand::List => {
            let runtime = build_runtime(cli, None, None)?;
            let models = runtime.provider.list_models().await?;
            if cli.json {
                print_json(&json!({ "ok": true, "models": models }));
            } else {
                for model in &models {
                    if let Some(owner) = &model.owned_by {
                        println!("{} ({owner})", model.id);
                    } else {
                        println!("{}", model.id);
                    }
                }
                println!("Total models: {}", models.len());
            }
        }
        ModelsCommand::Status => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let resolved = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = model_store.profile(&resolved.profile_name)?;
            let current_model = resolved.profile.provider.model.clone();
            let (effective_model, used_alias) =
                resolve_effective_model(&profile_models, &current_model);

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": resolved.profile_name,
                    "base_url": resolved.profile.provider.base_url,
                    "api_key_env": resolved.profile.provider.api_key_env,
                    "current_model": current_model,
                    "effective_model": effective_model,
                    "used_alias": used_alias,
                    "aliases": profile_models.aliases,
                    "fallbacks": profile_models.fallbacks,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else {
                println!("profile: {}", resolved.profile_name);
                println!("base url: {}", resolved.profile.provider.base_url);
                println!("api key env: {}", resolved.profile.provider.api_key_env);
                println!("current model: {}", current_model);
                if let Some(alias) = used_alias {
                    println!("effective model: {} (alias: {alias})", effective_model);
                } else {
                    println!("effective model: {}", effective_model);
                }
                if profile_models.aliases.is_empty() {
                    println!("aliases: <empty>");
                } else {
                    println!("aliases:");
                    for (alias, target) in profile_models.aliases {
                        println!("- {alias} => {target}");
                    }
                }
                if profile_models.fallbacks.is_empty() {
                    println!("fallbacks: <empty>");
                } else {
                    println!("fallbacks:");
                    for fallback in profile_models.fallbacks {
                        println!("- {fallback}");
                    }
                }
                println!("models path: {}", model_store.path().display());
            }
        }
        ModelsCommand::Set { model } => {
            let requested_model = model.trim();
            if requested_model.is_empty() {
                return Err(MosaicError::Validation("model cannot be empty".to_string()));
            }
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let mut config = manager.load()?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = model_store.profile(&cli.profile)?;
            let (effective_model, used_alias) =
                resolve_effective_model(&profile_models, requested_model);

            let profile = config.profiles.get_mut(&cli.profile).ok_or_else(|| {
                MosaicError::Config(format!("profile '{}' not found", cli.profile))
            })?;
            let previous_model = profile.provider.model.clone();
            profile.provider.model = effective_model.clone();
            config.active_profile = cli.profile.clone();
            manager.save(&config)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": cli.profile,
                    "requested_model": requested_model,
                    "effective_model": effective_model,
                    "used_alias": used_alias,
                    "previous_model": previous_model,
                }));
            } else {
                if let Some(alias) = used_alias {
                    println!(
                        "updated profile '{}' model: {} -> {} (from alias '{}')",
                        cli.profile, previous_model, effective_model, alias
                    );
                } else {
                    println!(
                        "updated profile '{}' model: {} -> {}",
                        cli.profile, previous_model, effective_model
                    );
                }
            }
        }
        ModelsCommand::Aliases { command } => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let _ = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = match command {
                ModelAliasesCommand::List => model_store.profile(&cli.profile)?,
                ModelAliasesCommand::Set { alias, model } => {
                    model_store.set_alias(&cli.profile, &alias, &model)?
                }
                ModelAliasesCommand::Remove { alias } => {
                    model_store.remove_alias(&cli.profile, &alias)?
                }
                ModelAliasesCommand::Clear => model_store.clear_aliases(&cli.profile)?,
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": cli.profile,
                    "aliases": profile_models.aliases,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else if profile_models.aliases.is_empty() {
                println!("aliases: <empty>");
                println!("models path: {}", model_store.path().display());
            } else {
                println!("aliases:");
                for (alias, target) in profile_models.aliases {
                    println!("- {alias} => {target}");
                }
                println!("models path: {}", model_store.path().display());
            }
        }
        ModelsCommand::Fallbacks { command } => {
            let paths = resolve_state_paths(cli.project_state)?;
            paths.ensure_dirs()?;
            let manager = ConfigManager::new(paths.config_path.clone());
            let config = manager.load()?;
            let _ = config.resolve_profile(Some(&cli.profile))?;
            let model_store = ModelRoutingStore::new(paths.models_path.clone());
            let profile_models = match command {
                ModelFallbacksCommand::List => model_store.profile(&cli.profile)?,
                ModelFallbacksCommand::Add { model } => {
                    model_store.add_fallback(&cli.profile, &model)?
                }
                ModelFallbacksCommand::Remove { model } => {
                    model_store.remove_fallback(&cli.profile, &model)?
                }
                ModelFallbacksCommand::Clear => model_store.clear_fallbacks(&cli.profile)?,
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": cli.profile,
                    "fallbacks": profile_models.fallbacks,
                    "models_path": model_store.path().display().to_string(),
                }));
            } else if profile_models.fallbacks.is_empty() {
                println!("fallbacks: <empty>");
                println!("models path: {}", model_store.path().display());
            } else {
                println!("fallbacks:");
                for fallback in profile_models.fallbacks {
                    println!("- {fallback}");
                }
                println!("models path: {}", model_store.path().display());
            }
        }
    }
    Ok(())
}

fn resolve_effective_model(
    profile_models: &ModelProfileConfig,
    requested_model: &str,
) -> (String, Option<String>) {
    let normalized_requested = requested_model.trim().to_ascii_lowercase();
    if let Some(target) = profile_models.aliases.get(&normalized_requested) {
        return (target.clone(), Some(normalized_requested));
    }
    (requested_model.trim().to_string(), None)
}

async fn handle_ask(cli: &Cli, args: AskArgs) -> Result<()> {
    let runtime = build_runtime(cli, args.agent.as_deref(), Some("ask"))?;
    let result = runtime
        .agent
        .ask(
            &args.prompt,
            AgentRunOptions {
                session_id: args.session,
                cwd: std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?,
                yes: cli.yes,
                interactive: false,
            },
        )
        .await?;
    if cli.json {
        print_json(&json!({
            "ok": true,
            "session_id": result.session_id,
            "response": result.response,
            "turns": result.turns,
            "agent_id": runtime.active_agent_id,
            "profile": runtime.active_profile_name,
        }));
    } else {
        println!("{}", result.response.trim());
        println!("session: {}", result.session_id);
        if let Some(agent_id) = &runtime.active_agent_id {
            println!("agent: {agent_id}");
        }
    }
    Ok(())
}

async fn handle_chat(cli: &Cli, args: ChatArgs) -> Result<()> {
    let runtime = build_runtime(cli, args.agent.as_deref(), Some("chat"))?;
    let mut session_id = args.session;

    if let Some(prompt) = args.prompt {
        let result = runtime
            .agent
            .ask(
                &prompt,
                AgentRunOptions {
                    session_id: session_id.clone(),
                    cwd: std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?,
                    yes: cli.yes,
                    interactive: true,
                },
            )
            .await?;
        session_id = Some(result.session_id.clone());
        if cli.json {
            print_json(&json!({
                "ok": true,
                "session_id": result.session_id,
                "response": result.response,
                "turns": result.turns,
                "agent_id": runtime.active_agent_id,
                "profile": runtime.active_profile_name,
            }));
            return Ok(());
        }
        println!("{}", result.response.trim());
        println!("session: {}", result.session_id);
        if let Some(agent_id) = &runtime.active_agent_id {
            println!("agent: {agent_id}");
        }
    } else if cli.json {
        return Err(MosaicError::Validation(
            "chat in --json mode requires --prompt".to_string(),
        ));
    }

    println!("Entering chat mode. Type /help for commands, /exit to quit.");
    if let Some(id) = &session_id {
        println!("Resumed session: {id}");
    }
    if let Some(agent_id) = &runtime.active_agent_id {
        println!("Using agent: {agent_id}");
    }
    loop {
        print!("you> ");
        io::stdout()
            .flush()
            .map_err(|err| MosaicError::Io(err.to_string()))?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|err| MosaicError::Io(err.to_string()))?;
        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        if matches!(prompt, "/exit" | "exit" | "quit") {
            println!("Bye.");
            break;
        }
        if prompt == "/help" {
            println!("/help     Show help");
            println!("/session  Show current session id");
            println!("/exit     Exit chat");
            continue;
        }
        if prompt == "/session" {
            if let Some(id) = &session_id {
                println!("session: {id}");
            } else {
                println!("session: <new session>");
            }
            continue;
        }

        let result = runtime
            .agent
            .ask(
                prompt,
                AgentRunOptions {
                    session_id: session_id.clone(),
                    cwd: std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?,
                    yes: cli.yes,
                    interactive: true,
                },
            )
            .await?;
        session_id = Some(result.session_id.clone());
        println!("assistant> {}", result.response.trim());
    }
    Ok(())
}

async fn handle_session(cli: &Cli, args: SessionArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    let store = SessionStore::new(paths.sessions_dir.clone());
    store.ensure_dirs()?;
    match args.command {
        SessionCommand::List => {
            let sessions = store.list_sessions()?;
            if cli.json {
                print_json(&json!({ "ok": true, "sessions": sessions }));
            } else if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                for session in sessions {
                    let last = session
                        .last_updated
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} events={} last={}",
                        session.session_id, session.event_count, last
                    );
                }
            }
        }
        SessionCommand::Show { session_id } => {
            let events = store.read_events(&session_id)?;
            if cli.json {
                print_json(&json!({ "ok": true, "session_id": session_id, "events": events }));
            } else {
                println!("Session: {session_id}");
                for event in events {
                    println!(
                        "{} {} {} {}",
                        event.ts.to_rfc3339(),
                        event.id,
                        format!("{:?}", event.kind),
                        event.payload
                    );
                }
            }
        }
        SessionCommand::Resume { session_id } => {
            handle_chat(
                cli,
                ChatArgs {
                    session: Some(session_id),
                    prompt: None,
                    agent: None,
                },
            )
            .await?;
        }
        SessionCommand::Clear { session_id, all } => {
            if all {
                let removed = store.clear_all()?;
                if cli.json {
                    print_json(&json!({ "ok": true, "removed": removed }));
                } else {
                    println!("Removed {removed} sessions.");
                }
            } else {
                let session_id = session_id.ok_or_else(|| {
                    MosaicError::Validation(
                        "session id is required unless --all is provided".to_string(),
                    )
                })?;
                store.clear_session(&session_id)?;
                if cli.json {
                    print_json(&json!({ "ok": true, "removed_session": session_id }));
                } else {
                    println!("Removed session {session_id}");
                }
            }
        }
    }
    Ok(())
}

async fn handle_gateway(cli: &Cli, args: GatewayArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let gateway_path = paths.data_dir.join("gateway.json");
    let gateway_service_path = paths.data_dir.join("gateway-service.json");
    match args.command {
        GatewayCommand::Install { host, port } => {
            let service =
                upsert_gateway_service(&gateway_service_path, Some(host), Some(port), true)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "service": service,
                    "path": gateway_service_path.display().to_string(),
                }));
            } else {
                println!("gateway service installed");
                println!("host: {}", service.host);
                println!("port: {}", service.port);
                println!("path: {}", gateway_service_path.display());
            }
        }
        GatewayCommand::Start { host, port } => {
            let (resolved_host, resolved_port) =
                resolve_gateway_start_target(&gateway_service_path, host, port, "127.0.0.1", 8787)?;
            let service = upsert_gateway_service(
                &gateway_service_path,
                Some(resolved_host.clone()),
                Some(resolved_port),
                true,
            )?;
            let start =
                start_gateway_runtime(cli, &gateway_path, resolved_host, resolved_port).await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "already_running": start.already_running,
                    "gateway": start.state,
                    "service": service,
                    "path": gateway_path.display().to_string(),
                }));
            } else if start.already_running {
                println!(
                    "Gateway already running on {}:{}",
                    start.state.host, start.state.port
                );
            } else {
                println!("Gateway is running.");
                println!("host: {}", start.state.host);
                println!("port: {}", start.state.port);
                println!("pid: {}", start.state.pid);
                println!("state: {}", gateway_path.display());
            }
        }
        GatewayCommand::Restart { host, port } => {
            let (resolved_host, resolved_port) =
                resolve_gateway_start_target(&gateway_service_path, host, port, "127.0.0.1", 8787)?;
            let service = upsert_gateway_service(
                &gateway_service_path,
                Some(resolved_host.clone()),
                Some(resolved_port),
                true,
            )?;
            let stop = stop_gateway_runtime(&gateway_path, false)?;
            let start =
                start_gateway_runtime(cli, &gateway_path, resolved_host, resolved_port).await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": stop.was_running,
                    "stopped": stop.stopped,
                    "gateway": start.state,
                    "service": service,
                    "path": gateway_path.display().to_string(),
                }));
            } else {
                println!(
                    "gateway restarted (previous_running={} stopped={})",
                    stop.was_running, stop.stopped
                );
                println!("host: {}", start.state.host);
                println!("port: {}", start.state.port);
                println!("pid: {}", start.state.pid);
            }
        }
        GatewayCommand::Status { deep } => {
            let status =
                collect_gateway_runtime_status(&gateway_path, &gateway_service_path).await?;
            if cli.json {
                let mut payload = json!({
                    "ok": true,
                    "running": status.running,
                    "installed": status
                        .service
                        .as_ref()
                        .map(|service| service.installed)
                        .unwrap_or(false),
                    "gateway": status.state,
                    "service": status.service,
                    "path": gateway_path.display().to_string(),
                    "service_path": gateway_service_path.display().to_string(),
                });
                if deep {
                    payload["deep"] = json!({
                        "process_alive": status.process_alive,
                        "endpoint_healthy": status.endpoint_healthy,
                        "target_host": status.target_host,
                        "target_port": status.target_port,
                        "state_file_exists": gateway_path.exists(),
                        "service_file_exists": gateway_service_path.exists(),
                    });
                }
                print_json(&payload);
            } else if let Some(state) = status.state {
                println!(
                    "gateway: running={} installed={} host={} port={} pid={} updated={}",
                    status.running,
                    status
                        .service
                        .as_ref()
                        .map(|service| service.installed)
                        .unwrap_or(false),
                    state.host,
                    state.port,
                    state.pid,
                    state.updated_at.to_rfc3339()
                );
                if deep {
                    println!("process_alive: {}", status.process_alive);
                    println!("endpoint_healthy: {}", status.endpoint_healthy);
                    println!("target: {}:{}", status.target_host, status.target_port);
                }
            } else {
                println!(
                    "gateway: not running (installed={})",
                    status
                        .service
                        .as_ref()
                        .map(|service| service.installed)
                        .unwrap_or(false)
                );
                if deep {
                    println!("process_alive: {}", status.process_alive);
                    println!("endpoint_healthy: {}", status.endpoint_healthy);
                    println!("target: {}:{}", status.target_host, status.target_port);
                }
            }
        }
        GatewayCommand::Health { verbose } => {
            let status =
                collect_gateway_runtime_status(&gateway_path, &gateway_service_path).await?;
            let installed = status
                .service
                .as_ref()
                .map(|service| service.installed)
                .unwrap_or(false);
            let mut checks = vec![
                run_check(
                    "gateway_service_file",
                    gateway_service_path.exists(),
                    "gateway service file",
                ),
                run_check(
                    "gateway_installed",
                    installed,
                    if installed {
                        "gateway service installed"
                    } else {
                        "gateway service not installed"
                    },
                ),
                run_check(
                    "gateway_state_file",
                    gateway_path.exists(),
                    "gateway runtime state file",
                ),
                run_check(
                    "gateway_process",
                    status.process_alive,
                    if status.process_alive {
                        "gateway process is alive"
                    } else {
                        "gateway process is not alive"
                    },
                ),
                run_check(
                    "gateway_endpoint",
                    status.endpoint_healthy,
                    if status.endpoint_healthy {
                        "GET /health reachable"
                    } else {
                        "GET /health unreachable"
                    },
                ),
            ];
            if let Some(state) = status.state {
                checks.push(run_check(
                    "gateway_target",
                    true,
                    format!("{}:{} (pid={})", state.host, state.port, state.pid),
                ));
            } else {
                checks.push(run_check(
                    "gateway_target",
                    installed,
                    format!("{}:{}", status.target_host, status.target_port),
                ));
            }
            if verbose {
                checks.push(run_check(
                    "gateway_runtime_running",
                    status.running,
                    format!("running={}", status.running),
                ));
            }
            emit_checks(cli.json, "gateway_health", checks)?;
            if verbose && !cli.json {
                println!(
                    "target endpoint: http://{}:{}",
                    status.target_host, status.target_port
                );
            }
        }
        GatewayCommand::Call { method, params } => {
            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    return Err(MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    ));
                }
                let params = params
                    .as_deref()
                    .map(|value| parse_json_input(value, "gateway params"))
                    .transpose()?
                    .unwrap_or(Value::Null);
                let data = match method.as_str() {
                    "status" => json!({
                        "ok": true,
                        "service": "mosaic-gateway",
                        "test_mode": true,
                    }),
                    "health" => json!({
                        "ok": true,
                        "service": "mosaic-gateway",
                        "test_mode": true,
                    }),
                    "echo" => json!({
                        "ok": true,
                        "echo": params,
                        "test_mode": true,
                    }),
                    "nodes.run" => json!({
                        "ok": true,
                        "status": "accepted",
                        "mode": "test_mode",
                        "node_id": params.get("node_id").cloned().unwrap_or(Value::Null),
                        "command": params.get("command").cloned().unwrap_or(Value::Null),
                    }),
                    "nodes.invoke" => json!({
                        "ok": true,
                        "status": "accepted",
                        "mode": "test_mode",
                        "node_id": params.get("node_id").cloned().unwrap_or(Value::Null),
                        "method": params.get("method").cloned().unwrap_or(Value::Null),
                        "params": params.get("params").cloned().unwrap_or(Value::Null),
                    }),
                    _ => {
                        return Err(MosaicError::GatewayProtocol(format!(
                            "unknown gateway method '{}' in test mode",
                            method
                        )));
                    }
                };
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "request_id": "gateway-test-mode",
                        "method": method,
                        "data": data,
                        "gateway": { "host": "127.0.0.1", "port": 8787 },
                    }));
                } else {
                    println!("gateway method: {method}");
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&data).unwrap_or_default()
                    );
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let params = params
                .as_deref()
                .map(|value| parse_json_input(value, "gateway params"))
                .transpose()?;
            let request = GatewayRequest::new(method.clone(), params);
            let request_id = request.id.clone();
            let response = client.call(request).await?;
            let result = response.result.unwrap_or(Value::Null);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request_id": request_id,
                    "method": method,
                    "data": result,
                    "gateway": {
                        "host": host,
                        "port": port,
                    }
                }));
            } else {
                println!("gateway method: {method}");
                println!("request id: {request_id}");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
                );
            }
        }
        GatewayCommand::Probe => {
            if gateway_test_mode() {
                let state: Option<GatewayState> = load_json_file_opt(&gateway_path)?;
                if !state.as_ref().is_some_and(|value| value.running) {
                    return Err(MosaicError::GatewayUnavailable(
                        "gateway is not running in test mode".to_string(),
                    ));
                }
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "probe": {
                            "ok": true,
                            "endpoint": "test-mode://gateway/health",
                            "latency_ms": 0,
                            "detail": "gateway test mode",
                        },
                        "gateway": { "host": "127.0.0.1", "port": 8787 },
                    }));
                } else {
                    println!("gateway probe ok (test mode)");
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let probe = client.probe().await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "probe": probe,
                    "gateway": {
                        "host": host,
                        "port": port,
                    }
                }));
            } else {
                println!("gateway probe ok");
                println!("endpoint: {}", probe.endpoint);
                println!("latency: {}ms", probe.latency_ms);
                println!("detail: {}", probe.detail);
            }
        }
        GatewayCommand::Discover => {
            if gateway_test_mode() {
                let methods = vec!["health", "status", "echo", "nodes.run", "nodes.invoke"];
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "discovery": {
                            "ok": true,
                            "endpoint": "test-mode://gateway/discover",
                            "methods": methods,
                        },
                        "gateway": { "host": "127.0.0.1", "port": 8787 },
                    }));
                } else {
                    println!("gateway methods:");
                    println!("- health");
                    println!("- status");
                    println!("- echo");
                    println!("- nodes.run");
                    println!("- nodes.invoke");
                }
                return Ok(());
            }

            let (host, port) = resolve_gateway_target(&gateway_path, &gateway_service_path)?;
            let client = HttpGatewayClient::new(&host, port)?;
            let discovery = client.discover().await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "discovery": discovery,
                    "gateway": {
                        "host": host,
                        "port": port,
                    }
                }));
            } else if discovery.methods.is_empty() {
                println!("gateway methods: <none>");
            } else {
                println!("gateway methods:");
                for method in discovery.methods {
                    println!("- {method}");
                }
            }
        }
        GatewayCommand::Stop => {
            let stop = stop_gateway_runtime(&gateway_path, true)?;
            let next = stop.state.ok_or_else(|| {
                MosaicError::Config("gateway state file not found; not running".to_string())
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": stop.was_running,
                    "stopped": stop.stopped,
                    "gateway": next,
                }));
            } else if stop.was_running {
                println!(
                    "Gateway {} (pid={})",
                    if stop.stopped {
                        "stopped"
                    } else {
                        "stop signal sent"
                    },
                    next.pid
                );
            } else {
                println!("Gateway process was not running.");
            }
        }
        GatewayCommand::Uninstall => {
            let stop = stop_gateway_runtime(&gateway_path, false)?;
            let removed_state_file = if gateway_path.exists() {
                std::fs::remove_file(&gateway_path)?;
                true
            } else {
                false
            };
            let removed_service_file = if gateway_service_path.exists() {
                std::fs::remove_file(&gateway_service_path)?;
                true
            } else {
                false
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "was_running": stop.was_running,
                    "stopped": stop.stopped,
                    "removed_state_file": removed_state_file,
                    "removed_service_file": removed_service_file,
                }));
            } else {
                println!(
                    "gateway uninstalled (was_running={} stopped={} removed_state={} removed_service={})",
                    stop.was_running, stop.stopped, removed_state_file, removed_service_file
                );
            }
        }
        GatewayCommand::Serve { host, port } => {
            run_gateway_http_server(&host, port)?;
        }
    }
    Ok(())
}

async fn handle_nodes(cli: &Cli, args: NodesArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let nodes_path = nodes_file_path(&paths.data_dir);
    let devices_path = devices_file_path(&paths.data_dir);
    let pairings_path = pairing_requests_file_path(&paths.data_dir);
    let mut nodes = load_nodes_or_default(&nodes_path)?;

    match args.command {
        NodesCommand::List => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "nodes": nodes,
                    "path": nodes_path.display().to_string(),
                }));
            } else {
                for node in nodes {
                    println!(
                        "{} name={} status={:?} capabilities={} last_seen={}",
                        node.id,
                        node.name,
                        node.status,
                        if node.capabilities.is_empty() {
                            "-".to_string()
                        } else {
                            node.capabilities.join(",")
                        },
                        node.last_seen_at.to_rfc3339()
                    );
                }
            }
        }
        NodesCommand::Status { node_id } => {
            let devices = load_devices_or_default(&devices_path)?;
            let pairings = load_pairing_requests_or_default(&pairings_path)?;
            if let Some(node_id) = node_id {
                let node = nodes
                    .iter()
                    .find(|item| item.id == node_id)
                    .ok_or_else(|| {
                        MosaicError::Validation(format!("node '{}' not found", node_id))
                    })?;
                let node_pairings = pairings
                    .iter()
                    .filter(|item| item.node_id == node.id)
                    .collect::<Vec<_>>();
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "node": node,
                        "pairings": {
                            "total": node_pairings.len(),
                            "pending": node_pairings
                                .iter()
                                .filter(|item| item.status == PairingStatus::Pending)
                                .count(),
                        },
                        "approved_devices": devices
                            .iter()
                            .filter(|item| item.status == DeviceStatus::Approved)
                            .count(),
                    }));
                } else {
                    println!("node: {} ({})", node.id, node.name);
                    println!("status: {:?}", node.status);
                    println!(
                        "capabilities: {}",
                        if node.capabilities.is_empty() {
                            "-".to_string()
                        } else {
                            node.capabilities.join(",")
                        }
                    );
                    println!("last seen: {}", node.last_seen_at.to_rfc3339());
                    println!(
                        "pairings: total={} pending={}",
                        node_pairings.len(),
                        node_pairings
                            .iter()
                            .filter(|item| item.status == PairingStatus::Pending)
                            .count()
                    );
                }
            } else if cli.json {
                let summary = json!({
                    "total": nodes.len(),
                    "online": nodes
                        .iter()
                        .filter(|item| item.status == NodeRuntimeStatus::Online)
                        .count(),
                    "approved_devices": devices
                        .iter()
                        .filter(|item| item.status == DeviceStatus::Approved)
                        .count(),
                    "pending_pairings": pairings
                        .iter()
                        .filter(|item| item.status == PairingStatus::Pending)
                        .count(),
                });
                print_json(&json!({
                    "ok": true,
                    "summary": summary,
                    "nodes": nodes,
                }));
            } else {
                println!("nodes total: {}", nodes.len());
                println!(
                    "online: {}",
                    nodes
                        .iter()
                        .filter(|item| item.status == NodeRuntimeStatus::Online)
                        .count()
                );
            }
        }
        NodesCommand::Run { node_id, command } => {
            let now = Utc::now();
            let run_id = format!("run-{}-{}", now.timestamp_millis(), next_pairing_seq());
            let node = nodes
                .iter_mut()
                .find(|item| item.id == node_id)
                .ok_or_else(|| MosaicError::Validation(format!("node '{}' not found", node_id)))?;
            node.last_seen_at = now;
            node.updated_at = now;
            let node_id = node.id.clone();
            save_nodes(&nodes_path, &nodes)?;

            let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
            let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
            let approval_policy = approval_store.load_or_default()?;
            let sandbox_policy = sandbox_store.load_or_default()?;
            if let Some(reason) = evaluate_sandbox(&command, sandbox_policy.profile) {
                return Err(MosaicError::SandboxDenied(reason));
            }
            let approved_by = match evaluate_approval(&command, &approval_policy) {
                ApprovalDecision::Auto { approved_by } => approved_by,
                ApprovalDecision::NeedsConfirmation { reason } => {
                    if cli.yes {
                        "flag_yes".to_string()
                    } else {
                        return Err(MosaicError::ApprovalRequired(format!(
                            "{reason}. rerun with --yes"
                        )));
                    }
                }
                ApprovalDecision::Deny { reason } => {
                    return Err(MosaicError::ApprovalRequired(reason));
                }
            };
            let gateway_path = paths.data_dir.join("gateway.json");
            let gateway_service_path = paths.data_dir.join("gateway-service.json");
            let gateway = dispatch_gateway_call(
                &gateway_path,
                &gateway_service_path,
                "nodes.run",
                json!({
                    "node_id": node_id.clone(),
                    "command": command.clone(),
                    "approved_by": approved_by.clone(),
                }),
            )
            .await?;
            let accepted = gateway
                .result
                .get("ok")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let status = gateway
                .result
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or(if accepted { "accepted" } else { "failed" });

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "run_id": run_id,
                    "node_id": node_id.clone(),
                    "accepted": accepted,
                    "status": status,
                    "gateway": {
                        "host": gateway.host,
                        "port": gateway.port,
                        "request_id": gateway.request_id,
                    },
                    "result": gateway.result,
                }));
            } else {
                println!("run submitted");
                println!("run id: {run_id}");
                println!("node: {}", node_id);
                println!("status: {status}");
                println!(
                    "gateway: {}:{} request_id={}",
                    gateway.host, gateway.port, gateway.request_id
                );
            }
        }
        NodesCommand::Invoke {
            node_id,
            method,
            params,
        } => {
            let now = Utc::now();
            let invoke_id = format!("invoke-{}-{}", now.timestamp_millis(), next_pairing_seq());
            let node = nodes
                .iter_mut()
                .find(|item| item.id == node_id)
                .ok_or_else(|| MosaicError::Validation(format!("node '{}' not found", node_id)))?;
            node.last_seen_at = now;
            node.updated_at = now;
            let node_id = node.id.clone();
            save_nodes(&nodes_path, &nodes)?;

            let parsed_params = params
                .as_deref()
                .map(|value| parse_json_input(value, "invoke params"))
                .transpose()?
                .unwrap_or(Value::Null);
            let gateway_path = paths.data_dir.join("gateway.json");
            let gateway_service_path = paths.data_dir.join("gateway-service.json");
            let gateway = dispatch_gateway_call(
                &gateway_path,
                &gateway_service_path,
                "nodes.invoke",
                json!({
                    "node_id": node_id.clone(),
                    "method": method.clone(),
                    "params": parsed_params.clone(),
                }),
            )
            .await?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "invoke_id": invoke_id,
                    "node_id": node_id.clone(),
                    "method": method.clone(),
                    "gateway": {
                        "host": gateway.host,
                        "port": gateway.port,
                        "request_id": gateway.request_id,
                    },
                    "result": gateway.result,
                }));
            } else {
                println!("invoke accepted");
                println!("invoke id: {invoke_id}");
                println!("node: {}", node_id);
                println!("method: {method}");
                println!(
                    "gateway: {}:{} request_id={}",
                    gateway.host, gateway.port, gateway.request_id
                );
            }
        }
    }
    Ok(())
}

fn handle_devices(cli: &Cli, args: DevicesArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let devices_path = devices_file_path(&paths.data_dir);
    let mut devices = load_devices_or_default(&devices_path)?;

    match args.command {
        DevicesCommand::List => {
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "devices": devices,
                    "path": devices_path.display().to_string(),
                }));
            } else if devices.is_empty() {
                println!("No devices found.");
            } else {
                for device in devices {
                    println!(
                        "{} name={} status={:?} token_v={} last_seen={} last_error={}",
                        device.id,
                        device.name,
                        device.status,
                        device.token_version,
                        device.last_seen_at.to_rfc3339(),
                        device.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        DevicesCommand::Approve { device_id, name } => {
            let now = Utc::now();
            let fingerprint = format!("fp-{}-{}", now.timestamp_millis(), next_pairing_seq());
            let device =
                if let Some(existing) = devices.iter_mut().find(|item| item.id == device_id) {
                    existing.status = DeviceStatus::Approved;
                    if let Some(name) = name {
                        existing.name = name.trim().to_string();
                    }
                    existing.updated_at = now;
                    existing.last_seen_at = now;
                    existing.last_error = None;
                    existing.clone()
                } else {
                    let device = DeviceRecord {
                        id: device_id.clone(),
                        name: name.unwrap_or_else(|| device_id.clone()),
                        fingerprint,
                        status: DeviceStatus::Approved,
                        token_version: 1,
                        last_seen_at: now,
                        updated_at: now,
                        last_error: None,
                    };
                    devices.push(device.clone());
                    device
                };
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!("device approved: {}", device.id);
            }
        }
        DevicesCommand::Reject { device_id, reason } => {
            let now = Utc::now();
            let device = devices
                .iter_mut()
                .find(|item| item.id == device_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("device '{}' not found", device_id))
                })?;
            device.status = DeviceStatus::Rejected;
            device.last_error = reason.clone();
            device.updated_at = now;
            let device = device.clone();
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!("device rejected: {}", device.id);
            }
        }
        DevicesCommand::Rotate { device_id } => {
            let now = Utc::now();
            let device = devices
                .iter_mut()
                .find(|item| item.id == device_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("device '{}' not found", device_id))
                })?;
            if device.status != DeviceStatus::Approved {
                return Err(MosaicError::Validation(format!(
                    "device '{}' must be approved before rotate",
                    device_id
                )));
            }
            device.token_version = device.token_version.saturating_add(1);
            device.updated_at = now;
            device.last_seen_at = now;
            let device = device.clone();
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!(
                    "device rotated: {} token_v={}",
                    device.id, device.token_version
                );
            }
        }
        DevicesCommand::Revoke { device_id, reason } => {
            let now = Utc::now();
            let device = devices
                .iter_mut()
                .find(|item| item.id == device_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("device '{}' not found", device_id))
                })?;
            device.status = DeviceStatus::Revoked;
            device.last_error = reason.clone();
            device.updated_at = now;
            let device = device.clone();
            save_devices(&devices_path, &devices)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "device": device,
                }));
            } else {
                println!("device revoked: {}", device.id);
            }
        }
    }
    Ok(())
}

fn handle_pairing(cli: &Cli, args: PairingArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let pairings_path = pairing_requests_file_path(&paths.data_dir);
    let devices_path = devices_file_path(&paths.data_dir);
    let nodes_path = nodes_file_path(&paths.data_dir);
    let mut pairings = load_pairing_requests_or_default(&pairings_path)?;
    let mut devices = load_devices_or_default(&devices_path)?;
    let mut nodes = load_nodes_or_default(&nodes_path)?;

    match args.command {
        PairingCommand::List { status } => {
            let filtered = if let Some(status) = status {
                let status: PairingStatus = status.into();
                pairings
                    .into_iter()
                    .filter(|item| item.status == status)
                    .collect::<Vec<_>>()
            } else {
                pairings
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "requests": filtered,
                    "path": pairings_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No pairing requests.");
            } else {
                for request in filtered {
                    println!(
                        "{} device={} node={} status={:?} requested={}",
                        request.id,
                        request.device_id,
                        request.node_id,
                        request.status,
                        request.requested_at.to_rfc3339()
                    );
                }
            }
        }
        PairingCommand::Approve { request_id } => {
            let now = Utc::now();
            let request = pairings
                .iter_mut()
                .find(|item| item.id == request_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("pairing request '{}' not found", request_id))
                })?;
            if request.status != PairingStatus::Pending {
                return Err(MosaicError::Validation(format!(
                    "pairing request '{}' is not pending",
                    request_id
                )));
            }
            request.status = PairingStatus::Approved;
            request.updated_at = now;
            let request_device_id = request.device_id.clone();
            let request_node_id = request.node_id.clone();

            let device = if let Some(device) =
                devices.iter_mut().find(|item| item.id == request_device_id)
            {
                device.status = DeviceStatus::Approved;
                device.updated_at = now;
                device.last_seen_at = now;
                device.last_error = None;
                device.clone()
            } else {
                let device = DeviceRecord {
                    id: request_device_id.clone(),
                    name: request_device_id.clone(),
                    fingerprint: format!("fp-{}-{}", now.timestamp_millis(), next_pairing_seq()),
                    status: DeviceStatus::Approved,
                    token_version: 1,
                    last_seen_at: now,
                    updated_at: now,
                    last_error: None,
                };
                devices.push(device.clone());
                device
            };

            let node = nodes
                .iter_mut()
                .find(|item| item.id == request_node_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("node '{}' not found", request_node_id))
                })?;
            node.status = NodeRuntimeStatus::Online;
            node.last_seen_at = now;
            node.updated_at = now;
            let request = request.clone();

            save_pairing_requests(&pairings_path, &pairings)?;
            save_devices(&devices_path, &devices)?;
            save_nodes(&nodes_path, &nodes)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request": request,
                    "device": device,
                }));
            } else {
                println!("pairing approved: {}", request.id);
                println!("device: {}", device.id);
            }
        }
        PairingCommand::Request {
            device,
            node,
            reason,
        } => {
            let now = Utc::now();
            if !nodes.iter().any(|item| item.id == node) {
                return Err(MosaicError::Validation(format!(
                    "node '{}' not found",
                    node
                )));
            }
            if !devices.iter().any(|item| item.id == device) {
                devices.push(DeviceRecord {
                    id: device.clone(),
                    name: device.clone(),
                    fingerprint: format!("fp-{}-{}", now.timestamp_millis(), next_pairing_seq()),
                    status: DeviceStatus::Pending,
                    token_version: 1,
                    last_seen_at: now,
                    updated_at: now,
                    last_error: None,
                });
                save_devices(&devices_path, &devices)?;
            }
            let request = PairingRequestRecord {
                id: generate_pairing_request_id(),
                device_id: device,
                node_id: node,
                status: PairingStatus::Pending,
                reason,
                requested_at: now,
                updated_at: now,
            };
            pairings.push(request.clone());
            save_pairing_requests(&pairings_path, &pairings)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "request": request,
                }));
            } else {
                println!("pairing request created: {}", request.id);
            }
        }
    }
    Ok(())
}

fn handle_hooks(cli: &Cli, args: HooksArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let hooks_path = hooks_file_path(&paths.data_dir);
    let mut hooks = load_hooks_or_default(&hooks_path)?;
    match args.command {
        HooksCommand::List { event } => {
            let event_filter = event.map(|value| value.trim().to_string());
            let filtered = hooks
                .into_iter()
                .filter(|hook| {
                    if let Some(filter) = &event_filter {
                        hook.event == *filter
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hooks": filtered,
                    "path": hooks_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No hooks configured.");
            } else {
                for hook in filtered {
                    let status = if hook.enabled { "enabled" } else { "disabled" };
                    println!(
                        "{} [{}] {} -> {}",
                        hook.id, status, hook.event, hook.command
                    );
                }
            }
        }
        HooksCommand::Add {
            name,
            event,
            command,
            disabled,
        } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MosaicError::Validation(
                    "hook name cannot be empty".to_string(),
                ));
            }
            let event = event.trim().to_string();
            if event.is_empty() {
                return Err(MosaicError::Validation(
                    "hook event cannot be empty".to_string(),
                ));
            }
            let command = command.trim().to_string();
            if command.is_empty() {
                return Err(MosaicError::Validation(
                    "hook command cannot be empty".to_string(),
                ));
            }
            let now = Utc::now();
            let hook = HookRecord {
                id: generate_hook_id(),
                name,
                event,
                command,
                enabled: !disabled,
                created_at: now,
                updated_at: now,
                last_triggered_at: None,
                last_result: None,
            };
            hooks.push(hook.clone());
            save_hooks(&hooks_path, &hooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "path": hooks_path.display().to_string(),
                }));
            } else {
                println!("hook added: {}", hook.id);
                println!("event: {}", hook.event);
                println!("enabled: {}", hook.enabled);
            }
        }
        HooksCommand::Remove { hook_id } => {
            let before = hooks.len();
            hooks.retain(|hook| hook.id != hook_id);
            let removed = hooks.len() != before;
            if removed {
                save_hooks(&hooks_path, &hooks)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "hook_id": hook_id,
                }));
            } else if removed {
                println!("removed hook {hook_id}");
            } else {
                println!("hook {hook_id} not found");
            }
        }
        HooksCommand::Enable { hook_id } => {
            let hook = hooks
                .iter_mut()
                .find(|item| item.id == hook_id)
                .ok_or_else(|| MosaicError::Validation(format!("hook '{}' not found", hook_id)))?;
            hook.enabled = true;
            hook.updated_at = Utc::now();
            let hook = hook.clone();
            save_hooks(&hooks_path, &hooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "path": hooks_path.display().to_string(),
                }));
            } else {
                println!("hook enabled: {}", hook.id);
            }
        }
        HooksCommand::Disable { hook_id } => {
            let hook = hooks
                .iter_mut()
                .find(|item| item.id == hook_id)
                .ok_or_else(|| MosaicError::Validation(format!("hook '{}' not found", hook_id)))?;
            hook.enabled = false;
            hook.updated_at = Utc::now();
            let hook = hook.clone();
            save_hooks(&hooks_path, &hooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "path": hooks_path.display().to_string(),
                }));
            } else {
                println!("hook disabled: {}", hook.id);
            }
        }
        HooksCommand::Run { hook_id, data } => {
            let hook = hooks
                .iter()
                .find(|item| item.id == hook_id)
                .ok_or_else(|| MosaicError::Validation(format!("hook '{}' not found", hook_id)))?
                .clone();
            let payload = data
                .as_deref()
                .map(|value| parse_json_input(value, "hook data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let report = execute_hook_command(cli, &paths, &hook, "manual", payload)?;
            apply_hook_last_result(&mut hooks, &hook.id, &report);
            save_hooks(&hooks_path, &hooks)?;
            if !report.ok {
                return Err(hook_execution_error(&hook.id, &report));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "hook": hook,
                    "result": report,
                }));
            } else {
                println!("hook executed: {}", hook.id);
                println!("status: success");
                if let Some(code) = report.exit_code {
                    println!("exit_code: {code}");
                }
            }
        }
        HooksCommand::Logs { hook, tail } => {
            let events = read_hook_events(&paths.data_dir, hook.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No hook events found.");
            } else {
                for item in events {
                    let status = if item.ok { "ok" } else { "error" };
                    let code = item
                        .exit_code
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} [{}] hook={} event={} trigger={} code={} msg={}",
                        item.ts.to_rfc3339(),
                        status,
                        item.hook_id,
                        item.event,
                        item.trigger,
                        code,
                        item.error.unwrap_or_default(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn handle_cron(cli: &Cli, args: CronArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let jobs_path = cron_jobs_file_path(&paths.data_dir);
    let mut jobs = load_cron_jobs_or_default(&jobs_path)?;
    match args.command {
        CronCommand::List { event } => {
            let event_filter = event.map(|value| value.trim().to_string());
            let filtered = jobs
                .into_iter()
                .filter(|job| {
                    if let Some(filter) = &event_filter {
                        job.event == *filter
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "jobs": filtered,
                    "path": jobs_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No cron jobs configured.");
            } else {
                for job in filtered {
                    let status = if job.enabled { "enabled" } else { "disabled" };
                    println!(
                        "{} [{}] every={}s event={} next={} name={}",
                        job.id,
                        status,
                        job.every_seconds,
                        job.event,
                        job.next_run_at.to_rfc3339(),
                        job.name
                    );
                }
            }
        }
        CronCommand::Add {
            name,
            event,
            every,
            data,
            disabled,
        } => {
            if every == 0 {
                return Err(MosaicError::Validation(
                    "--every must be greater than 0 seconds".to_string(),
                ));
            }
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MosaicError::Validation(
                    "cron job name cannot be empty".to_string(),
                ));
            }
            let event = event.trim().to_string();
            if event.is_empty() {
                return Err(MosaicError::Validation(
                    "cron event cannot be empty".to_string(),
                ));
            }
            let data = data
                .as_deref()
                .map(|value| parse_json_input(value, "cron data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let now = Utc::now();
            let job = CronJobRecord {
                id: generate_cron_job_id(),
                name,
                event,
                every_seconds: every,
                data,
                enabled: !disabled,
                created_at: now,
                updated_at: now,
                last_run_at: None,
                next_run_at: now,
                run_count: 0,
                last_result: None,
            };
            jobs.push(job.clone());
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": job,
                    "path": jobs_path.display().to_string(),
                }));
            } else {
                println!("cron job added: {}", job.id);
                println!("event: {}", job.event);
                println!("every: {}s", job.every_seconds);
                println!("enabled: {}", job.enabled);
            }
        }
        CronCommand::Remove { job_id } => {
            let before = jobs.len();
            jobs.retain(|job| job.id != job_id);
            let removed = jobs.len() != before;
            if removed {
                save_cron_jobs(&jobs_path, &jobs)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "job_id": job_id,
                }));
            } else if removed {
                println!("removed cron job {job_id}");
            } else {
                println!("cron job {job_id} not found");
            }
        }
        CronCommand::Enable { job_id } => {
            let job = jobs
                .iter_mut()
                .find(|item| item.id == job_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("cron job '{}' not found", job_id))
                })?;
            job.enabled = true;
            job.updated_at = Utc::now();
            let job = job.clone();
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": job,
                    "path": jobs_path.display().to_string(),
                }));
            } else {
                println!("cron job enabled: {}", job.id);
            }
        }
        CronCommand::Disable { job_id } => {
            let job = jobs
                .iter_mut()
                .find(|item| item.id == job_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("cron job '{}' not found", job_id))
                })?;
            job.enabled = false;
            job.updated_at = Utc::now();
            let job = job.clone();
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": job,
                    "path": jobs_path.display().to_string(),
                }));
            } else {
                println!("cron job disabled: {}", job.id);
            }
        }
        CronCommand::Run { job_id, data } => {
            let index = jobs
                .iter()
                .position(|item| item.id == job_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("cron job '{}' not found", job_id))
                })?;
            let payload_override = data
                .as_deref()
                .map(|value| parse_json_input(value, "cron data"))
                .transpose()?;
            let snapshot = jobs[index].clone();
            let report = execute_cron_job(cli, &paths, &snapshot, "manual", payload_override)?;
            apply_cron_result(&mut jobs[index], &report, Utc::now())?;
            save_cron_jobs(&jobs_path, &jobs)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "job": jobs[index].clone(),
                    "result": report,
                }));
            } else {
                println!("cron job executed: {}", jobs[index].id);
                println!("ok: {}", report.ok);
                if let Some(error) = report.error {
                    println!("error: {error}");
                }
            }
        }
        CronCommand::Tick { limit } => {
            if limit == 0 {
                return Err(MosaicError::Validation(
                    "--limit must be greater than 0".to_string(),
                ));
            }
            let now = Utc::now();
            let mut due = jobs
                .iter()
                .enumerate()
                .filter(|(_, job)| job.enabled && job.next_run_at <= now)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            due.sort_by_key(|index| jobs[*index].next_run_at);
            let mut reports = Vec::new();
            for index in due.into_iter().take(limit) {
                let snapshot = jobs[index].clone();
                let report = execute_cron_job(cli, &paths, &snapshot, "tick", None)?;
                apply_cron_result(&mut jobs[index], &report, Utc::now())?;
                reports.push(report);
            }
            if !reports.is_empty() {
                save_cron_jobs(&jobs_path, &jobs)?;
            }
            let ok_count = reports.iter().filter(|item| item.ok).count();
            let failed_count = reports.len().saturating_sub(ok_count);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "triggered": reports.len(),
                    "ok_count": ok_count,
                    "failed_count": failed_count,
                    "results": reports,
                }));
            } else {
                println!("cron tick triggered: {}", reports.len());
                println!("ok: {ok_count}");
                println!("failed: {failed_count}");
            }
        }
        CronCommand::Logs { job, tail } => {
            let events = read_cron_events(&paths.data_dir, job.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No cron events found.");
            } else {
                for item in events {
                    let status = if item.ok { "ok" } else { "error" };
                    println!(
                        "{} [{}] job={} trigger={} event={} hooks={}/{} error={}",
                        item.ts.to_rfc3339(),
                        status,
                        item.job_id,
                        item.trigger,
                        item.event,
                        item.hooks_ok,
                        item.hooks_triggered,
                        item.error.unwrap_or_default(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn handle_webhooks(cli: &Cli, args: WebhooksArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let webhooks_path = webhooks_file_path(&paths.data_dir);
    let mut webhooks = load_webhooks_or_default(&webhooks_path)?;
    match args.command {
        WebhooksCommand::List { event } => {
            let event_filter = event.map(|value| value.trim().to_string());
            let filtered = webhooks
                .into_iter()
                .filter(|webhook| {
                    if let Some(filter) = &event_filter {
                        webhook.event == *filter
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhooks": filtered,
                    "path": webhooks_path.display().to_string(),
                }));
            } else if filtered.is_empty() {
                println!("No webhooks configured.");
            } else {
                for webhook in filtered {
                    let status = if webhook.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    println!(
                        "{} [{}] {} {} -> {} ({})",
                        webhook.id,
                        status,
                        webhook.method,
                        webhook.path,
                        webhook.event,
                        webhook.name
                    );
                }
            }
        }
        WebhooksCommand::Add {
            name,
            event,
            path,
            method,
            secret_env,
            disabled,
        } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(MosaicError::Validation(
                    "webhook name cannot be empty".to_string(),
                ));
            }
            let event = event.trim().to_string();
            if event.is_empty() {
                return Err(MosaicError::Validation(
                    "webhook event cannot be empty".to_string(),
                ));
            }
            let path = normalize_webhook_path(&path)?;
            let secret_env = normalize_optional_secret_env(secret_env)?;
            let now = Utc::now();
            let webhook = WebhookRecord {
                id: generate_webhook_id(),
                name,
                event,
                path,
                method: method.into(),
                secret_env,
                enabled: !disabled,
                created_at: now,
                updated_at: now,
                last_triggered_at: None,
                last_result: None,
            };
            webhooks.push(webhook.clone());
            save_webhooks(&webhooks_path, &webhooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhook,
                    "path": webhooks_path.display().to_string(),
                }));
            } else {
                println!("webhook added: {}", webhook.id);
                println!("route: {} {}", webhook.method, webhook.path);
                println!("event: {}", webhook.event);
            }
        }
        WebhooksCommand::Remove { webhook_id } => {
            let before = webhooks.len();
            webhooks.retain(|item| item.id != webhook_id);
            let removed = webhooks.len() != before;
            if removed {
                save_webhooks(&webhooks_path, &webhooks)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "webhook_id": webhook_id,
                }));
            } else if removed {
                println!("removed webhook {webhook_id}");
            } else {
                println!("webhook {webhook_id} not found");
            }
        }
        WebhooksCommand::Enable { webhook_id } => {
            let webhook = webhooks
                .iter_mut()
                .find(|item| item.id == webhook_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("webhook '{}' not found", webhook_id))
                })?;
            webhook.enabled = true;
            webhook.updated_at = Utc::now();
            let webhook = webhook.clone();
            save_webhooks(&webhooks_path, &webhooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhook,
                    "path": webhooks_path.display().to_string(),
                }));
            } else {
                println!("webhook enabled: {}", webhook.id);
            }
        }
        WebhooksCommand::Disable { webhook_id } => {
            let webhook = webhooks
                .iter_mut()
                .find(|item| item.id == webhook_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("webhook '{}' not found", webhook_id))
                })?;
            webhook.enabled = false;
            webhook.updated_at = Utc::now();
            let webhook = webhook.clone();
            save_webhooks(&webhooks_path, &webhooks)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhook,
                    "path": webhooks_path.display().to_string(),
                }));
            } else {
                println!("webhook disabled: {}", webhook.id);
            }
        }
        WebhooksCommand::Trigger {
            webhook_id,
            data,
            secret,
        } => {
            let index = webhooks
                .iter()
                .position(|item| item.id == webhook_id)
                .ok_or_else(|| {
                    MosaicError::Validation(format!("webhook '{}' not found", webhook_id))
                })?;
            let payload = data
                .as_deref()
                .map(|value| parse_json_input(value, "webhook data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let snapshot = webhooks[index].clone();
            let report =
                execute_webhook(cli, &paths, &snapshot, "manual", payload, secret.as_deref())?;
            apply_webhook_last_result(&mut webhooks[index], &report);
            save_webhooks(&webhooks_path, &webhooks)?;
            if !report.ok {
                return Err(webhook_execution_error(&snapshot.id, &report));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhooks[index].clone(),
                    "result": report,
                }));
            } else {
                println!("webhook triggered: {}", snapshot.id);
                println!("event: {}", snapshot.event);
            }
        }
        WebhooksCommand::Resolve {
            path,
            method,
            data,
            secret,
        } => {
            let path = normalize_webhook_path(&path)?;
            let method: WebhookMethod = method.into();
            let index = webhooks
                .iter()
                .position(|item| item.enabled && item.path == path && item.method == method)
                .ok_or_else(|| {
                    MosaicError::Validation(format!(
                        "no enabled webhook matched {} {}",
                        method, path
                    ))
                })?;
            let payload = data
                .as_deref()
                .map(|value| parse_json_input(value, "webhook data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let snapshot = webhooks[index].clone();
            let report = execute_webhook(
                cli,
                &paths,
                &snapshot,
                "resolve",
                payload,
                secret.as_deref(),
            )?;
            apply_webhook_last_result(&mut webhooks[index], &report);
            save_webhooks(&webhooks_path, &webhooks)?;
            if !report.ok {
                return Err(webhook_execution_error(&snapshot.id, &report));
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "webhook": webhooks[index].clone(),
                    "result": report,
                }));
            } else {
                println!("webhook resolved: {}", snapshot.id);
                println!("route: {} {}", snapshot.method, snapshot.path);
                println!("event: {}", snapshot.event);
            }
        }
        WebhooksCommand::Logs { webhook, tail } => {
            let events = read_webhook_events(&paths.data_dir, webhook.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                }));
            } else if events.is_empty() {
                println!("No webhook events found.");
            } else {
                for event in events {
                    let status = if event.ok { "ok" } else { "error" };
                    println!(
                        "{} [{}] webhook={} trigger={} route={} {} event={} hooks={}/{} error={}",
                        event.ts.to_rfc3339(),
                        status,
                        event.webhook_id,
                        event.trigger,
                        event.method,
                        event.path,
                        event.event,
                        event.hooks_ok,
                        event.hooks_triggered,
                        event.error.unwrap_or_default(),
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_browser(cli: &Cli, args: BrowserArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let history_path = browser_history_file_path(&paths.data_dir);
    let mut history = load_browser_history_or_default(&history_path)?;
    match args.command {
        BrowserCommand::Open { url, timeout_ms } => {
            if timeout_ms == 0 {
                return Err(MosaicError::Validation(
                    "--timeout-ms must be greater than 0".to_string(),
                ));
            }
            let visit = browser_open_visit(&url, timeout_ms).await?;
            history.push(visit.clone());
            save_browser_history(&history_path, &history)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visit": visit,
                    "path": history_path.display().to_string(),
                }));
            } else {
                let status = visit
                    .http_status
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!("browser open: {}", visit.url);
                println!("visit id: {}", visit.id);
                println!("ok: {}", visit.ok);
                println!("status: {status}");
                if let Some(title) = visit.title {
                    println!("title: {title}");
                }
                if let Some(error) = visit.error {
                    println!("error: {error}");
                }
            }
        }
        BrowserCommand::History { tail } => {
            let mut visits = history;
            visits.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
            if visits.len() > tail {
                let keep_from = visits.len() - tail;
                visits = visits.split_off(keep_from);
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visits": visits,
                    "path": history_path.display().to_string(),
                }));
            } else if visits.is_empty() {
                println!("No browser history.");
            } else {
                for visit in visits {
                    let status = visit
                        .http_status
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{} [{}] {} status={} title={}",
                        visit.id,
                        if visit.ok { "ok" } else { "error" },
                        visit.url,
                        status,
                        visit.title.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        BrowserCommand::Show { visit_id } => {
            let visit = history
                .into_iter()
                .find(|item| item.id == visit_id)
                .ok_or_else(|| MosaicError::Validation(format!("visit '{}' not found", visit_id)))?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "visit": visit,
                }));
            } else {
                println!("visit: {}", visit.id);
                println!("ts: {}", visit.ts.to_rfc3339());
                println!("url: {}", visit.url);
                println!("ok: {}", visit.ok);
                if let Some(status) = visit.http_status {
                    println!("status: {status}");
                }
                if let Some(content_type) = visit.content_type {
                    println!("content_type: {content_type}");
                }
                if let Some(content_length) = visit.content_length {
                    println!("content_length: {content_length}");
                }
                if let Some(title) = visit.title {
                    println!("title: {title}");
                }
                if let Some(preview) = visit.preview {
                    println!("preview: {preview}");
                }
                if let Some(error) = visit.error {
                    println!("error: {error}");
                }
            }
        }
        BrowserCommand::Clear { visit_id, all } => {
            if all && visit_id.is_some() {
                return Err(MosaicError::Validation(
                    "cannot set visit_id and --all together".to_string(),
                ));
            }
            if !all && visit_id.is_none() {
                return Err(MosaicError::Validation(
                    "specify visit_id or use --all".to_string(),
                ));
            }

            let (removed, remaining) = if all {
                let removed = history.len();
                (removed, Vec::new())
            } else {
                let target = visit_id.expect("validated visit_id");
                let before = history.len();
                let remaining = history
                    .into_iter()
                    .filter(|item| item.id != target)
                    .collect::<Vec<_>>();
                (before.saturating_sub(remaining.len()), remaining)
            };
            if removed > 0 || all {
                save_browser_history(&history_path, &remaining)?;
            }
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "remaining": remaining.len(),
                    "path": history_path.display().to_string(),
                }));
            } else {
                println!("removed visits: {removed}");
                println!("remaining visits: {}", remaining.len());
            }
        }
    }
    Ok(())
}

async fn handle_channels(cli: &Cli, args: ChannelsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let channels_path = channels_file_path(&paths.data_dir);
    let channel_events_dir = channels_events_dir(&paths.data_dir);
    let repository = ChannelRepository::new(channels_path.clone(), channel_events_dir);
    match args.command {
        ChannelsCommand::List => {
            let channels = repository.list()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channels": channels,
                    "path": channels_path.display().to_string(),
                }));
            } else if channels.is_empty() {
                println!("No channels configured.");
            } else {
                for channel in channels {
                    println!(
                        "{} name={} kind={} endpoint={} target={} defaults={} last_login={} last_send={} last_error={}",
                        channel.id,
                        channel.name,
                        channel.kind,
                        channel.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        channel.target_masked.unwrap_or_else(|| "-".to_string()),
                        channel.has_template_defaults,
                        channel
                            .last_login_at
                            .map(|v| v.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        channel
                            .last_send_at
                            .map(|v| v.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        channel.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Status => {
            let status = repository.status()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "status": status,
                }));
            } else {
                println!("channels total: {}", status.total_channels);
                println!("channels healthy: {}", status.healthy_channels);
                println!("channels with errors: {}", status.channels_with_errors);
                if let Some(last_send_at) = status.last_send_at {
                    println!("last send at: {}", last_send_at.to_rfc3339());
                }
                if !status.kinds.is_empty() {
                    println!("kinds:");
                    for (kind, count) in status.kinds {
                        println!("- {kind}: {count}");
                    }
                }
            }
        }
        ChannelsCommand::Add {
            name,
            kind,
            endpoint,
            chat_id,
            token_env,
            default_parse_mode,
            default_title,
            default_block,
            default_metadata,
        } => {
            let default_metadata = default_metadata
                .map(|value| parse_json_input(&value, "channels add default metadata"))
                .transpose()?;
            let entry = repository.add(AddChannelInput {
                name,
                kind,
                endpoint,
                target: chat_id,
                token_env,
                template_defaults: ChannelTemplateDefaults {
                    parse_mode: default_parse_mode,
                    title: default_title,
                    blocks: default_block,
                    metadata: default_metadata,
                },
            })?;
            let rendered = format_channel_for_output(&entry);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": rendered,
                    "path": channels_path.display().to_string(),
                }));
            } else {
                println!("Channel added: {}", rendered.id);
            }
        }
        ChannelsCommand::Update {
            channel_id,
            name,
            endpoint,
            chat_id,
            token_env,
            clear_token_env,
            default_parse_mode,
            default_title,
            default_block,
            default_metadata,
            clear_defaults,
        } => {
            let default_metadata = default_metadata
                .map(|value| parse_json_input(&value, "channels update default metadata"))
                .transpose()?;
            let template_defaults = if default_parse_mode.is_some()
                || default_title.is_some()
                || !default_block.is_empty()
                || default_metadata.is_some()
            {
                Some(ChannelTemplateDefaults {
                    parse_mode: default_parse_mode,
                    title: default_title,
                    blocks: default_block,
                    metadata: default_metadata,
                })
            } else {
                None
            };
            let updated = repository.update(
                &channel_id,
                UpdateChannelInput {
                    name,
                    endpoint,
                    target: chat_id,
                    token_env,
                    clear_token_env,
                    template_defaults,
                    clear_template_defaults: clear_defaults,
                },
            )?;
            let rendered = format_channel_for_output(&updated);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": rendered,
                }));
            } else {
                println!("Channel updated: {}", rendered.id);
            }
        }
        ChannelsCommand::Login {
            channel_id,
            token_env,
        } => {
            let login = repository.login(&channel_id, token_env.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": channel_id,
                    "token_env": login.token_env,
                    "token_present": login.token_present,
                    "channel": format_channel_for_output(&login.channel),
                }));
            } else {
                println!("Channel login recorded for {channel_id}");
                println!(
                    "token env {} {}",
                    login.token_env,
                    if login.token_present {
                        "found"
                    } else {
                        "not found"
                    }
                );
            }
        }
        ChannelsCommand::Send {
            channel_id,
            text,
            parse_mode,
            title,
            block,
            metadata,
            idempotency_key,
            token_env,
        } => {
            let metadata = metadata
                .map(|value| parse_json_input(&value, "channels send metadata"))
                .transpose()?;
            let result = repository
                .send_with_options(
                    &channel_id,
                    &text,
                    token_env,
                    false,
                    ChannelSendOptions {
                        parse_mode,
                        title,
                        blocks: block,
                        idempotency_key,
                        metadata,
                    },
                )
                .await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": result.channel_id,
                    "kind": result.kind,
                    "delivered_via": result.delivered_via,
                    "attempts": result.attempts,
                    "http_status": result.http_status,
                    "endpoint_masked": result.endpoint_masked,
                    "target_masked": result.target_masked,
                    "parse_mode": result.parse_mode,
                    "idempotency_key": result.idempotency_key,
                    "deduplicated": result.deduplicated,
                    "rate_limited_ms": result.rate_limited_ms,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Message sent via {}", result.delivered_via);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
                if let Some(target) = result.target_masked {
                    println!("target: {target}");
                }
                if let Some(parse_mode) = result.parse_mode {
                    println!("parse_mode: {parse_mode}");
                }
                if let Some(key) = result.idempotency_key {
                    println!("idempotency_key: {key}");
                }
                if result.deduplicated {
                    println!("deduplicated: true");
                }
                if let Some(waited) = result.rate_limited_ms {
                    println!("rate_limited_ms: {waited}");
                }
            }
        }
        ChannelsCommand::Logs { channel, tail } => {
            let events = repository.logs(channel.as_deref(), tail)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "events": events,
                    "channel": channel,
                }));
            } else if events.is_empty() {
                println!("No channel events found.");
            } else {
                for event in events {
                    println!(
                        "{} channel={} kind={} status={} attempt={} http={} parse_mode={} idempotency_key={} deduplicated={} rate_limited_ms={} error={} preview={}",
                        event.ts.to_rfc3339(),
                        event.channel_id,
                        event.kind,
                        event.delivery_status,
                        event.attempt,
                        event
                            .http_status
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        event.parse_mode.unwrap_or_else(|| "-".to_string()),
                        event.idempotency_key.unwrap_or_else(|| "-".to_string()),
                        event.deduplicated,
                        event
                            .rate_limited_ms
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        event.error.unwrap_or_else(|| "-".to_string()),
                        event.text_preview
                    );
                }
            }
        }
        ChannelsCommand::Capabilities { channel, target } => {
            let capabilities = repository.capabilities(channel.as_deref(), target.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "capabilities": capabilities,
                }));
            } else if capabilities.is_empty() {
                println!("No channel capabilities resolved.");
            } else {
                for capability in capabilities {
                    println!(
                        "{} aliases={} endpoint={} token_env={} probe={} bearer_token={} parse_mode={} template={} idempotency={} rate_limit_report={}",
                        capability.kind,
                        if capability.aliases.is_empty() {
                            "-".to_string()
                        } else {
                            capability.aliases.join(",")
                        },
                        capability.supports_endpoint,
                        capability.supports_token_env,
                        capability.supports_test_probe,
                        capability.supports_bearer_token,
                        capability.supports_parse_mode,
                        capability.supports_message_template,
                        capability.supports_idempotency_key,
                        capability.supports_rate_limit_report
                    );
                }
            }
        }
        ChannelsCommand::Resolve { channel, query } => {
            let query = query.join(" ");
            let entries = repository.resolve(&channel, &query)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "entries": entries,
                    "channel": channel,
                    "query": query,
                }));
            } else if entries.is_empty() {
                println!("No channels resolved.");
            } else {
                for entry in entries {
                    println!(
                        "{} name={} kind={} endpoint={} target={} last_send={} last_error={}",
                        entry.id,
                        entry.name,
                        entry.kind,
                        entry.endpoint_masked.unwrap_or_else(|| "-".to_string()),
                        entry.target_masked.unwrap_or_else(|| "-".to_string()),
                        entry
                            .last_send_at
                            .map(|value| value.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string()),
                        entry.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        ChannelsCommand::Export { out } => {
            let file = repository.export_channels()?;
            let payload = json!({
                "schema": "mosaic.channels.export.v1",
                "exported_at": Utc::now(),
                "channels_file": file,
            });
            if let Some(path) = out {
                if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                let rendered = serde_json::to_string_pretty(&payload).map_err(|err| {
                    MosaicError::Validation(format!("failed to encode channels export JSON: {err}"))
                })?;
                std::fs::write(&path, rendered)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "path": path.display().to_string(),
                        "channels": payload["channels_file"]["channels"].as_array().map_or(0usize, |v| v.len()),
                    }));
                } else {
                    println!(
                        "Exported {} channels to {}",
                        payload["channels_file"]["channels"]
                            .as_array()
                            .map_or(0usize, |items| items.len()),
                        path.display()
                    );
                }
            } else if cli.json {
                print_json(&json!({
                    "ok": true,
                    "export": payload,
                }));
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(|err| {
                        MosaicError::Validation(format!(
                            "failed to render channels export JSON: {err}"
                        ))
                    })?
                );
            }
        }
        ChannelsCommand::Import {
            file,
            replace,
            strict,
            dry_run,
            report_out,
        } => {
            let raw = std::fs::read_to_string(&file).map_err(|err| {
                MosaicError::Config(format!(
                    "failed to read channels import file {}: {err}",
                    file.display()
                ))
            })?;
            let value = serde_json::from_str::<Value>(&raw).map_err(|err| {
                MosaicError::Validation(format!(
                    "invalid channels import JSON {}: {err}",
                    file.display()
                ))
            })?;
            let import_value = value
                .as_object()
                .and_then(|obj| obj.get("channels_file"))
                .cloned()
                .unwrap_or(value);
            let import_result =
                repository.import_channels_json(import_value, replace, strict, dry_run);
            let report_path = if let Some(path) = report_out.as_ref() {
                let report = match &import_result {
                    Ok(summary) => json!({
                        "schema": "mosaic.channels.import-report.v1",
                        "generated_at": Utc::now(),
                        "request": {
                            "file": file.display().to_string(),
                            "replace": replace,
                            "strict": strict,
                            "dry_run": dry_run,
                        },
                        "result": {
                            "ok": true,
                            "summary": summary,
                        }
                    }),
                    Err(err) => json!({
                        "schema": "mosaic.channels.import-report.v1",
                        "generated_at": Utc::now(),
                        "request": {
                            "file": file.display().to_string(),
                            "replace": replace,
                            "strict": strict,
                            "dry_run": dry_run,
                        },
                        "result": {
                            "ok": false,
                            "error": {
                                "code": err.code(),
                                "message": err.to_string(),
                                "exit_code": err.exit_code(),
                            },
                        }
                    }),
                };
                save_json_file(path, &report)?;
                Some(path.display().to_string())
            } else {
                None
            };
            let summary = import_result?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "file": file.display().to_string(),
                    "summary": summary,
                    "report_path": report_path,
                }));
            } else {
                println!(
                    "Import {}from {}: total={} imported={} updated={} skipped={} replace={} strict={}",
                    if summary.dry_run { "(dry-run) " } else { "" },
                    file.display(),
                    summary.total,
                    summary.imported,
                    summary.updated,
                    summary.skipped,
                    summary.replace,
                    summary.strict
                );
                if let Some(path) = report_path {
                    println!("report: {path}");
                }
            }
        }
        ChannelsCommand::RotateTokenEnv {
            channel,
            all,
            kind,
            from_token_env,
            to,
            dry_run,
            report_out,
        } => {
            let summary = repository.rotate_token_env(RotateTokenEnvInput {
                channel_id: channel,
                all,
                kind,
                from_token_env,
                to_token_env: to,
                dry_run,
            })?;
            let report_path = if let Some(path) = report_out {
                if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                let report = json!({
                    "schema": "mosaic.channels.token-rotation-report.v1",
                    "generated_at": Utc::now(),
                    "summary": summary.clone(),
                });
                let rendered = serde_json::to_string_pretty(&report).map_err(|err| {
                    MosaicError::Validation(format!(
                        "failed to encode token rotation report JSON: {err}"
                    ))
                })?;
                std::fs::write(&path, rendered)?;
                Some(path.display().to_string())
            } else {
                None
            };
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "summary": summary,
                    "report_path": report_path,
                }));
            } else {
                println!(
                    "Token env rotation {}complete: total={} updated={} skipped_already_set={} skipped_unsupported={} skipped_from_mismatch={} source={} target={}",
                    if summary.dry_run { "(dry-run) " } else { "" },
                    summary.total,
                    summary.updated,
                    summary.skipped_already_set,
                    summary.skipped_unsupported,
                    summary.skipped_from_mismatch,
                    summary.from_token_env.as_deref().unwrap_or("*"),
                    summary.to_token_env
                );
                if let Some(path) = report_path {
                    println!("report: {path}");
                }
            }
        }
        ChannelsCommand::Remove { channel_id } => {
            let removed = repository.remove(&channel_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": format_channel_for_output(&removed),
                }));
            } else {
                println!("Removed channel {}", removed.id);
            }
        }
        ChannelsCommand::Logout { channel_id } => {
            let logged_out = repository.logout(&channel_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel": format_channel_for_output(&logged_out),
                }));
            } else {
                println!("Cleared token env for channel {}", logged_out.id);
            }
        }
        ChannelsCommand::Test {
            channel_id,
            token_env,
        } => {
            let probe_text = "mosaic channel connectivity probe";
            let result = repository
                .send(&channel_id, probe_text, token_env, true)
                .await?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "channel_id": result.channel_id,
                    "kind": result.kind,
                    "probe": result.probe,
                    "attempts": result.attempts,
                    "http_status": result.http_status,
                    "endpoint_masked": result.endpoint_masked,
                    "target_masked": result.target_masked,
                    "parse_mode": result.parse_mode,
                    "idempotency_key": result.idempotency_key,
                    "deduplicated": result.deduplicated,
                    "rate_limited_ms": result.rate_limited_ms,
                    "event_path": result.event_path,
                }));
            } else {
                println!("Channel test passed for {}", result.channel_id);
                println!("attempts: {}", result.attempts);
                if let Some(endpoint) = result.endpoint_masked {
                    println!("endpoint: {endpoint}");
                }
                if let Some(target) = result.target_masked {
                    println!("target: {target}");
                }
                if let Some(waited) = result.rate_limited_ms {
                    println!("rate_limited_ms: {waited}");
                }
            }
        }
    }
    Ok(())
}

async fn handle_logs(cli: &Cli, args: LogsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;

    if !args.follow {
        let entries = collect_logs(&paths.data_dir, args.tail)?;
        if cli.json {
            print_json(&json!({
                "ok": true,
                "logs": entries,
            }));
        } else if entries.is_empty() {
            println!("No logs found.");
        } else {
            for entry in entries {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
        }
        return Ok(());
    }

    let mut printed = 0usize;
    loop {
        let entries = collect_logs(&paths.data_dir, args.tail.max(200))?;
        if entries.len() > printed {
            for entry in entries.iter().skip(printed) {
                println!(
                    "{} [{}] {}",
                    entry
                        .ts
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string()),
                    entry.source,
                    entry.payload
                );
            }
            printed = entries.len();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn handle_system(cli: &Cli, args: SystemArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SystemEventStore::new(system_events_path(&paths.data_dir));
    match args.command {
        SystemCommand::Event { name, data } => {
            let data = data
                .as_deref()
                .map(|value| parse_json_input(value, "system event data"))
                .transpose()?
                .unwrap_or(Value::Null);
            let dispatch = dispatch_system_event(cli, &paths, &name, data)?;
            let event = dispatch.event;
            let hook_reports = dispatch.hook_reports;
            let hooks_ok = hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hook_reports.len().saturating_sub(hooks_ok);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "event": event,
                    "path": store.path().display().to_string(),
                    "hooks": {
                        "triggered": hook_reports.len(),
                        "ok": hooks_ok,
                        "failed": hooks_failed,
                        "results": hook_reports,
                    }
                }));
            } else {
                println!("event recorded: {}", event.name);
                println!("path: {}", store.path().display());
                if hook_reports.is_empty() {
                    println!("hooks triggered: 0");
                } else {
                    println!("hooks triggered: {}", hook_reports.len());
                    println!("hooks ok: {hooks_ok}");
                    println!("hooks failed: {hooks_failed}");
                }
            }
        }
        SystemCommand::Presence => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let presence = snapshot_presence(&cwd);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "presence": presence,
                }));
            } else {
                println!("presence:");
                println!("hostname: {}", presence.hostname);
                println!("pid: {}", presence.pid);
                println!("cwd: {}", presence.cwd);
                println!("ts: {}", presence.ts.to_rfc3339());
            }
        }
    }
    Ok(())
}

fn handle_approvals(cli: &Cli, args: ApprovalsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let policy = match args.command {
        ApprovalsCommand::Get => store.load_or_default()?,
        ApprovalsCommand::Set { mode } => store.set_mode(mode.into())?,
        ApprovalsCommand::Allowlist { command } => match command {
            AllowlistCommand::Add { prefix } => store.add_allowlist(&prefix)?,
            AllowlistCommand::Remove { prefix } => store.remove_allowlist(&prefix)?,
        },
    };

    if cli.json {
        print_json(&json!({
            "ok": true,
            "policy": policy,
            "path": store.path().display().to_string(),
        }));
    } else {
        println!("approvals mode: {:?}", policy.mode);
        if policy.allowlist.is_empty() {
            println!("allowlist: <empty>");
        } else {
            println!("allowlist:");
            for item in policy.allowlist {
                println!("- {item}");
            }
        }
        println!("path: {}", store.path().display());
    }
    Ok(())
}

fn handle_sandbox(cli: &Cli, args: SandboxArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let policy = store.load_or_default()?;
    match args.command {
        SandboxCommand::List => {
            let profiles = list_profiles();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "current": policy.profile,
                    "profiles": profiles,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("current sandbox profile: {:?}", policy.profile);
                for profile in profiles {
                    println!("- {:?}: {}", profile.profile, profile.description);
                }
            }
        }
        SandboxCommand::Explain { profile } => {
            let profile = profile.map(Into::into).unwrap_or(policy.profile);
            let info = mosaic_ops::profile_info(profile);
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "profile": info,
                    "path": store.path().display().to_string(),
                }));
            } else {
                println!("sandbox profile: {:?}", info.profile);
                println!("{}", info.description);
                if info.blocked_examples.is_empty() {
                    println!("blocked examples: <none>");
                } else {
                    println!("blocked examples:");
                    for example in info.blocked_examples {
                        println!("- {example}");
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_memory(cli: &Cli, args: MemoryArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let store = MemoryStore::new(
        memory_index_path(&paths.data_dir),
        memory_status_path(&paths.data_dir),
    );

    match args.command {
        MemoryCommand::Index {
            path,
            max_files,
            max_file_size,
            max_content_bytes,
        } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let root = {
                let raw = PathBuf::from(path);
                if raw.is_absolute() {
                    raw
                } else {
                    cwd.join(raw)
                }
            };
            let result = store.index(MemoryIndexOptions {
                root,
                max_files,
                max_file_size,
                max_content_bytes,
            })?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "index": result,
                }));
            } else {
                println!("memory indexed documents: {}", result.indexed_documents);
                println!("memory skipped files: {}", result.skipped_files);
                println!("index path: {}", result.index_path);
            }
        }
        MemoryCommand::Search { query, limit } => {
            let result = store.search(&query, Some(limit))?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "result": result,
                }));
            } else if result.hits.is_empty() {
                println!("No memory hits.");
            } else {
                println!(
                    "memory search hits: {} (showing {})",
                    result.total_hits,
                    result.hits.len()
                );
                for hit in result.hits {
                    println!("{} score={} {}", hit.path, hit.score, hit.snippet);
                }
            }
        }
        MemoryCommand::Status => {
            let status = store.status()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "status": status,
                }));
            } else {
                println!("indexed documents: {}", status.indexed_documents);
                println!(
                    "last indexed at: {}",
                    status
                        .last_indexed_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("index path: {}", status.index_path);
            }
        }
    }
    Ok(())
}

fn handle_security(cli: &Cli, args: SecurityArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let auditor = SecurityAuditor::new();
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;

    match args.command {
        SecurityCommand::Audit {
            path,
            deep,
            max_files,
            max_file_size,
            baseline,
            no_baseline,
            update_baseline,
            sarif,
            sarif_output,
        } => {
            if no_baseline && update_baseline {
                return Err(MosaicError::Validation(
                    "--no-baseline and --update-baseline cannot be used together".to_string(),
                ));
            }
            let root = {
                let raw = PathBuf::from(path);
                if raw.is_absolute() {
                    raw
                } else {
                    cwd.join(raw)
                }
            };
            let mut report = auditor.audit(SecurityAuditOptions {
                root,
                deep,
                max_files,
                max_file_size,
            })?;
            let baseline_path = resolve_baseline_path(&paths, &cwd, baseline);
            let baseline_path_display = baseline_path.display().to_string();
            let mut baseline_added = 0usize;
            let mut baseline_enabled = false;
            let mut sarif_output_path = None;

            if !no_baseline || update_baseline {
                let mut baseline_config =
                    SecurityBaselineConfig::load_optional(&baseline_path)?.unwrap_or_default();
                if !no_baseline {
                    baseline_enabled = true;
                    let applied = apply_baseline(report, &baseline_config);
                    report = applied.report;
                    report.summary.baseline_path = Some(baseline_path_display.clone());
                }
                if update_baseline {
                    baseline_enabled = true;
                    baseline_added = baseline_config.add_findings(&report.findings);
                    baseline_config.save_to_path(&baseline_path)?;
                    if !report.findings.is_empty() {
                        report.summary.ignored += report.findings.len();
                        report.findings.clear();
                        report.summary.findings = 0;
                        report.summary.high = 0;
                        report.summary.medium = 0;
                        report.summary.low = 0;
                        report.summary.ok = true;
                    }
                    report.summary.baseline_path = Some(baseline_path_display.clone());
                }
            }

            let sarif_value = if sarif || sarif_output.is_some() {
                Some(report_to_sarif(&report))
            } else {
                None
            };
            if let Some(raw_path) = sarif_output {
                let output_path = resolve_output_path(&cwd, &raw_path);
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let encoded = serde_json::to_string_pretty(
                    sarif_value
                        .as_ref()
                        .ok_or_else(|| MosaicError::Unknown("sarif value missing".to_string()))?,
                )
                .map_err(|err| {
                    MosaicError::Validation(format!("failed to encode sarif JSON: {err}"))
                })?;
                std::fs::write(&output_path, encoded)?;
                sarif_output_path = Some(output_path.display().to_string());
            }

            if sarif {
                print_json(
                    sarif_value
                        .as_ref()
                        .ok_or_else(|| MosaicError::Unknown("sarif value missing".to_string()))?,
                );
                return Ok(());
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                    "baseline": {
                        "enabled": baseline_enabled,
                        "updated": update_baseline,
                        "added": baseline_added,
                        "path": if baseline_enabled {
                            Some(baseline_path_display.clone())
                        } else {
                            None
                        },
                    },
                    "sarif_output": sarif_output_path,
                }));
            } else {
                println!(
                    "security audit summary: findings={} high={} medium={} low={} ignored={} scanned={} skipped={}",
                    report.summary.findings,
                    report.summary.high,
                    report.summary.medium,
                    report.summary.low,
                    report.summary.ignored,
                    report.summary.scanned_files,
                    report.summary.skipped_files
                );
                if baseline_enabled {
                    println!("baseline: {baseline_path_display}");
                }
                if update_baseline {
                    println!("baseline updated: added {baseline_added} fingerprints");
                }
                if let Some(sarif_output_path) = sarif_output_path {
                    println!("sarif: {sarif_output_path}");
                }
                if report.findings.is_empty() {
                    println!("No security findings.");
                } else {
                    for finding in report.findings {
                        println!(
                            "[{:?}] {}:{} {} ({})",
                            finding.severity,
                            finding.path,
                            finding
                                .line
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                            finding.title,
                            finding.category
                        );
                        if let Some(suggestion) = finding.suggestion {
                            println!("  suggestion: {suggestion}");
                        }
                    }
                }
            }
        }
        SecurityCommand::Baseline { command } => {
            handle_security_baseline(cli, &paths, &cwd, command)?;
        }
    }
    Ok(())
}

fn handle_security_baseline(
    cli: &Cli,
    paths: &StatePaths,
    cwd: &std::path::Path,
    command: SecurityBaselineCommand,
) -> Result<()> {
    match command {
        SecurityBaselineCommand::Show { path } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let exists = baseline_path.exists();
            let baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let stats = json!({
                "fingerprints": baseline.ignored_fingerprints.len(),
                "categories": baseline.ignored_categories.len(),
                "paths": baseline.ignored_paths.len(),
            });
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "baseline": baseline,
                    "path": baseline_path.display().to_string(),
                    "exists": exists,
                    "stats": stats,
                }));
            } else {
                println!("baseline path: {}", baseline_path.display());
                println!("exists: {}", exists);
                println!(
                    "entries: fingerprints={} categories={} paths={}",
                    baseline.ignored_fingerprints.len(),
                    baseline.ignored_categories.len(),
                    baseline.ignored_paths.len()
                );
            }
        }
        SecurityBaselineCommand::Add {
            path,
            fingerprints,
            categories,
            match_paths,
        } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let mut baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let fingerprints = normalize_non_empty_list(fingerprints, "fingerprint")?;
            let categories = normalize_non_empty_list(categories, "category")?;
            let match_paths = normalize_non_empty_list(match_paths, "match-path")?;
            if fingerprints.is_empty() && categories.is_empty() && match_paths.is_empty() {
                return Err(MosaicError::Validation(
                    "baseline add requires at least one of --fingerprint/--category/--match-path"
                        .to_string(),
                ));
            }

            let mut added = 0usize;
            for value in fingerprints {
                if !baseline.ignored_fingerprints.contains(&value) {
                    baseline.ignored_fingerprints.push(value);
                    added += 1;
                }
            }
            for value in categories {
                if !baseline.ignored_categories.contains(&value) {
                    baseline.ignored_categories.push(value);
                    added += 1;
                }
            }
            for value in match_paths {
                if !baseline.ignored_paths.contains(&value) {
                    baseline.ignored_paths.push(value);
                    added += 1;
                }
            }
            baseline.save_to_path(&baseline_path)?;

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "added": added,
                    "path": baseline_path.display().to_string(),
                    "baseline": baseline,
                }));
            } else {
                println!("baseline updated: added {added} entries");
                println!("path: {}", baseline_path.display());
            }
        }
        SecurityBaselineCommand::Remove {
            path,
            fingerprints,
            categories,
            match_paths,
        } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let exists = baseline_path.exists();
            let mut baseline = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let fingerprints = normalize_non_empty_list(fingerprints, "fingerprint")?;
            let categories = normalize_non_empty_list(categories, "category")?;
            let match_paths = normalize_non_empty_list(match_paths, "match-path")?;
            if fingerprints.is_empty() && categories.is_empty() && match_paths.is_empty() {
                return Err(MosaicError::Validation(
                    "baseline remove requires at least one of --fingerprint/--category/--match-path"
                        .to_string(),
                ));
            }

            let mut removed = 0usize;
            removed += remove_matching(&mut baseline.ignored_fingerprints, &fingerprints);
            removed += remove_matching(&mut baseline.ignored_categories, &categories);
            removed += remove_matching(&mut baseline.ignored_paths, &match_paths);
            if exists {
                baseline.save_to_path(&baseline_path)?;
            }

            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "path": baseline_path.display().to_string(),
                    "exists": exists,
                    "baseline": baseline,
                }));
            } else {
                println!("baseline updated: removed {removed} entries");
                println!("path: {}", baseline_path.display());
            }
        }
        SecurityBaselineCommand::Clear { path } => {
            let baseline_path = resolve_baseline_path(paths, cwd, path);
            let old = SecurityBaselineConfig::load_optional(&baseline_path)?
                .unwrap_or_else(SecurityBaselineConfig::default);
            let removed = old.ignored_fingerprints.len()
                + old.ignored_categories.len()
                + old.ignored_paths.len();
            let baseline = SecurityBaselineConfig::default();
            baseline.save_to_path(&baseline_path)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "cleared": removed,
                    "path": baseline_path.display().to_string(),
                    "baseline": baseline,
                }));
            } else {
                println!("baseline cleared: removed {removed} entries");
                println!("path: {}", baseline_path.display());
            }
        }
    }
    Ok(())
}

fn handle_agents(cli: &Cli, args: AgentsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let manager = ConfigManager::new(paths.config_path.clone());
    let store = AgentStore::new(
        agents_file_path(&paths.data_dir),
        agent_routes_path(&paths.data_dir),
    );
    store.ensure_dirs()?;

    match args.command {
        AgentsCommand::List => {
            let agents = store.list()?;
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agents": agents,
                    "routes": routes,
                }));
            } else if agents.is_empty() {
                println!("No agents found.");
            } else {
                println!("agents: {}", agents.len());
                if let Some(default_agent_id) = &routes.default_agent_id {
                    println!("default agent: {default_agent_id}");
                }
                for agent in agents {
                    println!(
                        "- {} ({}) profile={} model={} temperature={} max_turns={}",
                        agent.id,
                        agent.name,
                        agent.profile,
                        agent.model.unwrap_or_else(|| "-".to_string()),
                        agent
                            .temperature
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        agent
                            .max_turns
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        AgentsCommand::Add {
            name,
            id,
            profile,
            model,
            temperature,
            max_turns,
            tools_enabled,
            guard_mode,
            set_default,
            route_keys,
        } => {
            if !manager.exists() {
                return Err(MosaicError::Config(
                    "config file not found. run `mosaic setup` first".to_string(),
                ));
            }
            let config = manager.load()?;
            let profile = profile.unwrap_or_else(|| cli.profile.clone());
            let _ = config.resolve_profile(Some(&profile))?;

            let created = store.add(AddAgentInput {
                id,
                name,
                profile,
                model,
                temperature,
                max_turns,
                tools_enabled,
                guard_mode: guard_mode.map(Into::into),
            })?;
            if set_default {
                store.set_default(&created.id)?;
            }
            for route_key in route_keys {
                store.set_route(&route_key, &created.id)?;
            }
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agent": created,
                    "routes": routes,
                }));
            } else {
                println!("Created agent {} ({})", created.id, created.name);
                println!("profile: {}", created.profile);
            }
        }
        AgentsCommand::Update {
            agent_id,
            name,
            profile,
            model,
            clear_model,
            temperature,
            clear_temperature,
            max_turns,
            clear_max_turns,
            tools_enabled,
            clear_tools_enabled,
            guard_mode,
            clear_guard_mode,
            set_default,
            route_keys,
        } => {
            if !manager.exists() {
                return Err(MosaicError::Config(
                    "config file not found. run `mosaic setup` first".to_string(),
                ));
            }
            let config = manager.load()?;
            if let Some(profile_name) = profile.as_deref() {
                let _ = config.resolve_profile(Some(profile_name))?;
            }

            let updated = store.update(
                &agent_id,
                UpdateAgentInput {
                    name,
                    profile,
                    model,
                    clear_model,
                    temperature,
                    clear_temperature,
                    max_turns,
                    clear_max_turns,
                    tools_enabled,
                    clear_tools_enabled,
                    guard_mode: guard_mode.map(Into::into),
                    clear_guard_mode,
                },
            )?;

            if set_default {
                store.set_default(&updated.id)?;
            }
            for route_key in route_keys {
                store.set_route(&route_key, &updated.id)?;
            }
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agent": updated,
                    "routes": routes,
                }));
            } else {
                println!("Updated agent {} ({})", updated.id, updated.name);
                println!("profile: {}", updated.profile);
            }
        }
        AgentsCommand::Show { agent_id } => {
            let agent = store
                .get(&agent_id)?
                .ok_or_else(|| MosaicError::Validation(format!("agent '{agent_id}' not found")))?;
            let routes = store.load_routes()?;
            let route_keys = routes
                .routes
                .iter()
                .filter_map(|(route, id)| {
                    if id == &agent.id {
                        Some(route.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "agent": agent,
                    "is_default": routes.default_agent_id.as_deref() == Some(agent_id.as_str()),
                    "route_keys": route_keys,
                }));
            } else {
                println!("id: {}", agent.id);
                println!("name: {}", agent.name);
                println!("profile: {}", agent.profile);
                println!("model: {}", agent.model.unwrap_or_else(|| "-".to_string()));
                println!(
                    "temperature: {}",
                    agent
                        .temperature
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "max_turns: {}",
                    agent
                        .max_turns
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "tools_enabled: {}",
                    agent
                        .tools_enabled
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "guard_mode: {}",
                    agent
                        .guard_mode
                        .map(|value| format!("{value:?}"))
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "default: {}",
                    routes.default_agent_id.as_deref() == Some(agent_id.as_str())
                );
                if route_keys.is_empty() {
                    println!("routes: <none>");
                } else {
                    println!("routes: {}", route_keys.join(", "));
                }
            }
        }
        AgentsCommand::Remove { agent_id } => {
            let removed = store.remove(&agent_id)?;
            if !removed {
                return Err(MosaicError::Validation(format!(
                    "agent '{agent_id}' not found"
                )));
            }
            let routes = store.load_routes()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": true,
                    "agent_id": agent_id,
                    "routes": routes,
                }));
            } else {
                println!("Removed agent {agent_id}");
            }
        }
        AgentsCommand::Default { agent_id } => match agent_id {
            Some(agent_id) => {
                let routes = store.set_default(&agent_id)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "default_agent_id": routes.default_agent_id,
                    }));
                } else {
                    println!(
                        "Default agent: {}",
                        routes.default_agent_id.unwrap_or_default()
                    );
                }
            }
            None => {
                let routes = store.load_routes()?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "default_agent_id": routes.default_agent_id,
                    }));
                } else {
                    println!(
                        "default agent: {}",
                        routes
                            .default_agent_id
                            .unwrap_or_else(|| "<none>".to_string())
                    );
                }
            }
        },
        AgentsCommand::Route { command } => match command {
            AgentsRouteCommand::List => {
                let routes = store.load_routes()?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "routes": routes.routes,
                        "default_agent_id": routes.default_agent_id,
                    }));
                } else if routes.routes.is_empty() {
                    println!("No route bindings.");
                } else {
                    if let Some(default_agent_id) = routes.default_agent_id {
                        println!("default: {default_agent_id}");
                    }
                    for (route, agent_id) in routes.routes {
                        println!("{route} -> {agent_id}");
                    }
                }
            }
            AgentsRouteCommand::Set {
                route_key,
                agent_id,
            } => {
                let routes = store.set_route(&route_key, &agent_id)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "route_key": route_key,
                        "agent_id": agent_id,
                        "routes": routes,
                    }));
                } else {
                    println!("Bound route {route_key} -> {agent_id}");
                }
            }
            AgentsRouteCommand::Remove { route_key } => {
                let (routes, removed) = store.remove_route(&route_key)?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "removed": removed,
                        "route_key": route_key,
                        "routes": routes,
                    }));
                } else if removed {
                    println!("Removed route {route_key}");
                } else {
                    println!("Route {route_key} not found.");
                }
            }
            AgentsRouteCommand::Resolve { route } => {
                let resolved = store.resolve_for_runtime(None, route.as_deref())?;
                if cli.json {
                    print_json(&json!({
                        "ok": true,
                        "route": route,
                        "agent_id": resolved,
                    }));
                } else {
                    println!(
                        "resolved agent: {}",
                        resolved.unwrap_or_else(|| "<none>".to_string())
                    );
                }
            }
        },
    }
    Ok(())
}

fn handle_plugins(cli: &Cli, args: PluginsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));

    match args.command {
        PluginsCommand::List => {
            let plugins = registry.list_plugins()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "count": plugins.len(),
                    "plugins": plugins,
                }));
            } else if plugins.is_empty() {
                println!("No plugins found.");
            } else {
                println!("plugins: {}", plugins.len());
                for plugin in plugins {
                    println!(
                        "- {} ({}) source={:?} version={} manifest_valid={}",
                        plugin.id,
                        plugin.name,
                        plugin.source,
                        plugin.version.unwrap_or_else(|| "-".to_string()),
                        plugin.manifest_valid
                    );
                }
            }
        }
        PluginsCommand::Info { plugin_id } => {
            let plugin = registry.plugin_info(&plugin_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "plugin": plugin,
                }));
            } else {
                println!("id: {}", plugin.id);
                println!("name: {}", plugin.name);
                println!("source: {:?}", plugin.source);
                println!(
                    "version: {}",
                    plugin.version.unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "description: {}",
                    plugin.description.unwrap_or_else(|| "-".to_string())
                );
                println!("path: {}", plugin.path);
                println!("manifest path: {}", plugin.manifest_path);
                println!("manifest valid: {}", plugin.manifest_valid);
                if let Some(error) = plugin.manifest_error {
                    println!("manifest error: {error}");
                }
            }
        }
        PluginsCommand::Check { plugin_id } => {
            let report = registry.check_plugins(plugin_id.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                }));
            } else {
                println!(
                    "plugin checks: checked={} failed={} ok={}",
                    report.checked, report.failed, report.ok
                );
                for result in report.results {
                    println!(
                        "- {} source={:?} ok={}",
                        result.id, result.source, result.ok
                    );
                    for check in result.checks {
                        let status = if check.ok { "OK" } else { "WARN" };
                        println!("  [{status}] {}: {}", check.name, check.detail);
                    }
                }
            }
        }
        PluginsCommand::Install { path, force } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let source = {
                let value = PathBuf::from(path);
                if value.is_absolute() {
                    value
                } else {
                    cwd.join(value)
                }
            };
            let outcome = registry.install_plugin_from_path(&source, force)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                }));
            } else {
                println!(
                    "Installed plugin {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing plugin package");
                }
            }
        }
        PluginsCommand::Remove { plugin_id } => {
            let removed = registry.remove_project_plugin(&plugin_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "plugin_id": plugin_id,
                }));
            } else if removed {
                println!("Removed plugin {plugin_id}");
            } else {
                println!("Plugin {plugin_id} not found in project scope.");
            }
        }
    }
    Ok(())
}

fn handle_skills(cli: &Cli, args: SkillsArgs) -> Result<()> {
    let paths = resolve_state_paths(cli.project_state)?;
    paths.ensure_dirs()?;
    let registry = ExtensionRegistry::new(RegistryRoots::from_state_root(paths.root_dir.clone()));

    match args.command {
        SkillsCommand::List => {
            let skills = registry.list_skills()?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "count": skills.len(),
                    "skills": skills,
                }));
            } else if skills.is_empty() {
                println!("No skills found.");
            } else {
                println!("skills: {}", skills.len());
                for skill in skills {
                    println!("- {} ({}) source={:?}", skill.id, skill.title, skill.source);
                }
            }
        }
        SkillsCommand::Info { skill_id } => {
            let skill = registry.skill_info(&skill_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "skill": skill,
                }));
            } else {
                println!("id: {}", skill.id);
                println!("title: {}", skill.title);
                println!(
                    "description: {}",
                    skill.description.unwrap_or_else(|| "-".to_string())
                );
                println!("source: {:?}", skill.source);
                println!("path: {}", skill.path);
                println!("skill file: {}", skill.skill_file);
            }
        }
        SkillsCommand::Check { skill_id } => {
            let report = registry.check_skills(skill_id.as_deref())?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "report": report,
                }));
            } else {
                println!(
                    "skill checks: checked={} failed={} ok={}",
                    report.checked, report.failed, report.ok
                );
                for result in report.results {
                    println!(
                        "- {} source={:?} ok={}",
                        result.id, result.source, result.ok
                    );
                    for check in result.checks {
                        let status = if check.ok { "OK" } else { "WARN" };
                        println!("  [{status}] {}: {}", check.name, check.detail);
                    }
                }
            }
        }
        SkillsCommand::Install { path, force } => {
            let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
            let source = {
                let value = PathBuf::from(path);
                if value.is_absolute() {
                    value
                } else {
                    cwd.join(value)
                }
            };
            let outcome = registry.install_skill_from_path(&source, force)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "installed": outcome,
                }));
            } else {
                println!(
                    "Installed skill {} -> {}",
                    outcome.id, outcome.installed_path
                );
                if outcome.replaced {
                    println!("replaced existing skill package");
                }
            }
        }
        SkillsCommand::Remove { skill_id } => {
            let removed = registry.remove_project_skill(&skill_id)?;
            if cli.json {
                print_json(&json!({
                    "ok": true,
                    "removed": removed,
                    "skill_id": skill_id,
                }));
            } else if removed {
                println!("Removed skill {skill_id}");
            } else {
                println!("Skill {skill_id} not found in project scope.");
            }
        }
    }
    Ok(())
}

fn upsert_gateway_service(
    service_path: &std::path::Path,
    host: Option<String>,
    port: Option<u16>,
    installed: bool,
) -> Result<GatewayServiceState> {
    let existing: Option<GatewayServiceState> = load_json_file_opt(service_path)?;
    let now = Utc::now();
    let service = GatewayServiceState {
        installed,
        host: host
            .or_else(|| existing.as_ref().map(|item| item.host.clone()))
            .unwrap_or_else(|| "127.0.0.1".to_string()),
        port: port
            .or_else(|| existing.as_ref().map(|item| item.port))
            .unwrap_or(8787),
        installed_at: existing
            .as_ref()
            .map(|item| item.installed_at)
            .unwrap_or(now),
        updated_at: now,
    };
    save_json_file(service_path, &service)?;
    Ok(service)
}

fn resolve_gateway_start_target(
    service_path: &std::path::Path,
    host: Option<String>,
    port: Option<u16>,
    default_host: &str,
    default_port: u16,
) -> Result<(String, u16)> {
    let service: Option<GatewayServiceState> = load_json_file_opt(service_path)?;
    let resolved_host = host
        .or_else(|| service.as_ref().map(|item| item.host.clone()))
        .unwrap_or_else(|| default_host.to_string());
    let resolved_port = port
        .or_else(|| service.as_ref().map(|item| item.port))
        .unwrap_or(default_port);
    Ok((resolved_host, resolved_port))
}

async fn start_gateway_runtime(
    cli: &Cli,
    gateway_path: &std::path::Path,
    host: String,
    port: u16,
) -> Result<GatewayStartResult> {
    if let Some(existing) = load_json_file_opt::<GatewayState>(gateway_path)? {
        let alive = if gateway_test_mode() {
            existing.running
        } else {
            is_process_alive(existing.pid)
                && probe_gateway_health(&existing.host, existing.port).await
        };
        if alive {
            return Ok(GatewayStartResult {
                state: existing,
                already_running: true,
            });
        }
    }

    let pid = if gateway_test_mode() {
        0
    } else {
        spawn_gateway_process(cli, &host, port)?
    };
    if !gateway_test_mode() {
        let mut ready = false;
        for _ in 0..40 {
            if probe_gateway_health(&host, port).await {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !ready {
            return Err(MosaicError::Network(format!(
                "gateway did not become healthy at http://{host}:{port}/health"
            )));
        }
    }
    let now = Utc::now();
    let state = GatewayState {
        running: true,
        host,
        port,
        pid,
        started_at: now,
        updated_at: now,
    };
    save_json_file(gateway_path, &state)?;
    Ok(GatewayStartResult {
        state,
        already_running: false,
    })
}

fn stop_gateway_runtime(
    gateway_path: &std::path::Path,
    required: bool,
) -> Result<GatewayStopResult> {
    let Some(state) = load_json_file_opt::<GatewayState>(gateway_path)? else {
        if required {
            return Err(MosaicError::Config(
                "gateway state file not found; not running".to_string(),
            ));
        }
        return Ok(GatewayStopResult {
            was_running: false,
            stopped: false,
            state: None,
        });
    };

    let was_alive = if gateway_test_mode() {
        state.running
    } else {
        is_process_alive(state.pid)
    };
    let stopped = if was_alive {
        if gateway_test_mode() {
            true
        } else {
            stop_process(state.pid)?
        }
    } else {
        false
    };

    let next = GatewayState {
        running: false,
        host: state.host,
        port: state.port,
        pid: state.pid,
        started_at: state.started_at,
        updated_at: Utc::now(),
    };
    save_json_file(gateway_path, &next)?;
    Ok(GatewayStopResult {
        was_running: was_alive,
        stopped: stopped || !was_alive,
        state: Some(next),
    })
}

async fn collect_gateway_runtime_status(
    gateway_path: &std::path::Path,
    gateway_service_path: &std::path::Path,
) -> Result<GatewayRuntimeStatus> {
    let state: Option<GatewayState> = load_json_file_opt(gateway_path)?;
    let service: Option<GatewayServiceState> = load_json_file_opt(gateway_service_path)?;
    let process_alive = state.as_ref().is_some_and(|value| {
        if gateway_test_mode() {
            value.running
        } else {
            is_process_alive(value.pid)
        }
    });
    let endpoint_healthy = if let Some(value) = &state {
        if gateway_test_mode() {
            value.running
        } else {
            probe_gateway_health(&value.host, value.port).await
        }
    } else {
        false
    };
    let running = match &state {
        Some(value) => {
            if gateway_test_mode() {
                value.running
            } else {
                process_alive && endpoint_healthy
            }
        }
        None => false,
    };
    let (target_host, target_port) = resolve_gateway_target(gateway_path, gateway_service_path)?;
    Ok(GatewayRuntimeStatus {
        running,
        process_alive,
        endpoint_healthy,
        state,
        service,
        target_host,
        target_port,
    })
}

#[derive(Debug, Clone)]
struct GatewayCallDispatch {
    request_id: String,
    host: String,
    port: u16,
    result: Value,
}

async fn dispatch_gateway_call(
    gateway_path: &std::path::Path,
    gateway_service_path: &std::path::Path,
    method: &str,
    params: Value,
) -> Result<GatewayCallDispatch> {
    if gateway_test_mode() {
        let state: GatewayState = load_json_file_opt(gateway_path)?.ok_or_else(|| {
            MosaicError::GatewayUnavailable("gateway is not running in test mode".to_string())
        })?;
        if !state.running {
            return Err(MosaicError::GatewayUnavailable(
                "gateway is not running in test mode".to_string(),
            ));
        }
        let result = match method {
            "nodes.run" => json!({
                "ok": true,
                "status": "accepted",
                "mode": "test_mode",
                "node_id": params.get("node_id").cloned().unwrap_or(Value::Null),
                "command": params.get("command").cloned().unwrap_or(Value::Null),
            }),
            "nodes.invoke" => json!({
                "ok": true,
                "status": "accepted",
                "mode": "test_mode",
                "node_id": params.get("node_id").cloned().unwrap_or(Value::Null),
                "method": params.get("method").cloned().unwrap_or(Value::Null),
                "params": params.get("params").cloned().unwrap_or(Value::Null),
            }),
            _ => {
                return Err(MosaicError::GatewayProtocol(format!(
                    "gateway test mode does not support method '{}'",
                    method
                )));
            }
        };
        return Ok(GatewayCallDispatch {
            request_id: "gateway-test-mode".to_string(),
            host: state.host,
            port: state.port,
            result,
        });
    }

    let (host, port) = resolve_gateway_target(gateway_path, gateway_service_path)?;
    let client = HttpGatewayClient::new(&host, port)?;
    let request = GatewayRequest::new(method.to_string(), Some(params));
    let request_id = request.id.clone();
    let response = client.call(request).await?;
    Ok(GatewayCallDispatch {
        request_id,
        host,
        port,
        result: response.result.unwrap_or(Value::Null),
    })
}

fn spawn_gateway_process(cli: &Cli, host: &str, port: u16) -> Result<u32> {
    let exe = std::env::current_exe().map_err(|err| {
        MosaicError::Io(format!("failed to resolve current executable path: {err}"))
    })?;
    let mut cmd = Command::new(exe);
    if !cli.debug {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    if cli.project_state {
        cmd.arg("--project-state");
    }
    if cli.debug {
        cmd.arg("--debug");
    }
    if cli.profile != DEFAULT_PROFILE {
        cmd.arg("--profile").arg(&cli.profile);
    }
    cmd.arg("gateway")
        .arg("serve")
        .arg("--host")
        .arg(host)
        .arg("--port")
        .arg(port.to_string());
    let child = cmd
        .spawn()
        .map_err(|err| MosaicError::Io(format!("failed to spawn gateway process: {err}")))?;
    Ok(child.id())
}

fn gateway_test_mode() -> bool {
    std::env::var("MOSAIC_GATEWAY_TEST_MODE").ok().as_deref() == Some("1")
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        match Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }
    #[cfg(windows)]
    {
        match Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .output()
        {
            Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
}

fn stop_process(pid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .arg(pid.to_string())
            .status()
            .map_err(|err| MosaicError::Io(format!("failed to issue kill for pid {pid}: {err}")))?;
        if !status.success() {
            return Ok(false);
        }
        for _ in 0..30 {
            if !is_process_alive(pid) {
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(false)
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/F")
            .status()
            .map_err(|err| {
                MosaicError::Io(format!("failed to issue taskkill for pid {pid}: {err}"))
            })?;
        Ok(status.success())
    }
}

async fn probe_gateway_health(host: &str, port: u16) -> bool {
    let address = format!("{host}:{port}");
    let mut addrs = match address.to_socket_addrs() {
        Ok(values) => values,
        Err(_) => return false,
    };
    let Some(first_addr) = addrs.next() else {
        return false;
    };
    if TcpStream::connect_timeout(&first_addr, Duration::from_millis(250)).is_err() {
        return false;
    }
    let url = format!("http://{host}:{port}/health");
    match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => match client.get(url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

fn run_gateway_http_server(host: &str, port: u16) -> Result<()> {
    let server = Server::http(format!("{host}:{port}"))
        .map_err(|err| MosaicError::Network(format!("failed to bind gateway server: {err}")))?;
    let started_at = Utc::now();
    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();
        let response = match (method, url.as_str()) {
            (Method::Get, "/health") => Response::from_string(
                json!({
                    "ok": true,
                    "service": "mosaic-gateway",
                    "ts": Utc::now(),
                })
                .to_string(),
            ),
            (Method::Get, "/status") => Response::from_string(
                json!({
                    "ok": true,
                    "service": "mosaic-gateway",
                    "started_at": started_at,
                    "uptime_seconds": (Utc::now() - started_at).num_seconds(),
                })
                .to_string(),
            ),
            (Method::Get, "/discover") => Response::from_string(
                json!({
                    "ok": true,
                    "methods": ["health", "status", "echo", "nodes.run", "nodes.invoke"],
                })
                .to_string(),
            ),
            (Method::Post, "/call") => {
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    Response::from_string(
                        json!({
                            "ok": false,
                            "error": {
                                "code": "invalid_request",
                                "message": "failed to read request body",
                            }
                        })
                        .to_string(),
                    )
                    .with_status_code(400)
                } else {
                    let parsed = serde_json::from_str::<GatewayRequest>(&body);
                    match parsed {
                        Ok(payload) => match payload.method.as_str() {
                            "health" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "service": "mosaic-gateway",
                                        "ts": Utc::now(),
                                    }
                                })
                                .to_string(),
                            ),
                            "status" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "service": "mosaic-gateway",
                                        "started_at": started_at,
                                        "uptime_seconds": (Utc::now() - started_at).num_seconds(),
                                    }
                                })
                                .to_string(),
                            ),
                            "echo" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "echo": payload.params,
                                    }
                                })
                                .to_string(),
                            ),
                            "nodes.run" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "status": "accepted",
                                        "node_id": payload.params.get("node_id").cloned().unwrap_or(Value::Null),
                                        "command": payload.params.get("command").cloned().unwrap_or(Value::Null),
                                    }
                                })
                                .to_string(),
                            ),
                            "nodes.invoke" => Response::from_string(
                                json!({
                                    "ok": true,
                                    "result": {
                                        "ok": true,
                                        "status": "accepted",
                                        "node_id": payload.params.get("node_id").cloned().unwrap_or(Value::Null),
                                        "method": payload.params.get("method").cloned().unwrap_or(Value::Null),
                                        "params": payload.params.get("params").cloned().unwrap_or(Value::Null),
                                    }
                                })
                                .to_string(),
                            ),
                            _ => Response::from_string(
                                json!({
                                    "ok": false,
                                    "error": {
                                        "code": "method_not_found",
                                        "message": format!("unknown method '{}'", payload.method),
                                    }
                                })
                                .to_string(),
                            )
                            .with_status_code(404),
                        },
                        Err(err) => Response::from_string(
                            json!({
                                "ok": false,
                                "error": {
                                    "code": "invalid_request",
                                    "message": format!("invalid JSON request: {err}"),
                                }
                            })
                            .to_string(),
                        )
                        .with_status_code(400),
                    }
                }
            }
            _ => Response::from_string(
                json!({
                    "ok": false,
                    "error": "not_found",
                })
                .to_string(),
            )
            .with_status_code(404),
        };
        let response = response.with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json").map_err(|err| {
                MosaicError::Unknown(format!("failed to create response header: {err:?}"))
            })?,
        );
        let _ = request.respond(response);
    }
    Ok(())
}

fn handle_status(cli: &Cli) -> Result<()> {
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

async fn handle_health(cli: &Cli) -> Result<()> {
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

async fn handle_doctor(cli: &Cli) -> Result<()> {
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

    let security_root = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
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

    emit_checks(cli.json, "doctor", checks)
}

fn run_check(
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

fn emit_checks(json_mode: bool, kind: &str, checks: Vec<BTreeMap<String, Value>>) -> Result<()> {
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

fn resolve_state_paths(project_state: bool) -> Result<StatePaths> {
    let mode = if project_state {
        StateMode::Project
    } else {
        StateMode::Xdg
    };
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
    StatePaths::resolve(mode, &cwd, PROJECT_STATE_DIR)
}

fn resolve_gateway_target(
    gateway_path: &std::path::Path,
    gateway_service_path: &std::path::Path,
) -> Result<(String, u16)> {
    let state: Option<GatewayState> = load_json_file_opt(gateway_path)?;
    if let Some(state) = state {
        return Ok((state.host, state.port));
    }
    let service: Option<GatewayServiceState> = load_json_file_opt(gateway_service_path)?;
    if let Some(service) = service {
        return Ok((service.host, service.port));
    }
    Ok(("127.0.0.1".to_string(), 8787))
}

fn nodes_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("nodes.json")
}

fn devices_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("devices.json")
}

fn pairing_requests_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("pairing-requests.json")
}

fn hooks_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("hooks.json")
}

fn hook_events_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("hook-events")
}

fn hook_events_file_path(data_dir: &std::path::Path, hook_id: &str) -> PathBuf {
    hook_events_dir(data_dir).join(format!("{hook_id}.jsonl"))
}

fn webhooks_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("webhooks.json")
}

fn webhook_events_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("webhook-events")
}

fn webhook_events_file_path(data_dir: &std::path::Path, webhook_id: &str) -> PathBuf {
    webhook_events_dir(data_dir).join(format!("{webhook_id}.jsonl"))
}

fn browser_history_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("browser-history.json")
}

fn cron_jobs_file_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("cron-jobs.json")
}

fn cron_events_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("cron-events")
}

fn cron_events_file_path(data_dir: &std::path::Path, job_id: &str) -> PathBuf {
    cron_events_dir(data_dir).join(format!("{job_id}.jsonl"))
}

fn load_nodes_or_default(path: &std::path::Path) -> Result<Vec<NodeRecord>> {
    let nodes =
        load_json_file_opt::<Vec<NodeRecord>>(path)?.unwrap_or_else(|| vec![default_local_node()]);
    if nodes.is_empty() {
        return Ok(vec![default_local_node()]);
    }
    Ok(nodes)
}

fn save_nodes(path: &std::path::Path, nodes: &[NodeRecord]) -> Result<()> {
    save_json_file(path, &nodes.to_vec())
}

fn load_devices_or_default(path: &std::path::Path) -> Result<Vec<DeviceRecord>> {
    Ok(load_json_file_opt::<Vec<DeviceRecord>>(path)?.unwrap_or_default())
}

fn save_devices(path: &std::path::Path, devices: &[DeviceRecord]) -> Result<()> {
    save_json_file(path, &devices.to_vec())
}

fn load_pairing_requests_or_default(path: &std::path::Path) -> Result<Vec<PairingRequestRecord>> {
    Ok(load_json_file_opt::<Vec<PairingRequestRecord>>(path)?.unwrap_or_default())
}

fn save_pairing_requests(path: &std::path::Path, requests: &[PairingRequestRecord]) -> Result<()> {
    save_json_file(path, &requests.to_vec())
}

fn load_hooks_or_default(path: &std::path::Path) -> Result<Vec<HookRecord>> {
    Ok(load_json_file_opt::<Vec<HookRecord>>(path)?.unwrap_or_default())
}

fn save_hooks(path: &std::path::Path, hooks: &[HookRecord]) -> Result<()> {
    save_json_file(path, &hooks.to_vec())
}

fn load_webhooks_or_default(path: &std::path::Path) -> Result<Vec<WebhookRecord>> {
    Ok(load_json_file_opt::<Vec<WebhookRecord>>(path)?.unwrap_or_default())
}

fn save_webhooks(path: &std::path::Path, webhooks: &[WebhookRecord]) -> Result<()> {
    save_json_file(path, &webhooks.to_vec())
}

fn load_browser_history_or_default(path: &std::path::Path) -> Result<Vec<BrowserVisitRecord>> {
    Ok(load_json_file_opt::<Vec<BrowserVisitRecord>>(path)?.unwrap_or_default())
}

fn save_browser_history(path: &std::path::Path, visits: &[BrowserVisitRecord]) -> Result<()> {
    save_json_file(path, &visits.to_vec())
}

fn load_cron_jobs_or_default(path: &std::path::Path) -> Result<Vec<CronJobRecord>> {
    Ok(load_json_file_opt::<Vec<CronJobRecord>>(path)?.unwrap_or_default())
}

fn save_cron_jobs(path: &std::path::Path, jobs: &[CronJobRecord]) -> Result<()> {
    save_json_file(path, &jobs.to_vec())
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

fn next_pairing_seq() -> u64 {
    PAIRING_REQUEST_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn generate_pairing_request_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("pr-{ts}-{}", next_pairing_seq())
}

fn next_hook_seq() -> u64 {
    HOOK_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn generate_hook_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("hk-{ts}-{}", next_hook_seq())
}

fn next_cron_seq() -> u64 {
    CRON_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn generate_cron_job_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("cj-{ts}-{}", next_cron_seq())
}

fn next_webhook_seq() -> u64 {
    WEBHOOK_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn generate_webhook_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("wh-{ts}-{}", next_webhook_seq())
}

fn next_browser_seq() -> u64 {
    BROWSER_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn generate_browser_visit_id() -> String {
    let ts = Utc::now().timestamp_millis();
    format!("bv-{ts}-{}", next_browser_seq())
}

fn dispatch_system_event(
    cli: &Cli,
    paths: &StatePaths,
    event_name: &str,
    data: Value,
) -> Result<SystemEventDispatch> {
    let store = SystemEventStore::new(system_events_path(&paths.data_dir));
    let event = store.append_event(event_name, data.clone())?;
    let hook_reports = run_hooks_for_system_event(cli, paths, event_name, data)?;
    Ok(SystemEventDispatch {
        event,
        hook_reports,
    })
}

fn run_hooks_for_system_event(
    cli: &Cli,
    paths: &StatePaths,
    event_name: &str,
    data: Value,
) -> Result<Vec<HookExecutionReport>> {
    let hooks_path = hooks_file_path(&paths.data_dir);
    let mut hooks = load_hooks_or_default(&hooks_path)?;
    let mut reports = Vec::new();
    let mut changed = false;

    for index in 0..hooks.len() {
        if !hooks[index].enabled || hooks[index].event != event_name {
            continue;
        }
        let hook = hooks[index].clone();
        let report = execute_hook_command(cli, paths, &hook, "system_event", data.clone())?;
        apply_hook_last_result(&mut hooks, &hook.id, &report);
        reports.push(report);
        changed = true;
    }

    if changed {
        save_hooks(&hooks_path, &hooks)?;
    }
    Ok(reports)
}

fn execute_hook_command(
    cli: &Cli,
    paths: &StatePaths,
    hook: &HookRecord,
    trigger: &str,
    payload: Value,
) -> Result<HookExecutionReport> {
    let cwd = std::env::current_dir().map_err(|err| MosaicError::Io(err.to_string()))?;
    let approval_store = ApprovalStore::new(paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(paths.sandbox_policy_path.clone());
    let runtime_policy = RuntimePolicy {
        approval: approval_store.load_or_default()?,
        sandbox: sandbox_store.load_or_default()?,
    };
    let executor = ToolExecutor::new(RunGuardMode::ConfirmDangerous, Some(runtime_policy));
    let context = ToolContext {
        cwd,
        yes: cli.yes,
        interactive: false,
    };
    let execution = executor.execute(
        "run_cmd",
        json!({
            "command": hook.command.clone(),
        }),
        &context,
    );

    let report = match execution {
        Ok(value) => {
            let output: RunCommandOutput = serde_json::from_value(value)?;
            let ok = output.exit_code == 0;
            HookExecutionReport {
                hook_id: hook.id.clone(),
                hook_name: hook.name.clone(),
                event: hook.event.clone(),
                trigger: trigger.to_string(),
                command: output.command,
                ok,
                exit_code: Some(output.exit_code),
                duration_ms: Some(u64::try_from(output.duration_ms).unwrap_or(u64::MAX)),
                approved_by: Some(output.approved_by),
                error_code: if ok { None } else { Some("tool".to_string()) },
                error: if ok {
                    None
                } else {
                    Some(format!("command exited with status {}", output.exit_code))
                },
                stdout: Some(output.stdout),
                stderr: Some(output.stderr),
            }
        }
        Err(err) => HookExecutionReport {
            hook_id: hook.id.clone(),
            hook_name: hook.name.clone(),
            event: hook.event.clone(),
            trigger: trigger.to_string(),
            command: hook.command.clone(),
            ok: false,
            exit_code: None,
            duration_ms: None,
            approved_by: None,
            error_code: Some(err.code().to_string()),
            error: Some(err.to_string()),
            stdout: None,
            stderr: None,
        },
    };

    append_hook_event(&paths.data_dir, hook, trigger, &report, payload)?;
    Ok(report)
}

fn append_hook_event(
    data_dir: &std::path::Path,
    hook: &HookRecord,
    trigger: &str,
    report: &HookExecutionReport,
    payload: Value,
) -> Result<()> {
    let event = HookEventRecord {
        ts: Utc::now(),
        hook_id: hook.id.clone(),
        hook_name: hook.name.clone(),
        event: hook.event.clone(),
        trigger: trigger.to_string(),
        command: hook.command.clone(),
        delivery_status: if report.ok {
            "success".to_string()
        } else {
            "failed".to_string()
        },
        ok: report.ok,
        exit_code: report.exit_code,
        duration_ms: report.duration_ms,
        approved_by: report.approved_by.clone(),
        error_code: report.error_code.clone(),
        error: report.error.clone(),
        stdout_preview: report
            .stdout
            .as_deref()
            .and_then(|value| preview_text(value, 240)),
        stderr_preview: report
            .stderr
            .as_deref()
            .and_then(|value| preview_text(value, 240)),
        data: payload,
    };
    let path = hook_events_file_path(data_dir, &hook.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string(&event).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to encode hook event {}: {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write as _;
    file.write_all(encoded.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn apply_hook_last_result(hooks: &mut [HookRecord], hook_id: &str, report: &HookExecutionReport) {
    if let Some(hook) = hooks.iter_mut().find(|item| item.id == hook_id) {
        let now = Utc::now();
        hook.updated_at = now;
        hook.last_triggered_at = Some(now);
        hook.last_result = Some(HookLastResult {
            ok: report.ok,
            exit_code: report.exit_code,
            duration_ms: report.duration_ms,
            approved_by: report.approved_by.clone(),
            error_code: report.error_code.clone(),
            error: report.error.clone(),
        });
    }
}

fn hook_execution_error(hook_id: &str, report: &HookExecutionReport) -> MosaicError {
    let message = report
        .error
        .clone()
        .unwrap_or_else(|| format!("hook '{}' execution failed", hook_id));
    match report.error_code.as_deref() {
        Some("config") => MosaicError::Config(message),
        Some("auth") => MosaicError::Auth(message),
        Some("network") => MosaicError::Network(message),
        Some("io") => MosaicError::Io(message),
        Some("validation") => MosaicError::Validation(message),
        Some("gateway_unavailable") => MosaicError::GatewayUnavailable(message),
        Some("gateway_protocol") => MosaicError::GatewayProtocol(message),
        Some("channel_unsupported") => MosaicError::ChannelUnsupported(message),
        Some("approval_required") => MosaicError::ApprovalRequired(message),
        Some("sandbox_denied") => MosaicError::SandboxDenied(message),
        _ => {
            if let Some(code) = report.exit_code {
                MosaicError::Tool(format!("hook '{}' exited with status {}", hook_id, code))
            } else {
                MosaicError::Tool(message)
            }
        }
    }
}

fn read_hook_events(
    data_dir: &std::path::Path,
    hook_id: Option<&str>,
    tail: usize,
) -> Result<Vec<HookEventRecord>> {
    let mut events = Vec::new();
    if let Some(hook_id) = hook_id {
        let path = hook_events_file_path(data_dir, hook_id);
        load_hook_events_file(&path, &mut events)?;
    } else {
        let dir = hook_events_dir(data_dir);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                load_hook_events_file(&path, &mut events)?;
            }
        }
    }
    events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    Ok(events)
}

fn load_hook_events_file(path: &std::path::Path, events: &mut Vec<HookEventRecord>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<HookEventRecord>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid hook event format {}: {err}",
                path.display()
            ))
        })?;
        events.push(event);
    }
    Ok(())
}

fn execute_cron_job(
    cli: &Cli,
    paths: &StatePaths,
    job: &CronJobRecord,
    trigger: &str,
    payload_override: Option<Value>,
) -> Result<CronExecutionReport> {
    let payload = payload_override.unwrap_or_else(|| job.data.clone());
    let dispatch = dispatch_system_event(cli, paths, &job.event, payload.clone());
    let report = match dispatch {
        Ok(dispatch) => {
            let hooks_triggered = dispatch.hook_reports.len();
            let hooks_ok = dispatch.hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hooks_triggered.saturating_sub(hooks_ok);
            let error = if hooks_failed > 0 {
                Some(format!("{hooks_failed} hook execution(s) failed"))
            } else {
                None
            };
            CronExecutionReport {
                job_id: job.id.clone(),
                job_name: job.name.clone(),
                trigger: trigger.to_string(),
                event: job.event.clone(),
                ok: hooks_failed == 0,
                hooks_triggered,
                hooks_ok,
                hooks_failed,
                error,
                system_event: Some(dispatch.event),
            }
        }
        Err(err) => CronExecutionReport {
            job_id: job.id.clone(),
            job_name: job.name.clone(),
            trigger: trigger.to_string(),
            event: job.event.clone(),
            ok: false,
            hooks_triggered: 0,
            hooks_ok: 0,
            hooks_failed: 0,
            error: Some(err.to_string()),
            system_event: None,
        },
    };
    append_cron_event(&paths.data_dir, job, trigger, payload, &report)?;
    Ok(report)
}

fn append_cron_event(
    data_dir: &std::path::Path,
    job: &CronJobRecord,
    trigger: &str,
    payload: Value,
    report: &CronExecutionReport,
) -> Result<()> {
    let event = CronEventRecord {
        ts: Utc::now(),
        job_id: job.id.clone(),
        job_name: job.name.clone(),
        trigger: trigger.to_string(),
        event: job.event.clone(),
        data: payload,
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error: report.error.clone(),
    };
    let path = cron_events_file_path(data_dir, &job.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string(&event).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to encode cron event {}: {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write as _;
    file.write_all(encoded.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn apply_cron_result(
    job: &mut CronJobRecord,
    report: &CronExecutionReport,
    ran_at: DateTime<Utc>,
) -> Result<()> {
    job.updated_at = ran_at;
    job.last_run_at = Some(ran_at);
    job.run_count = job.run_count.saturating_add(1);
    job.next_run_at = next_cron_run_at(ran_at, job.every_seconds)?;
    job.last_result = Some(CronLastResult {
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error: report.error.clone(),
    });
    Ok(())
}

fn next_cron_run_at(from: DateTime<Utc>, every_seconds: u64) -> Result<DateTime<Utc>> {
    if every_seconds == 0 {
        return Err(MosaicError::Validation(
            "cron interval must be greater than 0 seconds".to_string(),
        ));
    }
    let every_seconds = i64::try_from(every_seconds).map_err(|_| {
        MosaicError::Validation("cron interval is too large to schedule".to_string())
    })?;
    Ok(from + ChronoDuration::seconds(every_seconds))
}

fn read_cron_events(
    data_dir: &std::path::Path,
    job_id: Option<&str>,
    tail: usize,
) -> Result<Vec<CronEventRecord>> {
    let mut events = Vec::new();
    if let Some(job_id) = job_id {
        let path = cron_events_file_path(data_dir, job_id);
        load_cron_events_file(&path, &mut events)?;
    } else {
        let dir = cron_events_dir(data_dir);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                load_cron_events_file(&path, &mut events)?;
            }
        }
    }
    events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    Ok(events)
}

fn load_cron_events_file(path: &std::path::Path, events: &mut Vec<CronEventRecord>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<CronEventRecord>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid cron event format {}: {err}",
                path.display()
            ))
        })?;
        events.push(event);
    }
    Ok(())
}

fn normalize_webhook_path(path: &str) -> Result<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(MosaicError::Validation(
            "webhook path cannot be empty".to_string(),
        ));
    }
    if path.contains(' ') || path.contains('\t') || path.contains('\n') || path.contains('\r') {
        return Err(MosaicError::Validation(
            "webhook path cannot contain whitespace".to_string(),
        ));
    }
    if path.starts_with('/') {
        Ok(path.to_string())
    } else {
        Ok(format!("/{path}"))
    }
}

fn normalize_optional_secret_env(secret_env: Option<String>) -> Result<Option<String>> {
    match secret_env {
        None => Ok(None),
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(MosaicError::Validation(
                    "webhook secret env cannot be empty".to_string(),
                ));
            }
            if !is_valid_env_name(trimmed) {
                return Err(MosaicError::Validation(format!(
                    "invalid webhook secret env '{}'",
                    trimmed
                )));
            }
            Ok(Some(trimmed.to_string()))
        }
    }
}

fn is_valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|item| item == '_' || item.is_ascii_alphanumeric())
}

fn verify_webhook_secret(webhook: &WebhookRecord, provided_secret: Option<&str>) -> Result<()> {
    let Some(secret_env) = &webhook.secret_env else {
        return Ok(());
    };
    let expected = std::env::var(secret_env).map_err(|_| {
        MosaicError::Auth(format!(
            "webhook '{}' requires secret env '{}' to be set",
            webhook.id, secret_env
        ))
    })?;
    if expected.is_empty() {
        return Err(MosaicError::Auth(format!(
            "webhook '{}' secret env '{}' is empty",
            webhook.id, secret_env
        )));
    }
    let Some(provided_secret) = provided_secret else {
        return Err(MosaicError::Auth(format!(
            "webhook '{}' requires --secret to trigger",
            webhook.id
        )));
    };
    if provided_secret != expected {
        return Err(MosaicError::Auth(format!(
            "webhook '{}' secret mismatch",
            webhook.id
        )));
    }
    Ok(())
}

fn execute_webhook(
    cli: &Cli,
    paths: &StatePaths,
    webhook: &WebhookRecord,
    trigger: &str,
    payload: Value,
    provided_secret: Option<&str>,
) -> Result<WebhookExecutionReport> {
    let dispatch = match verify_webhook_secret(webhook, provided_secret) {
        Ok(()) => dispatch_system_event(cli, paths, &webhook.event, payload.clone()),
        Err(err) => Err(err),
    };
    let report = match dispatch {
        Ok(dispatch) => {
            let hooks_triggered = dispatch.hook_reports.len();
            let hooks_ok = dispatch.hook_reports.iter().filter(|item| item.ok).count();
            let hooks_failed = hooks_triggered.saturating_sub(hooks_ok);
            let error = if hooks_failed > 0 {
                Some(format!("{hooks_failed} hook execution(s) failed"))
            } else {
                None
            };
            WebhookExecutionReport {
                webhook_id: webhook.id.clone(),
                webhook_name: webhook.name.clone(),
                trigger: trigger.to_string(),
                event: webhook.event.clone(),
                path: webhook.path.clone(),
                method: webhook.method.clone(),
                ok: hooks_failed == 0,
                hooks_triggered,
                hooks_ok,
                hooks_failed,
                error_code: if hooks_failed == 0 {
                    None
                } else {
                    Some("tool".to_string())
                },
                error,
                system_event: Some(dispatch.event),
            }
        }
        Err(err) => WebhookExecutionReport {
            webhook_id: webhook.id.clone(),
            webhook_name: webhook.name.clone(),
            trigger: trigger.to_string(),
            event: webhook.event.clone(),
            path: webhook.path.clone(),
            method: webhook.method.clone(),
            ok: false,
            hooks_triggered: 0,
            hooks_ok: 0,
            hooks_failed: 0,
            error_code: Some(err.code().to_string()),
            error: Some(err.to_string()),
            system_event: None,
        },
    };
    append_webhook_event(&paths.data_dir, webhook, trigger, payload, &report)?;
    Ok(report)
}

fn append_webhook_event(
    data_dir: &std::path::Path,
    webhook: &WebhookRecord,
    trigger: &str,
    payload: Value,
    report: &WebhookExecutionReport,
) -> Result<()> {
    let event = WebhookEventRecord {
        ts: Utc::now(),
        webhook_id: webhook.id.clone(),
        webhook_name: webhook.name.clone(),
        trigger: trigger.to_string(),
        event: webhook.event.clone(),
        path: webhook.path.clone(),
        method: webhook.method.clone(),
        data: payload,
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error_code: report.error_code.clone(),
        error: report.error.clone(),
    };
    let path = webhook_events_file_path(data_dir, &webhook.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_string(&event).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to encode webhook event {}: {err}",
            path.display()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write as _;
    file.write_all(encoded.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn apply_webhook_last_result(webhook: &mut WebhookRecord, report: &WebhookExecutionReport) {
    let now = Utc::now();
    webhook.updated_at = now;
    webhook.last_triggered_at = Some(now);
    webhook.last_result = Some(WebhookLastResult {
        ok: report.ok,
        hooks_triggered: report.hooks_triggered,
        hooks_ok: report.hooks_ok,
        hooks_failed: report.hooks_failed,
        error_code: report.error_code.clone(),
        error: report.error.clone(),
    });
}

fn webhook_execution_error(webhook_id: &str, report: &WebhookExecutionReport) -> MosaicError {
    let message = report
        .error
        .clone()
        .unwrap_or_else(|| format!("webhook '{}' execution failed", webhook_id));
    match report.error_code.as_deref() {
        Some("config") => MosaicError::Config(message),
        Some("auth") => MosaicError::Auth(message),
        Some("network") => MosaicError::Network(message),
        Some("io") => MosaicError::Io(message),
        Some("validation") => MosaicError::Validation(message),
        Some("gateway_unavailable") => MosaicError::GatewayUnavailable(message),
        Some("gateway_protocol") => MosaicError::GatewayProtocol(message),
        Some("channel_unsupported") => MosaicError::ChannelUnsupported(message),
        Some("approval_required") => MosaicError::ApprovalRequired(message),
        Some("sandbox_denied") => MosaicError::SandboxDenied(message),
        _ => MosaicError::Tool(message),
    }
}

fn read_webhook_events(
    data_dir: &std::path::Path,
    webhook_id: Option<&str>,
    tail: usize,
) -> Result<Vec<WebhookEventRecord>> {
    let mut events = Vec::new();
    if let Some(webhook_id) = webhook_id {
        let path = webhook_events_file_path(data_dir, webhook_id);
        load_webhook_events_file(&path, &mut events)?;
    } else {
        let dir = webhook_events_dir(data_dir);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                    continue;
                }
                load_webhook_events_file(&path, &mut events)?;
            }
        }
    }
    events.sort_by(|lhs, rhs| lhs.ts.cmp(&rhs.ts));
    if events.len() > tail {
        let keep_from = events.len() - tail;
        events = events.split_off(keep_from);
    }
    Ok(events)
}

fn load_webhook_events_file(
    path: &std::path::Path,
    events: &mut Vec<WebhookEventRecord>,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<WebhookEventRecord>(line).map_err(|err| {
            MosaicError::Validation(format!(
                "invalid webhook event format {}: {err}",
                path.display()
            ))
        })?;
        events.push(event);
    }
    Ok(())
}

async fn browser_open_visit(url: &str, timeout_ms: u64) -> Result<BrowserVisitRecord> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|err| MosaicError::Validation(format!("invalid browser url '{}': {err}", url)))?;
    let visit_id = generate_browser_visit_id();
    let ts = Utc::now();
    match parsed.scheme() {
        "mock" => Ok(browser_open_mock_visit(visit_id, ts, url, &parsed)),
        "http" | "https" => Ok(browser_open_http_visit(visit_id, ts, url, &parsed, timeout_ms).await),
        scheme => Err(MosaicError::Validation(format!(
            "unsupported browser url scheme '{}', expected http/https/mock",
            scheme
        ))),
    }
}

fn browser_open_mock_visit(
    visit_id: String,
    ts: DateTime<Utc>,
    url: &str,
    parsed: &reqwest::Url,
) -> BrowserVisitRecord {
    let status = resolve_mock_browser_status(parsed);
    let title = parsed
        .query_pairs()
        .find(|(key, _)| key == "title")
        .map(|(_, value)| value.to_string())
        .or_else(|| Some("Mock Page".to_string()));
    let body = format!(
        "<html><head><title>{}</title></head><body>mock browser response status {status}</body></html>",
        title.clone().unwrap_or_else(|| "Mock Page".to_string())
    );
    let ok = (200..300).contains(&status);
    BrowserVisitRecord {
        id: visit_id,
        ts,
        url: url.to_string(),
        ok,
        http_status: Some(status),
        title,
        content_type: Some("text/html; charset=utf-8".to_string()),
        content_length: Some(body.len()),
        preview: preview_text(&body, 240),
        error: if ok {
            None
        } else {
            Some(format!("http status {status}"))
        },
    }
}

fn resolve_mock_browser_status(parsed: &reqwest::Url) -> u16 {
    if let Some(host) = parsed.host_str() {
        if host.eq_ignore_ascii_case("ok") {
            return 200;
        }
        if let Ok(code) = host.parse::<u16>() {
            return code;
        }
        if host.eq_ignore_ascii_case("status") {
            let path = parsed.path().trim_start_matches('/');
            if let Ok(code) = path.parse::<u16>() {
                return code;
            }
        }
    }
    200
}

async fn browser_open_http_visit(
    visit_id: String,
    ts: DateTime<Utc>,
    url: &str,
    parsed: &reqwest::Url,
    timeout_ms: u64,
) -> BrowserVisitRecord {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return BrowserVisitRecord {
                id: visit_id,
                ts,
                url: url.to_string(),
                ok: false,
                http_status: None,
                title: None,
                content_type: None,
                content_length: None,
                preview: None,
                error: Some(format!("failed to build http client: {err}")),
            };
        }
    };

    let response = match client.get(parsed.clone()).send().await {
        Ok(response) => response,
        Err(err) => {
            return BrowserVisitRecord {
                id: visit_id,
                ts,
                url: url.to_string(),
                ok: false,
                http_status: None,
                title: None,
                content_type: None,
                content_length: None,
                preview: None,
                error: Some(format!("request failed: {err}")),
            };
        }
    };

    let status = response.status().as_u16();
    let ok = response.status().is_success();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let body = match response.text().await {
        Ok(body) => body,
        Err(err) => {
            return BrowserVisitRecord {
                id: visit_id,
                ts,
                url: url.to_string(),
                ok: false,
                http_status: Some(status),
                title: None,
                content_type,
                content_length: None,
                preview: None,
                error: Some(format!("failed to read response body: {err}")),
            };
        }
    };
    let title = extract_html_title(&body);
    BrowserVisitRecord {
        id: visit_id,
        ts,
        url: url.to_string(),
        ok,
        http_status: Some(status),
        title,
        content_type,
        content_length: Some(body.len()),
        preview: preview_text(&body, 240),
        error: if ok {
            None
        } else {
            Some(format!("http status {status}"))
        },
    }
}

fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    let title_start = lower.find("<title")?;
    let open_end_rel = lower[title_start..].find('>')?;
    let content_start = title_start + open_end_rel + 1;
    let close_rel = lower[content_start..].find("</title>")?;
    let content_end = content_start + close_rel;
    let title = body[content_start..content_end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn preview_text(value: &str, max_len: usize) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    if text.len() <= max_len {
        return Some(text.to_string());
    }
    let mut clipped = text.chars().take(max_len).collect::<String>();
    clipped.push_str("...");
    Some(clipped)
}

fn parse_json_input(raw: &str, field_name: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|err| {
        MosaicError::Validation(format!(
            "{field_name} must be valid JSON, parse error: {err}"
        ))
    })
}

fn resolve_baseline_path(
    paths: &StatePaths,
    cwd: &std::path::Path,
    raw: Option<String>,
) -> PathBuf {
    raw.map_or_else(
        || paths.root_dir.join("security").join("baseline.toml"),
        |value| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        },
    )
}

fn resolve_output_path(cwd: &std::path::Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn normalize_non_empty_list(values: Vec<String>, field_name: &str) -> Result<Vec<String>> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if normalized.len()
        != normalized
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    {
        normalized.sort();
        normalized.dedup();
    }
    for value in &normalized {
        if value.trim().is_empty() {
            return Err(MosaicError::Validation(format!(
                "{field_name} entry cannot be empty"
            )));
        }
    }
    Ok(normalized)
}

fn remove_matching(target: &mut Vec<String>, values: &[String]) -> usize {
    let before = target.len();
    target.retain(|item| !values.contains(item));
    before.saturating_sub(target.len())
}

fn build_runtime(
    cli: &Cli,
    requested_agent_id: Option<&str>,
    route_hint: Option<&str>,
) -> Result<RuntimeContext> {
    let state_paths = resolve_state_paths(cli.project_state)?;
    state_paths.ensure_dirs()?;
    let manager = ConfigManager::new(state_paths.config_path.clone());
    let config = manager.load()?;
    let agent_store = AgentStore::new(
        agents_file_path(&state_paths.data_dir),
        agent_routes_path(&state_paths.data_dir),
    );
    let mut resolved = agent_store.resolve_effective_profile(
        &config,
        &cli.profile,
        requested_agent_id,
        route_hint,
    )?;
    let model_store = ModelRoutingStore::new(state_paths.models_path.clone());
    let profile_models = model_store.profile(&resolved.profile_name)?;
    resolved.profile.provider.model =
        profile_models.resolve_model_ref(&resolved.profile.provider.model);
    let fallback_models = profile_models
        .fallbacks
        .iter()
        .map(|model| profile_models.resolve_model_ref(model))
        .filter(|model| model != &resolved.profile.provider.model)
        .fold(Vec::<String>::new(), |mut acc, model| {
            if !acc.iter().any(|item| item == &model) {
                acc.push(model);
            }
            acc
        });
    let mut provider: Arc<dyn Provider> =
        Arc::new(OpenAiCompatibleProvider::from_profile(&resolved.profile)?);
    if !fallback_models.is_empty() {
        provider = Arc::new(ModelRoutingProvider::new(provider, fallback_models));
    }
    let session_store = SessionStore::new(state_paths.sessions_dir.clone());
    let audit_store = AuditStore::new(
        state_paths.audit_dir.clone(),
        state_paths.audit_log_path.clone(),
    );
    let approval_store = ApprovalStore::new(state_paths.approvals_policy_path.clone());
    let sandbox_store = SandboxStore::new(state_paths.sandbox_policy_path.clone());
    let tool_executor = ToolExecutor::new(
        resolved.profile.tools.run.guard_mode.clone(),
        Some(RuntimePolicy {
            approval: approval_store.load_or_default()?,
            sandbox: sandbox_store.load_or_default()?,
        }),
    );
    let agent = AgentRunner::new(
        provider.clone(),
        resolved.profile.clone(),
        session_store,
        audit_store,
        tool_executor,
    );
    Ok(RuntimeContext {
        provider,
        agent,
        active_agent_id: resolved.agent_id,
        active_profile_name: resolved.profile_name,
    })
}

fn load_json_file_opt<T>(path: &std::path::Path) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<T>(&raw).map_err(|err| {
        MosaicError::Validation(format!("invalid JSON {}: {err}", path.display()))
    })?;
    Ok(Some(parsed))
}

fn save_json_file<T>(path: &std::path::Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|err| {
        MosaicError::Validation(format!(
            "failed to serialize JSON {}: {err}",
            path.display()
        ))
    })?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn print_json(value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    println!("{rendered}");
}

fn binary_in_path(name: &str) -> bool {
    if PathBuf::from(name).is_absolute() {
        return PathBuf::from(name).exists();
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let full = dir.join(name);
                if full.exists() { Some(full) } else { None }
            })
        })
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    struct StubProvider {
        network_fail_models: HashSet<String>,
        auth_fail_models: HashSet<String>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl Provider for StubProvider {
        async fn list_models(&self) -> Result<Vec<ModelInfo>> {
            Ok(Vec::new())
        }

        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
            self.calls
                .lock()
                .expect("calls lock")
                .push(request.model.clone());
            if self.auth_fail_models.contains(&request.model) {
                return Err(MosaicError::Auth("invalid api key".to_string()));
            }
            if self.network_fail_models.contains(&request.model) {
                return Err(MosaicError::Network("upstream unavailable".to_string()));
            }
            Ok(ChatResponse {
                content: request.model,
            })
        }

        async fn health(&self) -> Result<ProviderHealth> {
            Ok(ProviderHealth {
                ok: true,
                latency_ms: Some(1),
                detail: "ok".to_string(),
            })
        }
    }

    #[test]
    fn cli_accepts_openclaw_aliases() {
        let alias_ask = Cli::try_parse_from(["mosaic", "message", "hello"]).unwrap();
        assert!(matches!(alias_ask.command, Commands::Ask(_)));

        let alias_chat = Cli::try_parse_from(["mosaic", "agent"]).unwrap();
        assert!(matches!(alias_chat.command, Commands::Chat(_)));

        let alias_setup = Cli::try_parse_from(["mosaic", "onboard"]).unwrap();
        assert!(matches!(alias_setup.command, Commands::Setup(_)));
    }

    #[test]
    fn resolve_effective_model_uses_alias_mapping() {
        let mut profile = ModelProfileConfig {
            aliases: BTreeMap::new(),
            fallbacks: vec![],
        };
        profile
            .aliases
            .insert("fast".to_string(), "gpt-4o-mini".to_string());
        let (effective, used_alias) = resolve_effective_model(&profile, "FAST");
        assert_eq!(effective, "gpt-4o-mini");
        assert_eq!(used_alias.as_deref(), Some("fast"));
    }

    #[tokio::test]
    async fn model_routing_provider_falls_back_on_network_error() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = StubProvider {
            network_fail_models: ["primary".to_string()].into_iter().collect(),
            auth_fail_models: HashSet::new(),
            calls: calls.clone(),
        };
        let provider = ModelRoutingProvider::new(
            Arc::new(provider),
            vec!["backup".to_string(), "backup".to_string()],
        );

        let response = provider
            .chat(ChatRequest {
                model: "primary".to_string(),
                temperature: 0.2,
                messages: Vec::new(),
            })
            .await
            .expect("fallback succeeds");

        assert_eq!(response.content, "backup");
        assert_eq!(
            calls.lock().expect("calls lock").as_slice(),
            &["primary".to_string(), "backup".to_string()]
        );
    }

    #[tokio::test]
    async fn model_routing_provider_does_not_retry_auth_error() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let provider = StubProvider {
            network_fail_models: HashSet::new(),
            auth_fail_models: ["primary".to_string()].into_iter().collect(),
            calls: calls.clone(),
        };
        let provider = ModelRoutingProvider::new(Arc::new(provider), vec!["backup".to_string()]);

        let err = provider
            .chat(ChatRequest {
                model: "primary".to_string(),
                temperature: 0.2,
                messages: Vec::new(),
            })
            .await
            .expect_err("auth should fail without fallback");

        assert!(matches!(err, MosaicError::Auth(_)));
        assert_eq!(
            calls.lock().expect("calls lock").as_slice(),
            &["primary".to_string()]
        );
    }
}
