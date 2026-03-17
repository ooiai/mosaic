use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use mosaic_core::config::RunGuardMode;
use mosaic_ops::{ApprovalMode, SandboxProfile, SystemEvent};

#[derive(Parser, Debug)]
#[command(
    name = "mosaic",
    version,
    about = "Mosaic local agent CLI",
    after_help = "When no subcommand is provided, `mosaic` launches the interactive TUI."
)]
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
    command: Option<Commands>,
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
    Mcp(McpArgs),
    Channels(ChannelsArgs),
    #[command(visible_alias = "node")]
    Nodes(NodesArgs),
    Devices(DevicesArgs),
    Pairing(PairingArgs),
    Hooks(HooksArgs),
    Cron(CronArgs),
    Webhooks(WebhooksArgs),
    Tts(TtsArgs),
    Voicecall(VoicecallArgs),
    Browser(BrowserArgs),
    Logs(LogsArgs),
    Observability(ObservabilityArgs),
    System(SystemArgs),
    #[command(visible_alias = "acp")]
    Approvals(ApprovalsArgs),
    Sandbox(SandboxArgs),
    Safety(SafetyArgs),
    Memory(MemoryArgs),
    Knowledge(KnowledgeArgs),
    Security(SecurityArgs),
    Agents(AgentsArgs),
    Plugins(PluginsArgs),
    Skills(SkillsArgs),
    Completion(CompletionArgs),
    Directory(DirectoryArgs),
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
    #[command(subcommand)]
    command: Option<ConfigureCommand>,
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

#[derive(Subcommand, Debug, Clone)]
enum ConfigureCommand {
    Keys,
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
    Patch(ConfigurePatchArgs),
    Preview(ConfigurePreviewArgs),
    Template(ConfigureTemplateArgs),
}

#[derive(Args, Debug, Clone)]
struct ConfigurePatchArgs {
    #[arg(long = "set", value_name = "KEY=VALUE", action = ArgAction::Append)]
    set: Vec<String>,
    #[arg(long)]
    file: Option<String>,
    #[arg(long)]
    target_profile: Option<String>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct ConfigurePreviewArgs {
    #[arg(long = "set", value_name = "KEY=VALUE", action = ArgAction::Append)]
    set: Vec<String>,
    #[arg(long)]
    file: Option<String>,
    #[arg(long)]
    target_profile: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct ConfigureTemplateArgs {
    #[arg(long, value_enum, default_value_t = ConfigureTemplateFormatArg::Json)]
    format: ConfigureTemplateFormatArg,
    #[arg(long)]
    defaults: bool,
    #[arg(long)]
    target_profile: Option<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureTemplateFormatArg {
    Json,
    Toml,
}

#[derive(Args, Debug, Clone)]
struct ModelsArgs {
    #[command(subcommand)]
    command: ModelsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ModelsCommand {
    List {
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Status,
    Resolve {
        model: Option<String>,
    },
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
    #[arg(
        value_name = "PROMPT",
        required_unless_present_any = ["prompt_file", "script"],
        conflicts_with_all = ["prompt_file", "script"]
    )]
    prompt: Option<String>,
    #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "script"])]
    prompt_file: Option<String>,
    #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "prompt_file"])]
    script: Option<String>,
    #[arg(long)]
    session: Option<String>,
    #[arg(long)]
    agent: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct ChatArgs {
    #[arg(long)]
    session: Option<String>,
    #[arg(long, conflicts_with_all = ["prompt_file", "script"])]
    prompt: Option<String>,
    #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "script"])]
    prompt_file: Option<String>,
    #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "prompt_file"])]
    script: Option<String>,
    #[arg(long)]
    agent: Option<String>,
    #[arg(long)]
    emit_events: bool,
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

#[derive(Args, Debug, Clone)]
struct McpArgs {
    #[command(subcommand)]
    command: McpCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum McpCommand {
    List,
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        command: String,
        #[arg(long = "arg", action = ArgAction::Append, allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long = "env", value_name = "KEY=VALUE", action = ArgAction::Append)]
        env: Vec<String>,
        #[arg(long = "env-from", value_name = "KEY=ENV_NAME", action = ArgAction::Append)]
        env_from: Vec<String>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        disabled: bool,
    },
    Update {
        server_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        command: Option<String>,
        #[arg(long = "arg", action = ArgAction::Append, allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long)]
        clear_args: bool,
        #[arg(long = "env", value_name = "KEY=VALUE", action = ArgAction::Append)]
        env: Vec<String>,
        #[arg(long)]
        clear_env: bool,
        #[arg(long = "env-from", value_name = "KEY=ENV_NAME", action = ArgAction::Append)]
        env_from: Vec<String>,
        #[arg(long)]
        clear_env_from: bool,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        clear_cwd: bool,
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        #[arg(long, conflicts_with = "enable")]
        disable: bool,
    },
    Show {
        server_id: String,
    },
    Check {
        server_id: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = false)]
        deep: bool,
        #[arg(long, default_value_t = 2_000)]
        timeout_ms: u64,
        #[arg(long)]
        report_out: Option<String>,
    },
    Diagnose {
        server_id: String,
        #[arg(long, default_value_t = 2_000)]
        timeout_ms: u64,
        #[arg(long)]
        report_out: Option<String>,
    },
    Repair {
        server_id: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = 2_000)]
        timeout_ms: u64,
        #[arg(long)]
        clear_missing_cwd: bool,
        #[arg(long = "set-env-from", value_name = "KEY=ENV_NAME", action = ArgAction::Append)]
        set_env_from: Vec<String>,
        #[arg(long)]
        report_out: Option<String>,
    },
    Enable {
        server_id: String,
    },
    Disable {
        server_id: String,
    },
    Remove {
        server_id: String,
    },
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
        #[arg(long, alias = "auto-repair")]
        repair: bool,
    },
    Call {
        method: String,
        #[arg(long)]
        params: Option<String>,
    },
    Probe,
    Discover,
    Diagnose {
        #[arg(long)]
        method: Option<String>,
        #[arg(long)]
        params: Option<String>,
    },
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
    Diagnose {
        node_id: Option<String>,
        #[arg(long, default_value_t = 30)]
        stale_after_minutes: u64,
        #[arg(long)]
        repair: bool,
        #[arg(long)]
        report_out: Option<String>,
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
    Reject {
        request_id: String,
        #[arg(long)]
        reason: Option<String>,
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
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long)]
        summary: bool,
    },
    Replay {
        #[arg(long)]
        hook: Option<String>,
        #[arg(long, default_value_t = 200)]
        tail: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        batch_size: Option<usize>,
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long = "reason", value_enum, action = ArgAction::Append)]
        reasons: Vec<AutomationReplayReasonArg>,
        #[arg(long)]
        retryable_only: bool,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        max_apply: Option<usize>,
        #[arg(long)]
        stop_on_error: bool,
        #[arg(long)]
        report_out: Option<PathBuf>,
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
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long)]
        summary: bool,
    },
    Replay {
        #[arg(long)]
        job: Option<String>,
        #[arg(long, default_value_t = 200)]
        tail: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        batch_size: Option<usize>,
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long = "reason", value_enum, action = ArgAction::Append)]
        reasons: Vec<AutomationReplayReasonArg>,
        #[arg(long)]
        retryable_only: bool,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        max_apply: Option<usize>,
        #[arg(long)]
        stop_on_error: bool,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
}

#[derive(Args, Debug, Clone)]
struct WebhooksArgs {
    #[command(subcommand)]
    command: WebhooksCommand,
}

#[derive(Args, Debug, Clone)]
struct TtsArgs {
    #[command(subcommand)]
    command: TtsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum TtsCommand {
    Voices,
    Speak {
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "alloy")]
        voice: String,
        #[arg(long, default_value = "wav")]
        format: String,
        #[arg(long)]
        out: Option<String>,
    },
    Diagnose {
        #[arg(long, default_value = "alloy")]
        voice: String,
        #[arg(long, default_value = "wav")]
        format: String,
        #[arg(long, default_value = "mosaic tts diagnose probe")]
        text: String,
        #[arg(long)]
        out: Option<String>,
        #[arg(long, default_value_t = 3_000)]
        timeout_ms: u64,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
}

#[derive(Args, Debug, Clone)]
struct VoicecallArgs {
    #[command(subcommand)]
    command: VoicecallCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum VoicecallCommand {
    Start {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        channel_id: Option<String>,
    },
    Status,
    Send {
        #[arg(long)]
        text: String,
        #[arg(long)]
        parse_mode: Option<String>,
        #[arg(long)]
        token_env: Option<String>,
    },
    History {
        #[arg(long, default_value_t = 20)]
        tail: usize,
    },
    Stop,
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
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long)]
        summary: bool,
    },
    Replay {
        #[arg(long)]
        webhook: Option<String>,
        #[arg(long, default_value_t = 200)]
        tail: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        batch_size: Option<usize>,
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long = "reason", value_enum, action = ArgAction::Append)]
        reasons: Vec<AutomationReplayReasonArg>,
        #[arg(long)]
        retryable_only: bool,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        max_apply: Option<usize>,
        #[arg(long)]
        stop_on_error: bool,
        #[arg(long)]
        report_out: Option<PathBuf>,
    },
}

#[derive(Args, Debug, Clone)]
struct BrowserArgs {
    #[command(subcommand)]
    command: BrowserCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum BrowserCommand {
    Start,
    Stop,
    Status,
    Diagnose {
        #[arg(long, default_value_t = 120)]
        stale_after_minutes: u64,
        #[arg(long = "probe-url", action = ArgAction::Append)]
        probe_urls: Vec<String>,
        #[arg(long)]
        probe_file: Option<String>,
        #[arg(long, default_value_t = 5_000)]
        probe_timeout_ms: u64,
        #[arg(long)]
        artifact_max_age_hours: Option<u64>,
        #[arg(long)]
        report_out: Option<String>,
        #[arg(long)]
        repair: bool,
    },
    #[command(visible_alias = "visit")]
    Open {
        #[arg(long)]
        url: String,
        #[arg(long, default_value_t = 10_000)]
        timeout_ms: u64,
    },
    Navigate {
        #[arg(long)]
        url: String,
        #[arg(long, default_value_t = 10_000)]
        timeout_ms: u64,
    },
    History {
        #[arg(long, default_value_t = 20)]
        tail: usize,
    },
    Tabs {
        #[arg(long, default_value_t = 20)]
        tail: usize,
    },
    Show {
        visit_id: String,
    },
    Focus {
        visit_id: String,
    },
    Snapshot {
        visit_id: Option<String>,
    },
    Screenshot {
        visit_id: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    Close {
        visit_id: Option<String>,
        #[arg(long)]
        all: bool,
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
        #[arg(long)]
        summary: bool,
    },
    Replay {
        channel_id: String,
        #[arg(long, default_value_t = 50)]
        tail: usize,
        #[arg(long)]
        since_minutes: Option<u64>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        batch_size: Option<usize>,
        #[arg(long)]
        min_attempt: Option<usize>,
        #[arg(long = "http-status", action = ArgAction::Append)]
        http_statuses: Vec<u16>,
        #[arg(long)]
        include_non_retryable: bool,
        #[arg(long = "reason", value_enum, action = ArgAction::Append)]
        reasons: Vec<ReplayReasonArg>,
        #[arg(long)]
        apply: bool,
        #[arg(long, requires = "apply")]
        max_apply: Option<usize>,
        #[arg(long, requires = "apply")]
        require_full_payload: bool,
        #[arg(long, requires = "apply")]
        stop_on_error: bool,
        #[arg(long)]
        report_out: Option<PathBuf>,
        #[arg(long)]
        token_env: Option<String>,
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
    #[arg(long)]
    source: Option<String>,
}

#[derive(Args, Debug, Clone)]
struct ObservabilityArgs {
    #[command(subcommand)]
    command: ObservabilityCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ObservabilityCommand {
    Report {
        #[arg(long, default_value_t = 100)]
        tail: usize,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, default_value_t = 50)]
        event_tail: usize,
        #[arg(long, default_value_t = 50)]
        audit_tail: usize,
        #[arg(long, default_value_t = 0)]
        compare_window: usize,
        #[arg(long)]
        event_name: Option<String>,
        #[arg(long)]
        no_doctor: bool,
        #[arg(long)]
        plugin_soak_report: Option<PathBuf>,
    },
    Export {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 100)]
        tail: usize,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, default_value_t = 50)]
        event_tail: usize,
        #[arg(long, default_value_t = 50)]
        audit_tail: usize,
        #[arg(long, default_value_t = 0)]
        compare_window: usize,
        #[arg(long)]
        event_name: Option<String>,
        #[arg(long)]
        no_doctor: bool,
        #[arg(long)]
        plugin_soak_report: Option<PathBuf>,
    },
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
    List {
        #[arg(long, default_value_t = 50)]
        tail: usize,
        #[arg(long)]
        name: Option<String>,
    },
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
    Check {
        #[arg(long)]
        command: String,
    },
    Allowlist {
        #[command(subcommand)]
        command: AllowlistCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum AllowlistCommand {
    List,
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
    Get,
    Set {
        #[arg(value_enum)]
        profile: SandboxProfileArg,
    },
    Check {
        #[arg(long)]
        command: String,
    },
    List,
    Explain {
        #[arg(long, value_enum)]
        profile: Option<SandboxProfileArg>,
    },
}

#[derive(Args, Debug, Clone)]
struct SafetyArgs {
    #[command(subcommand)]
    command: SafetyCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SafetyCommand {
    Get,
    Check {
        #[arg(long)]
        command: String,
    },
    Report {
        #[arg(long)]
        command: Option<String>,
        #[arg(long, default_value_t = 50)]
        audit_tail: usize,
        #[arg(long, default_value_t = 0)]
        compare_window: usize,
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
        #[arg(long, default_value = "default")]
        namespace: String,
        #[arg(long, default_value_t = false)]
        incremental: bool,
        #[arg(long)]
        stale_after_hours: Option<u64>,
        #[arg(long, default_value_t = false)]
        retain_missing: bool,
        #[arg(long, default_value_t = 500)]
        max_files: usize,
        #[arg(long, default_value_t = 262_144)]
        max_file_size: usize,
        #[arg(long, default_value_t = 16_384)]
        max_content_bytes: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value = "default")]
        namespace: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Status {
        #[arg(long, default_value = "default")]
        namespace: String,
        #[arg(long, default_value_t = false)]
        all_namespaces: bool,
    },
    Clear {
        #[arg(long, default_value = "default")]
        namespace: String,
    },
    Prune {
        #[arg(long)]
        max_namespaces: Option<usize>,
        #[arg(long)]
        max_age_hours: Option<u64>,
        #[arg(long)]
        max_documents_per_namespace: Option<usize>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Policy {
        #[command(subcommand)]
        command: MemoryPolicyCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum MemoryPolicyCommand {
    Get,
    Set {
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long)]
        max_namespaces: Option<usize>,
        #[arg(long)]
        max_age_hours: Option<u64>,
        #[arg(long)]
        max_documents_per_namespace: Option<usize>,
        #[arg(long)]
        min_interval_minutes: Option<u64>,
        #[arg(long, default_value_t = false)]
        clear_limits: bool,
    },
    Apply {
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct KnowledgeArgs {
    #[command(subcommand)]
    command: KnowledgeCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum KnowledgeCommand {
    Ingest {
        #[arg(long, value_enum, default_value_t = KnowledgeSourceArg::LocalMd)]
        source: KnowledgeSourceArg,
        #[arg(long)]
        path: Option<String>,
        #[arg(long = "url", action = ArgAction::Append)]
        urls: Vec<String>,
        #[arg(long)]
        url_file: Option<String>,
        #[arg(long, default_value_t = false)]
        continue_on_error: bool,
        #[arg(long)]
        report_out: Option<String>,
        #[arg(long = "header", value_name = "KEY=VALUE", action = ArgAction::Append)]
        headers: Vec<String>,
        #[arg(long = "header-env", value_name = "KEY=ENV", action = ArgAction::Append)]
        header_envs: Vec<String>,
        #[arg(long)]
        mcp_server: Option<String>,
        #[arg(long)]
        mcp_path: Option<String>,
        #[arg(long, default_value = "knowledge")]
        namespace: String,
        #[arg(long, default_value_t = 20_000)]
        max_chunk_bytes: usize,
        #[arg(long, default_value_t = 2_000)]
        chunk_overlap_bytes: usize,
        #[arg(long, default_value_t = false)]
        incremental: bool,
        #[arg(long)]
        stale_after_hours: Option<u64>,
        #[arg(long, default_value_t = false)]
        retain_missing: bool,
        #[arg(long, default_value_t = 50_000)]
        max_files: usize,
        #[arg(long, default_value_t = 1_048_576)]
        max_file_size: usize,
        #[arg(long, default_value_t = 16_384)]
        max_content_bytes: usize,
        #[arg(long, default_value_t = 15)]
        http_timeout_seconds: u64,
        #[arg(long, default_value_t = 3)]
        http_retries: u32,
        #[arg(long, default_value_t = 200)]
        http_retry_backoff_ms: u64,
    },
    Search {
        query: String,
        #[arg(long, default_value = "knowledge")]
        namespace: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, default_value_t = 1)]
        min_score: usize,
    },
    Ask {
        prompt: String,
        #[arg(long, default_value = "knowledge")]
        namespace: String,
        #[arg(long, default_value_t = 8)]
        top_k: usize,
        #[arg(long, default_value_t = 1)]
        min_score: usize,
        #[arg(long, default_value_t = false)]
        references_only: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Evaluate {
        #[arg(long = "query", action = ArgAction::Append)]
        queries: Vec<String>,
        #[arg(long)]
        query_file: Option<String>,
        #[arg(long, default_value = "knowledge")]
        namespace: String,
        #[arg(long, default_value_t = 8)]
        top_k: usize,
        #[arg(long, default_value_t = 1)]
        min_score: usize,
        #[arg(long)]
        baseline: Option<String>,
        #[arg(long)]
        no_baseline: bool,
        #[arg(long)]
        update_baseline: bool,
        #[arg(long, default_value_t = false)]
        fail_on_regression: bool,
        #[arg(long, default_value_t = 0.0)]
        max_coverage_drop: f64,
        #[arg(long, default_value_t = 0.0)]
        max_avg_top_score_drop: f64,
        #[arg(long, default_value_t = 20)]
        history_window: usize,
        #[arg(long)]
        report_out: Option<String>,
    },
    Datasets {
        #[command(subcommand)]
        command: KnowledgeDatasetsCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum KnowledgeDatasetsCommand {
    List {
        #[arg(long)]
        namespace: Option<String>,
    },
    Remove {
        namespace: String,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
enum KnowledgeSourceArg {
    LocalMd,
    Http,
    Mcp,
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
        #[arg(long, value_enum)]
        min_severity: Option<SecuritySeverityArg>,
        #[arg(long = "category", action = ArgAction::Append)]
        categories: Vec<String>,
        #[arg(long)]
        top: Option<usize>,
    },
    Baseline {
        #[command(subcommand)]
        command: SecurityBaselineCommand,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum SecuritySeverityArg {
    Low,
    Medium,
    High,
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
    Current {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        route: Option<String>,
    },
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long = "skill", action = ArgAction::Append)]
        skills: Vec<String>,
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
        #[arg(long = "skill", action = ArgAction::Append)]
        skills: Vec<String>,
        #[arg(long)]
        clear_skills: bool,
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
    List {
        #[arg(long, value_enum, default_value_t = ExtensionSourceFilterArg::All)]
        source: ExtensionSourceFilterArg,
    },
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
    Enable {
        plugin_id: String,
    },
    Disable {
        plugin_id: String,
    },
    Doctor,
    Run {
        plugin_id: String,
        #[arg(long, value_enum, default_value_t = PluginHookArg::Run)]
        hook: PluginHookArg,
        #[arg(long)]
        timeout_ms: Option<u64>,
        #[arg(long = "arg", action = ArgAction::Append)]
        args: Vec<String>,
    },
    Remove {
        plugin_id: String,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum PluginHookArg {
    Run,
    Doctor,
}

#[derive(Args, Debug, Clone)]
struct SkillsArgs {
    #[command(subcommand)]
    command: SkillsCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum SkillsCommand {
    List {
        #[arg(long, value_enum, default_value_t = ExtensionSourceFilterArg::All)]
        source: ExtensionSourceFilterArg,
    },
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
struct DirectoryArgs {
    #[arg(long)]
    ensure: bool,
    #[arg(long)]
    check_writable: bool,
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
    #[arg(long, value_enum, default_value_t = TuiFocusArg::Input)]
    focus: TuiFocusArg,
    #[arg(long)]
    no_inspector: bool,
}

impl Default for TuiArgs {
    fn default() -> Self {
        Self {
            session: None,
            prompt: None,
            agent: None,
            focus: TuiFocusArg::Input,
            no_inspector: false,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum TuiFocusArg {
    Messages,
    Input,
    Sessions,
    Inspector,
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

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ExtensionSourceFilterArg {
    All,
    Project,
    CodexHome,
    UserHome,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ReplayReasonArg {
    #[value(name = "rate_limited")]
    RateLimited,
    #[value(name = "upstream_5xx")]
    Upstream5xx,
    #[value(name = "timeout")]
    Timeout,
    #[value(name = "auth")]
    Auth,
    #[value(name = "target_not_found")]
    TargetNotFound,
    #[value(name = "client_4xx")]
    Client4xx,
    #[value(name = "unknown")]
    Unknown,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum AutomationReplayReasonArg {
    #[value(name = "approval_required")]
    ApprovalRequired,
    #[value(name = "sandbox_denied")]
    SandboxDenied,
    #[value(name = "auth")]
    Auth,
    #[value(name = "validation")]
    Validation,
    #[value(name = "tool")]
    Tool,
    #[value(name = "hook_failures")]
    HookFailures,
    #[value(name = "unknown")]
    Unknown,
}

#[derive(Args, Debug, Clone)]
struct ClawbotArgs {
    #[command(subcommand)]
    command: ClawbotCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ClawbotCommand {
    Ask {
        #[arg(
            value_name = "PROMPT",
            required_unless_present_any = ["prompt_file", "script"],
            conflicts_with_all = ["prompt_file", "script"]
        )]
        prompt: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "script"])]
        prompt_file: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "prompt_file"])]
        script: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Chat {
        #[arg(long)]
        session: Option<String>,
        #[arg(long, conflicts_with_all = ["prompt_file", "script"])]
        prompt: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "script"])]
        prompt_file: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with_all = ["prompt", "prompt_file"])]
        script: Option<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    Send {
        #[arg(
            value_name = "TEXT",
            required_unless_present = "text_file",
            conflicts_with = "text_file"
        )]
        text: Option<String>,
        #[arg(long, value_name = "PATH", conflicts_with = "text")]
        text_file: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrowserRuntimeState {
    running: bool,
    started_at: Option<DateTime<Utc>>,
    stopped_at: Option<DateTime<Utc>>,
    active_visit_id: Option<String>,
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
