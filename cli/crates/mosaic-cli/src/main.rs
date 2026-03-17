#![recursion_limit = "256"]

use std::sync::atomic::AtomicU64;

use serde_json::json;

use mosaic_core::config::DEFAULT_PROFILE;
#[cfg(test)]
use mosaic_core::error::MosaicError;
use mosaic_core::error::Result;
#[cfg(test)]
use mosaic_core::models::ModelProfileConfig;
#[cfg(test)]
use mosaic_core::provider::{ChatRequest, ChatResponse, ModelInfo, Provider, ProviderHealth};
use mosaic_gateway::HttpGatewayClient;

mod agents_command;
mod automation_commands;
mod automation_runtime;
mod browser_runtime;
mod channels_command;
mod compat_commands;
mod core_commands;
mod devices_pairing_command;
mod diagnostics_command;
mod discovery_commands;
mod feature_commands;
mod gateway_command;
mod gateway_runtime;
mod knowledge_command;
mod maintenance_commands;
mod mcp_command;
mod nodes_command;
mod nodes_telemetry;
mod ops_command;
mod runtime_context;
mod security_command;
mod state_records;
#[cfg(test)]
mod tests;
mod tts_voicecall_command;
mod tui_command;
mod utils;

use agents_command::handle_agents;
use automation_commands::{handle_cron, handle_hooks, handle_webhooks};
use automation_runtime::{
    apply_cron_result, apply_hook_last_result, apply_webhook_last_result, dispatch_system_event,
    execute_cron_job, execute_hook_command, execute_webhook, hook_execution_error,
    normalize_optional_secret_env, normalize_webhook_path, read_cron_events, read_hook_events,
    read_webhook_events, webhook_execution_error,
};
use browser_runtime::browser_open_visit;
use channels_command::handle_channels;
use compat_commands::{handle_completion, handle_dashboard, handle_directory};
use core_commands::{
    handle_ask, handle_chat, handle_configure, handle_models, handle_session, handle_setup,
};
use devices_pairing_command::{handle_devices, handle_pairing};
use diagnostics_command::{emit_checks, handle_doctor, handle_health, handle_status, run_check};
use discovery_commands::{handle_dns, handle_docs, handle_qr};
use feature_commands::{handle_browser, handle_memory, handle_plugins, handle_skills};
use gateway_command::handle_gateway;
use gateway_runtime::{
    collect_gateway_runtime_status, dispatch_gateway_call, gateway_test_mode,
    resolve_gateway_start_target, resolve_gateway_target, run_gateway_http_server,
    start_gateway_runtime, stop_gateway_runtime, upsert_gateway_service,
};
use knowledge_command::handle_knowledge;
use maintenance_commands::{handle_reset, handle_uninstall, handle_update};
use mcp_command::handle_mcp;
use nodes_command::handle_nodes;
use nodes_telemetry::{NodeTelemetryEventInput, nodes_events_file_path, write_nodes_event};
use ops_command::{
    handle_approvals, handle_logs, handle_observability, handle_safety, handle_sandbox,
    handle_system,
};
#[cfg(test)]
use runtime_context::ModelRoutingProvider;
use runtime_context::{build_runtime, resolve_effective_model, resolve_state_paths};
use security_command::handle_security;
use state_records::{
    browser_history_file_path, browser_state_file_path, cron_events_dir, cron_events_file_path,
    cron_jobs_file_path, devices_file_path, generate_browser_visit_id, generate_cron_job_id,
    generate_hook_id, generate_pairing_request_id, generate_webhook_id, hook_events_dir,
    hook_events_file_path, hooks_file_path, load_browser_history_or_default,
    load_browser_state_or_default, load_cron_jobs_or_default, load_devices_or_default,
    load_hooks_or_default, load_nodes_or_default, load_pairing_requests_or_default,
    load_webhooks_or_default, next_pairing_seq, nodes_file_path, pairing_requests_file_path,
    save_browser_history, save_browser_state, save_cron_jobs, save_devices, save_hooks, save_nodes,
    save_pairing_requests, save_webhooks, webhook_events_dir, webhook_events_file_path,
    webhooks_file_path,
};
use tts_voicecall_command::{handle_tts, handle_voicecall};
use tui_command::handle_tui;
use utils::{
    binary_in_path, load_json_file_opt, normalize_non_empty_list, parse_json_input, preview_text,
    print_json, print_json_line, remove_matching, resolve_baseline_path, resolve_output_path,
    save_json_file,
};

const PROJECT_STATE_DIR: &str = ".mosaic";
static PAIRING_REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);
static HOOK_SEQ: AtomicU64 = AtomicU64::new(1);
static CRON_SEQ: AtomicU64 = AtomicU64::new(1);
static WEBHOOK_SEQ: AtomicU64 = AtomicU64::new(1);
static BROWSER_SEQ: AtomicU64 = AtomicU64::new(1);

include!("cli_schema.rs");

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
        None => handle_tui(&cli, TuiArgs::default()).await,
        Some(Commands::Setup(args)) => handle_setup(&cli, args),
        Some(Commands::Configure(args)) => handle_configure(&cli, args),
        Some(Commands::Models(args)) => handle_models(&cli, args).await,
        Some(Commands::Ask(args)) => handle_ask(&cli, args).await,
        Some(Commands::Chat(args)) => handle_chat(&cli, args).await,
        Some(Commands::Session(args)) => handle_session(&cli, args).await,
        Some(Commands::Gateway(args)) => handle_gateway(&cli, args).await,
        Some(Commands::Mcp(args)) => handle_mcp(&cli, args),
        Some(Commands::Channels(args)) => handle_channels(&cli, args).await,
        Some(Commands::Nodes(args)) => handle_nodes(&cli, args).await,
        Some(Commands::Devices(args)) => handle_devices(&cli, args),
        Some(Commands::Pairing(args)) => handle_pairing(&cli, args),
        Some(Commands::Hooks(args)) => handle_hooks(&cli, args),
        Some(Commands::Cron(args)) => handle_cron(&cli, args),
        Some(Commands::Webhooks(args)) => handle_webhooks(&cli, args),
        Some(Commands::Tts(args)) => handle_tts(&cli, args),
        Some(Commands::Voicecall(args)) => handle_voicecall(&cli, args).await,
        Some(Commands::Browser(args)) => handle_browser(&cli, args).await,
        Some(Commands::Logs(args)) => handle_logs(&cli, args).await,
        Some(Commands::Observability(args)) => handle_observability(&cli, args).await,
        Some(Commands::System(args)) => handle_system(&cli, args),
        Some(Commands::Approvals(args)) => handle_approvals(&cli, args),
        Some(Commands::Sandbox(args)) => handle_sandbox(&cli, args),
        Some(Commands::Safety(args)) => handle_safety(&cli, args),
        Some(Commands::Memory(args)) => handle_memory(&cli, args),
        Some(Commands::Knowledge(args)) => handle_knowledge(&cli, args).await,
        Some(Commands::Security(args)) => handle_security(&cli, args),
        Some(Commands::Agents(args)) => handle_agents(&cli, args),
        Some(Commands::Plugins(args)) => handle_plugins(&cli, args),
        Some(Commands::Skills(args)) => handle_skills(&cli, args),
        Some(Commands::Completion(args)) => handle_completion(&cli, args),
        Some(Commands::Directory(args)) => handle_directory(&cli, args),
        Some(Commands::Dashboard) => handle_dashboard(&cli),
        Some(Commands::Update(args)) => handle_update(&cli, args).await,
        Some(Commands::Reset) => handle_reset(&cli),
        Some(Commands::Uninstall) => handle_uninstall(&cli),
        Some(Commands::Docs(args)) => handle_docs(&cli, args),
        Some(Commands::Dns(args)) => handle_dns(&cli, args),
        Some(Commands::Tui(args)) => handle_tui(&cli, args).await,
        Some(Commands::Qr(args)) => handle_qr(&cli, args),
        Some(Commands::Clawbot(args)) => match args.command {
            ClawbotCommand::Ask {
                prompt,
                prompt_file,
                script,
                session,
                agent,
            } => {
                handle_ask(
                    &cli,
                    AskArgs {
                        prompt,
                        prompt_file,
                        script,
                        session,
                        agent,
                    },
                )
                .await
            }
            ClawbotCommand::Chat {
                session,
                prompt,
                prompt_file,
                script,
                agent,
            } => {
                handle_chat(
                    &cli,
                    ChatArgs {
                        session,
                        prompt,
                        prompt_file,
                        script,
                        agent,
                        emit_events: false,
                    },
                )
                .await
            }
            ClawbotCommand::Send {
                text,
                text_file,
                session,
                agent,
            } => {
                handle_ask(
                    &cli,
                    AskArgs {
                        prompt: text,
                        prompt_file: text_file,
                        script: None,
                        session,
                        agent,
                    },
                )
                .await
            }
            ClawbotCommand::Status => handle_status(&cli),
        },
        Some(Commands::Status) => handle_status(&cli),
        Some(Commands::Health) => handle_health(&cli).await,
        Some(Commands::Doctor) => handle_doctor(&cli).await,
    }
}
