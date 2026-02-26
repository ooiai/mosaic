use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use mosaic_core::config::RunGuardMode;
use mosaic_ops::{ApprovalMode, SandboxProfile, SystemEvent};

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
    #[command(visible_alias = "config")]
    Configure(ConfigureArgs),
    Models(ModelsArgs),
    #[command(visible_alias = "message")]
    Ask(AskArgs),
    #[command(visible_alias = "agent")]
    Chat(ChatArgs),
    #[command(visible_alias = "sessions")]
    Session(SessionArgs),
    #[command(visible_alias = "daemon")]
    Gateway(GatewayArgs),
    Channels(ChannelsArgs),
    #[command(visible_alias = "node")]
    Nodes(NodesArgs),
    Devices(DevicesArgs),
    Pairing(PairingArgs),
    Hooks(HooksArgs),
    Cron(CronArgs),
    Webhooks(WebhooksArgs),
    Browser(BrowserArgs),
    Logs(LogsArgs),
    System(SystemArgs),
    #[command(visible_alias = "acp")]
    Approvals(ApprovalsArgs),
    Sandbox(SandboxArgs),
    Memory(MemoryArgs),
    Security(SecurityArgs),
    Agents(AgentsArgs),
    Plugins(PluginsArgs),
    Skills(SkillsArgs),
    Completion(CompletionArgs),
    Directory,
    Dashboard,
    Update(UpdateArgs),
    Reset,
    Uninstall,
    Docs(DocsArgs),
    Dns(DnsArgs),
    Tui(TuiArgs),
    Qr(QrArgs),
    Clawbot(ClawbotArgs),
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

#[derive(Args, Debug, Clone)]
struct CompletionArgs {
    #[command(subcommand)]
    command: CompletionCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum CompletionCommand {
    Shell {
        #[arg(value_enum)]
        shell: CompletionShellArg,
    },
    Install {
        #[arg(value_enum)]
        shell: CompletionShellArg,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

#[derive(Args, Debug, Clone)]
struct UpdateArgs {
    #[arg(long)]
    check: bool,
    #[arg(long)]
    source: Option<String>,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
}

#[derive(Args, Debug, Clone)]
struct DocsArgs {
    topic: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct DnsArgs {
    #[command(subcommand)]
    command: DnsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum DnsCommand {
    Resolve {
        host: String,
        #[arg(long, default_value_t = 443)]
        port: u16,
    },
}

#[derive(Args, Debug, Clone)]
struct TuiArgs {
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    prompt: Option<String>,
    #[arg(long)]
    agent: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct QrArgs {
    #[command(subcommand)]
    command: QrCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum QrCommand {
    Encode {
        value: String,
        #[arg(long, value_enum, default_value_t = QrRenderArg::Payload)]
        render: QrRenderArg,
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = 2)]
        quiet_zone: u8,
        #[arg(long, default_value_t = 8)]
        module_size: u32,
    },
    Pairing {
        #[arg(long)]
        device: String,
        #[arg(long, default_value = "local")]
        node: String,
        #[arg(long, default_value_t = 900)]
        ttl_seconds: u64,
        #[arg(long, value_enum, default_value_t = QrRenderArg::Payload)]
        render: QrRenderArg,
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = 2)]
        quiet_zone: u8,
        #[arg(long, default_value_t = 8)]
        module_size: u32,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum QrRenderArg {
    Payload,
    Ascii,
    Png,
}

#[derive(Args, Debug, Clone)]
struct ClawbotArgs {
    #[command(subcommand)]
    command: ClawbotCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ClawbotCommand {
    Ask {
        prompt: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Chat {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Send {
        text: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Status,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CompletionShellArg {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Elvish,
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
