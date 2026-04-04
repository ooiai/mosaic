//! Mosaic terminal UI crate.
//! This crate owns the single-shell terminal operator surface: compact header,
//! scrolling transcript, bottom composer, transient slash popup, and the first
//! event-loop boundary for local Gateway-backed interaction.

pub mod app;
pub mod app_event;
pub mod bottom_pane;
pub mod chat_widget;
pub mod command_popup;
pub mod composer;
pub mod diff_render;
pub mod highlight;
pub mod history_cell;
pub mod markdown;
pub mod mock;
pub mod overlays;
pub mod shell_view;
pub mod status_bar;
pub mod transcript;
pub mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{
    fs::File,
    io::{self, IsTerminal, Read},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use chrono::Utc;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mosaic_control_protocol::{
    GatewayEvent, HealthResponse, IngressTrace, ReadinessResponse, SessionDetailDto,
    SessionSummaryDto, TranscriptRoleDto,
};
use mosaic_gateway::{GatewayHandle, GatewayRunRequest};
use mosaic_node_protocol::DEFAULT_STALE_AFTER_SECS;
use mosaic_runtime::events::{RunEvent, RunEventSink};
use mosaic_sdk::GatewayClient;
#[cfg(test)]
use mosaic_session_core::SessionStore;
use mosaic_session_core::{
    SessionGatewayMetadata, SessionRecord as StoredSessionRecord, SessionSummary,
    TranscriptMessage, TranscriptRole,
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::runtime::Handle;

use self::app::{
    App, AppAction, ComposerRunRequest, ProfileOption, SessionRecord as UiSessionRecord,
    SessionState, SkillOption,
};

enum InputSource {
    Crossterm,
    Pipe(Receiver<InputEvent>),
}

enum InputEvent {
    Key(KeyEvent),
    Scroll(i32),
}

#[derive(Clone)]
pub enum InteractiveGateway {
    Local(GatewayHandle),
    Remote(GatewayClient),
}

#[derive(Clone, Default)]
pub struct TuiEventBuffer {
    inner: Arc<Mutex<Vec<RunEvent>>>,
}

impl TuiEventBuffer {
    pub fn push(&self, event: RunEvent) {
        if let Ok(mut events) = self.inner.lock() {
            events.push(event);
        }
    }

    pub fn drain(&self) -> Vec<RunEvent> {
        if let Ok(mut events) = self.inner.lock() {
            return events.drain(..).collect();
        }

        Vec::new()
    }
}

pub struct TuiEventSink {
    buffer: TuiEventBuffer,
}

impl TuiEventSink {
    pub fn new(buffer: TuiEventBuffer) -> Self {
        Self { buffer }
    }
}

impl RunEventSink for TuiEventSink {
    fn emit(&self, event: RunEvent) {
        self.buffer.push(event);
    }
}

#[derive(Clone)]
pub struct InteractiveSessionContext {
    pub gateway: InteractiveGateway,
    pub runtime_handle: Handle,
    pub event_buffer: TuiEventBuffer,
    pub session_id: String,
    pub system: Option<String>,
    pub active_profile: String,
    pub active_model: String,
    pub available_profiles: Vec<ProfileOption>,
    pub available_skills: Vec<SkillOption>,
    pub extension_summary: String,
    pub extension_policy_summary: String,
    pub extension_errors: Vec<String>,
}

pub fn build_tui_event_buffer() -> TuiEventBuffer {
    TuiEventBuffer::default()
}

pub fn build_tui_event_sink(buffer: TuiEventBuffer) -> Arc<dyn RunEventSink> {
    Arc::new(TuiEventSink::new(buffer))
}

fn session_state_from_run_label(status: &str) -> SessionState {
    match status {
        "queued" | "running" | "streaming" | "cancel_requested" => SessionState::Active,
        "failed" | "canceled" => SessionState::Degraded,
        _ => SessionState::Waiting,
    }
}

pub fn run(start_in_resume: bool) -> io::Result<()> {
    let buffer = build_tui_event_buffer();
    run_with_event_buffer(start_in_resume, buffer)
}

pub fn run_with_event_buffer(
    start_in_resume: bool,
    event_buffer: TuiEventBuffer,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, start_in_resume, event_buffer, false);
    restore_terminal(&mut terminal)?;
    result
}

pub fn run_until_complete_with_event_buffer(
    start_in_resume: bool,
    event_buffer: TuiEventBuffer,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, start_in_resume, event_buffer, true);
    restore_terminal(&mut terminal)?;
    result
}

pub fn run_interactive_session(
    start_in_resume: bool,
    context: InteractiveSessionContext,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_interactive_app(&mut terminal, start_in_resume, context);
    restore_terminal(&mut terminal)?;
    result
}

fn build_app(workspace_path: std::path::PathBuf, start_in_resume: bool) -> App {
    if start_in_resume {
        App::new_with_resume(workspace_path, true)
    } else {
        App::new(workspace_path)
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    start_in_resume: bool,
    event_buffer: TuiEventBuffer,
    exit_on_run_completion: bool,
) -> io::Result<()> {
    let workspace_path = std::env::current_dir()?;
    let mut app = build_app(workspace_path, start_in_resume);
    app.git_branch = detect_git_branch();
    let input_source = build_input_source();

    loop {
        let saw_terminal_event = drain_run_events(&mut app, &event_buffer);

        terminal.draw(|frame| ui::render(frame, &app))?;

        if exit_on_run_completion && saw_terminal_event {
            break;
        }

        match poll_input_event(&input_source, Duration::from_millis(200))? {
            Some(InputEvent::Key(key)) => {
                if app.handle_key(key) == AppAction::Quit {
                    break;
                }
            }
            Some(InputEvent::Scroll(_)) | None => {}
        }

        app.tick();
    }

    Ok(())
}

fn run_interactive_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    start_in_resume: bool,
    context: InteractiveSessionContext,
) -> io::Result<()> {
    let workspace_path = std::env::current_dir()?;
    let mut app = App::new_interactive(
        workspace_path,
        context.session_id.clone(),
        context.active_profile.clone(),
        context.active_model.clone(),
        context.available_profiles.clone(),
        context.available_skills.clone(),
        start_in_resume,
    );
    app.set_gateway_target(match &context.gateway {
        InteractiveGateway::Local(_) => "local",
        InteractiveGateway::Remote(_) => "remote",
    });
    app.set_extension_state(
        context.extension_summary.clone(),
        context.extension_policy_summary.clone(),
        context.extension_errors.clone(),
    );
    app.set_gateway_state(None, None);
    app.set_node_state(None, None);
    app.git_branch = detect_git_branch();
    let input_source = build_input_source();
    let gateway_link = Arc::new(AtomicBool::new(true));
    let current_session_id = Arc::new(Mutex::new(context.session_id.clone()));
    refresh_interactive_session_from_gateway(
        &mut app,
        &context,
        current_session_id_value(&current_session_id).as_str(),
    );

    let event_forwarder = match context.gateway.clone() {
        InteractiveGateway::Local(gateway) => {
            context.runtime_handle.spawn(forward_gateway_runtime_events(
                gateway.subscribe(),
                context.event_buffer.clone(),
                gateway_link.clone(),
                Some(current_session_id.clone()),
            ))
        }
        InteractiveGateway::Remote(client) => {
            context.runtime_handle.spawn(forward_remote_runtime_events(
                client,
                context.event_buffer.clone(),
                gateway_link.clone(),
                Some(current_session_id.clone()),
            ))
        }
    };

    loop {
        drain_run_events(&mut app, &context.event_buffer);
        if gateway_link.load(Ordering::Relaxed) {
            let session_id = current_session_id_value(&current_session_id);
            refresh_interactive_session_from_gateway(&mut app, &context, &session_id);
        }

        // Sync transcript scroll to follow new content (when follow=true) before drawing.
        if let Ok(size) = terminal.size() {
            let total_lines = app.chat_total_lines(size.width);
            let visible_height = size.height.saturating_sub(4); // 4-row composer (border+context+input+hint)
            app.transcript.sync_follow(total_lines, visible_height);
        }

        terminal.draw(|frame| ui::render(frame, &app))?;

        match poll_input_event(&input_source, Duration::from_millis(200))? {
            Some(InputEvent::Key(key)) => match app.handle_key(key) {
                AppAction::Quit => break,
                AppAction::Continue => {}
                AppAction::SubmitRun(request) => {
                    if gateway_link.load(Ordering::Relaxed) {
                        let session_id = current_session_id_value(&current_session_id);
                        spawn_interactive_run(
                            &context,
                            session_id,
                            app.active_profile().to_owned(),
                            request,
                        );
                    } else {
                        app.push_command_error("Gateway is disconnected for this TUI session");
                    }
                }
                AppAction::GatewayConnect => {
                    gateway_link.store(true, Ordering::Relaxed);
                    let session_id = current_session_id_value(&current_session_id);
                    refresh_interactive_session_from_gateway(&mut app, &context, &session_id);
                }
                AppAction::GatewayDisconnect => {
                    gateway_link.store(false, Ordering::Relaxed);
                }
                AppAction::AdapterStatus => {
                    handle_tui_adapter_status(&mut app, &context);
                }
                AppAction::NodeList => {
                    handle_tui_node_list(
                        &mut app,
                        &context,
                        &current_session_id_value(&current_session_id),
                    );
                }
                AppAction::NodeShow(node_id) => {
                    handle_tui_node_show(
                        &mut app,
                        &context,
                        &current_session_id_value(&current_session_id),
                        &node_id,
                    );
                }
                AppAction::SandboxStatus => {
                    handle_tui_sandbox_status(&mut app, &context);
                }
                AppAction::SandboxInspect(env_id) => {
                    handle_tui_sandbox_inspect(&mut app, &context, &env_id);
                }
                AppAction::SandboxRebuild(env_id) => {
                    handle_tui_sandbox_rebuild(&mut app, &context, &env_id);
                }
                AppAction::SandboxClean => {
                    handle_tui_sandbox_clean(&mut app, &context);
                }
                AppAction::SwitchSession(session_id) => {
                    set_current_session_id(&current_session_id, &session_id);
                    if gateway_link.load(Ordering::Relaxed) {
                        refresh_interactive_session_from_gateway(&mut app, &context, &session_id);
                    }
                }
                AppAction::CancelRun(run_id) => {
                    handle_tui_cancel_run(&mut app, &context, &run_id);
                }
                AppAction::RetryRun(run_id) => {
                    handle_tui_retry_run(
                        &mut app,
                        &context,
                        &current_session_id_value(&current_session_id),
                        &run_id,
                    );
                }
                AppAction::InspectRun(run_id) => {
                    handle_tui_inspect_run(&mut app, &context, &run_id);
                }
                AppAction::ApproveCapability(call_id) => {
                    app.push_system_entry(
                        "approval",
                        format!("Approved capability call {call_id}"),
                    );
                }
                AppAction::DenyCapability(call_id) => {
                    app.push_system_entry("approval", format!("Denied capability call {call_id}"));
                }
            },
            Some(InputEvent::Scroll(delta)) => {
                if delta > 0 {
                    app.transcript.scroll_down(delta as u16);
                } else {
                    app.transcript.scroll_up((-delta) as u16);
                }
            }
            None => {}
        }

        app.tick();
    }

    event_forwarder.abort();

    Ok(())
}

fn build_input_source() -> InputSource {
    if io::stdin().is_terminal() || File::open("/dev/tty").is_ok() {
        InputSource::Crossterm
    } else {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut stdin = io::stdin().lock();
            let mut buffer = [0u8; 1];
            loop {
                match stdin.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Some(event) = decode_input_byte(buffer[0]) {
                            if sender.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        InputSource::Pipe(receiver)
    }
}

fn poll_input_event(
    input_source: &InputSource,
    timeout: Duration,
) -> io::Result<Option<InputEvent>> {
    match input_source {
        InputSource::Crossterm => {
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => Ok(Some(InputEvent::Key(key))),
                    Event::Paste(text) => Ok(text.chars().next().map(|character| {
                        InputEvent::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
                    })),
                    Event::Mouse(m) => Ok(match m.kind {
                        MouseEventKind::ScrollDown => Some(InputEvent::Scroll(3)),
                        MouseEventKind::ScrollUp => Some(InputEvent::Scroll(-3)),
                        _ => None,
                    }),
                    Event::Resize(_, _) => Ok(None),
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        }
        InputSource::Pipe(receiver) => match receiver.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        },
    }
}

fn decode_input_byte(byte: u8) -> Option<InputEvent> {
    let key = match byte {
        3 => KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        b'\r' | b'\n' => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        9 => KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        8 | 127 => KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        27 => KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        byte if byte.is_ascii_graphic() || byte == b' ' => {
            KeyEvent::new(KeyCode::Char(byte as char), KeyModifiers::NONE)
        }
        _ => return None,
    };
    Some(InputEvent::Key(key))
}

fn current_session_id_value(session_id: &Arc<Mutex<String>>) -> String {
    session_id
        .lock()
        .map(|value| value.clone())
        .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
}

fn set_current_session_id(session_id: &Arc<Mutex<String>>, value: &str) {
    match session_id.lock() {
        Ok(mut guard) => *guard = value.to_owned(),
        Err(poisoned) => *poisoned.into_inner() = value.to_owned(),
    }
}

fn spawn_interactive_run(
    context: &InteractiveSessionContext,
    session_id: String,
    profile: String,
    request: ComposerRunRequest,
) {
    let event_buffer = context.event_buffer.clone();
    let input = request.input.clone();
    let tool = request.tool.clone();
    let skill = request.skill.clone();
    let workflow = request.workflow.clone();

    match context.gateway.clone() {
        InteractiveGateway::Local(gateway) => {
            let request = GatewayRunRequest {
                system: context.system.clone(),
                input,
                tool,
                skill,
                workflow,
                session_id: Some(session_id.clone()),
                profile: Some(profile.clone()),
                ingress: Some(IngressTrace {
                    kind: "local_tui".to_owned(),
                    channel: Some("tui".to_owned()),
                    adapter: Some("tui_local".to_owned()),
                    bot_name: None,
                    bot_route: None,
                    bot_profile: None,
                    bot_token_env: None,
                    bot_secret_env: None,
                    source: Some("mosaic-tui".to_owned()),
                    remote_addr: None,
                    display_name: None,
                    actor_id: None,
                    conversation_id: None,
                    thread_id: None,
                    thread_title: None,
                    reply_target: None,
                    message_id: None,
                    received_at: None,
                    raw_event_id: None,
                    session_hint: Some(session_id.clone()),
                    profile_hint: Some(profile.clone()),
                    control_command: None,
                    original_text: None,
                    attachments: Vec::new(),
                    attachment_failures: Vec::new(),
                    gateway_url: None,
                }),
            };

            context.runtime_handle.spawn(async move {
                match gateway.submit_run(request) {
                    Ok(submitted) => {
                        if let Err(err) = submitted.wait().await {
                            event_buffer.push(RunEvent::RunFailed {
                                run_id: session_id.clone(),
                                error: err.to_string(),
                                failure_kind: Some("gateway".to_owned()),
                                failure_origin: Some("gateway".to_owned()),
                            });
                        }
                    }
                    Err(err) => {
                        event_buffer.push(RunEvent::RunFailed {
                            run_id: session_id.clone(),
                            error: err.to_string(),
                            failure_kind: Some("gateway".to_owned()),
                            failure_origin: Some("gateway".to_owned()),
                        });
                    }
                }
            });
        }
        InteractiveGateway::Remote(client) => {
            let gateway_url = client.base_url().to_owned();
            let request = GatewayRunRequest {
                system: context.system.clone(),
                input,
                tool,
                skill,
                workflow,
                session_id: Some(session_id.clone()),
                profile: Some(profile.clone()),
                ingress: Some(IngressTrace {
                    kind: "remote_operator".to_owned(),
                    channel: Some("tui".to_owned()),
                    adapter: Some("tui_remote".to_owned()),
                    bot_name: None,
                    bot_route: None,
                    bot_profile: None,
                    bot_token_env: None,
                    bot_secret_env: None,
                    source: Some("mosaic-tui".to_owned()),
                    remote_addr: None,
                    display_name: None,
                    actor_id: None,
                    conversation_id: None,
                    thread_id: None,
                    thread_title: None,
                    reply_target: None,
                    message_id: None,
                    received_at: None,
                    raw_event_id: None,
                    session_hint: Some(session_id.clone()),
                    profile_hint: Some(profile.clone()),
                    control_command: None,
                    original_text: None,
                    attachments: Vec::new(),
                    attachment_failures: Vec::new(),
                    gateway_url: Some(gateway_url),
                }),
            };

            context.runtime_handle.spawn(async move {
                if let Err(err) = client.submit_run(request).await {
                    event_buffer.push(RunEvent::RunFailed {
                        run_id: session_id.clone(),
                        error: err.to_string(),
                        failure_kind: Some("gateway".to_owned()),
                        failure_origin: Some("gateway".to_owned()),
                    });
                }
            });
        }
    }
}

fn handle_tui_cancel_run(app: &mut App, context: &InteractiveSessionContext, run_id: &str) {
    let result = match &context.gateway {
        InteractiveGateway::Local(gateway) => gateway.cancel_run(run_id),
        InteractiveGateway::Remote(client) => {
            context.runtime_handle.block_on(client.cancel_run(run_id))
        }
    };

    match result {
        Ok(detail) => {
            app.push_system_entry(
                "Run cancel accepted",
                format!(
                    "run={}\nstatus={:?}\nerror={}",
                    detail.summary.gateway_run_id,
                    detail.summary.status,
                    detail.summary.error.as_deref().unwrap_or("<none>")
                ),
            );
        }
        Err(err) => app.push_command_error(format!("Failed to cancel run {run_id}: {err}")),
    }
}

fn handle_tui_retry_run(
    app: &mut App,
    context: &InteractiveSessionContext,
    session_id: &str,
    run_id: &str,
) {
    let result = match &context.gateway {
        InteractiveGateway::Local(gateway) => gateway
            .retry_run(run_id)
            .map(|submitted| submitted.gateway_run_id().to_owned()),
        InteractiveGateway::Remote(client) => context
            .runtime_handle
            .block_on(client.retry_run(run_id))
            .map(|response| response.gateway_run_id),
    };

    match result {
        Ok(new_run_id) => {
            app.push_system_entry(
                "Run retry accepted",
                format!(
                    "previous_run={}\nnew_run={}\nsession={}",
                    run_id, new_run_id, session_id
                ),
            );
        }
        Err(err) => app.push_command_error(format!("Failed to retry run {run_id}: {err}")),
    }
}

fn handle_tui_inspect_run(app: &mut App, context: &InteractiveSessionContext, run_id: &str) {
    let result = match &context.gateway {
        InteractiveGateway::Local(gateway) => gateway.load_run(run_id),
        InteractiveGateway::Remote(client) => {
            context.runtime_handle.block_on(client.get_run(run_id))
        }
    };

    match result {
        Ok(Some(detail)) => app.show_run_detail(&detail),
        Ok(None) => app.push_command_error(format!("Run {run_id} was not found.")),
        Err(err) => app.push_command_error(format!("Failed to inspect run {run_id}: {err}")),
    }
}

fn handle_tui_adapter_status(app: &mut App, context: &InteractiveSessionContext) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => {
            let adapters = gateway.list_adapter_statuses();
            if adapters.is_empty() {
                app.push_system_entry("Adapter status", "No adapters are registered.");
                return;
            }

            let body = adapters
                .iter()
                .map(|adapter| {
                    let mut line = format!(
                        "{} | channel={} | transport={} | status={} | outbound_ready={} | path={}",
                        adapter.name,
                        adapter.channel,
                        adapter.transport,
                        adapter.status,
                        adapter.outbound_ready,
                        adapter.ingress_path,
                    );
                    if let Some(bot_name) = adapter.bot_name.as_deref() {
                        line.push_str(&format!(
                            " | bot={} route={} profile={}",
                            bot_name,
                            adapter.bot_route.as_deref().unwrap_or("<none>"),
                            adapter.bot_profile.as_deref().unwrap_or("<none>"),
                        ));
                    }
                    if !adapter.capabilities.is_empty() {
                        line.push_str(&format!(
                            " | capabilities={}",
                            adapter.capabilities.join(", ")
                        ));
                    }
                    line.push_str(&format!(" | detail={}", adapter.detail));
                    line
                })
                .collect::<Vec<_>>()
                .join("\n");
            app.push_system_entry("Adapter status", body);
        }
        InteractiveGateway::Remote(client) => {
            match context.runtime_handle.block_on(client.list_adapters()) {
                Ok(adapters) if adapters.is_empty() => {
                    app.push_system_entry("Adapter status", "No adapters are registered.");
                }
                Ok(adapters) => {
                    let body = adapters
                        .iter()
                        .map(|adapter| {
                            format!(
                                "{} | channel={} | transport={} | status={} | outbound_ready={} | path={} | detail={}",
                                adapter.name,
                                adapter.channel,
                                adapter.transport,
                                adapter.status,
                                adapter.outbound_ready,
                                adapter.ingress_path,
                                adapter.detail
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    app.push_system_entry("Adapter status", body);
                }
                Err(error) => {
                    app.push_command_error(format!("Adapter status failed: {error}"));
                }
            }
        }
    }
}

fn handle_tui_node_list(app: &mut App, context: &InteractiveSessionContext, session_id: &str) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => match (
            gateway.list_nodes(),
            gateway.list_node_affinities(),
            gateway.node_binding(Some(session_id)),
        ) {
            (Ok(nodes), Ok(affinities), Ok(binding)) => {
                if nodes.is_empty() {
                    app.push_system_entry("Node list", "No nodes are registered.");
                    return;
                }
                let body = nodes
                    .iter()
                    .map(|node| {
                        let health = node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS).label();
                        let affinity_scopes = affinities
                            .iter()
                            .filter(|record| record.node_id == node.node_id)
                            .map(|record| record.session_id.as_str())
                            .collect::<Vec<_>>();
                        let binding_marker = binding
                            .as_ref()
                            .filter(|record| record.node_id == node.node_id)
                            .map(|record| format!(" current_binding={}", record.affinity_scope))
                            .unwrap_or_default();
                        format!(
                            "{} | health={} | transport={} | platform={} | capabilities={} | affinity_scopes={}{} | disconnect_reason={}",
                            node.node_id,
                            health,
                            node.transport,
                            node.platform,
                            if node.capabilities.is_empty() {
                                "<none>".to_owned()
                            } else {
                                node.capabilities
                                    .iter()
                                    .map(|capability| capability.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            },
                            if affinity_scopes.is_empty() {
                                "<none>".to_owned()
                            } else {
                                affinity_scopes.join(", ")
                            },
                            binding_marker,
                            node.last_disconnect_reason
                                .as_deref()
                                .unwrap_or("<none>")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                app.push_system_entry("Node list", body);
            }
            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
                app.push_command_error(format!("Node list failed: {error}"));
            }
        },
        InteractiveGateway::Remote(_) => app.push_command_error(
            "Remote node list is not available from the TUI yet; use `mosaic node list` locally against the workspace.",
        ),
    }
}

fn handle_tui_node_show(
    app: &mut App,
    context: &InteractiveSessionContext,
    session_id: &str,
    node_id: &str,
) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => match (
            gateway.list_nodes(),
            gateway.node_binding(Some(session_id)),
            gateway.node_capabilities(node_id),
        ) {
            (Ok(nodes), Ok(binding), Ok(capabilities)) => {
                let Some(node) = nodes.iter().find(|node| node.node_id == node_id) else {
                    app.push_command_error(format!("Node {node_id} was not found."));
                    return;
                };
                let health = node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS).label();
                let bound = binding
                    .as_ref()
                    .filter(|record| record.node_id == node_id)
                    .map(|record| record.affinity_scope.clone())
                    .unwrap_or_else(|| "<none>".to_owned());
                let capabilities = if capabilities.is_empty() {
                    "<none>".to_owned()
                } else {
                    capabilities
                        .iter()
                        .map(|capability| {
                            format!(
                                "{} kind={} risk={} scopes={}",
                                capability.name,
                                capability.kind.label(),
                                capability.risk.label(),
                                if capability.permission_scopes.is_empty() {
                                    "<none>".to_owned()
                                } else {
                                    capability
                                        .permission_scopes
                                        .iter()
                                        .map(|scope| scope.label().to_owned())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                app.push_system_entry(
                    "Node inspect",
                    format!(
                        "node_id={}\nlabel={}\nhealth={}\ntransport={}\nplatform={}\nregistered_at={}\nlast_heartbeat_at={}\nbound_scope={}\nlast_disconnect_reason={}\ncapabilities=\n{}",
                        node.node_id,
                        node.label,
                        health,
                        node.transport,
                        node.platform,
                        node.registered_at,
                        node.last_heartbeat_at,
                        bound,
                        node.last_disconnect_reason.as_deref().unwrap_or("<none>"),
                        capabilities
                    ),
                );
            }
            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
                app.push_command_error(format!("Node inspect failed: {error}"));
            }
        },
        InteractiveGateway::Remote(_) => app.push_command_error(
            "Remote node inspect is not available from the TUI yet; use `mosaic node capabilities` or `mosaic node list` locally against the workspace.",
        ),
    }
}

fn handle_tui_sandbox_status(app: &mut App, context: &InteractiveSessionContext) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => match gateway.sandbox_status() {
            Ok(status) => {
                let runtime_lines = status
                    .runtime_statuses
                    .iter()
                    .map(|runtime| {
                        format!(
                            "{} | strategy={} | available={} | detail={}",
                            runtime.kind.label(),
                            runtime.strategy,
                            runtime.available,
                            runtime.detail.clone().unwrap_or_else(|| "<none>".to_owned())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                app.push_system_entry(
                    "Sandbox status",
                    format!(
                        "base_dir={}\npython={} install_enabled={}\nnode={} install_enabled={}\nenv_count={}\ncleanup(run_workdirs_after_hours={}, attachments_after_hours={})\nruntimes:\n{}",
                        status.base_dir,
                        status.python_strategy,
                        status.python_install_enabled,
                        status.node_strategy,
                        status.node_install_enabled,
                        status.env_count,
                        status.run_workdirs_after_hours,
                        status.attachments_after_hours,
                        runtime_lines,
                    ),
                );
            }
            Err(error) => app.push_command_error(format!("Sandbox status failed: {error}")),
        },
        InteractiveGateway::Remote(_) => app.push_command_error(
            "Remote sandbox status is not available from the TUI yet; use local CLI sandbox commands against the workspace.",
        ),
    }
}

fn handle_tui_sandbox_inspect(app: &mut App, context: &InteractiveSessionContext, env_id: &str) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => match gateway.sandbox_inspect_env(env_id) {
            Ok(record) => {
                let dependencies = if record.dependency_spec.is_empty() {
                    "<none>".to_owned()
                } else {
                    record.dependency_spec.join(", ")
                };
                let allowed_sources = if record.allowed_sources.is_empty() {
                    "<none>".to_owned()
                } else {
                    record
                        .allowed_sources
                        .iter()
                        .map(|source| source.label().to_owned())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                app.push_system_entry(
                    "Sandbox env",
                    format!(
                        "env_id={}\nkind={} scope={}\nstrategy={} status={} transition={}\nenv_dir={}\nruntime_dir={}\ndeps={}\ninstall(enabled={} timeout_ms={} retry_limit={} allowed_sources={})\nfailure_stage={}\nerror={}",
                        record.env_id,
                        record.kind.label(),
                        record.scope.label(),
                        record.strategy,
                        record.status.label(),
                        record.last_transition,
                        record.env_dir.display(),
                        record.runtime_dir.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "<none>".to_owned()),
                        dependencies,
                        record.install_enabled,
                        record.install_timeout_ms,
                        record.install_retry_limit,
                        allowed_sources,
                        record.failure_stage.unwrap_or_else(|| "<none>".to_owned()),
                        record.error.unwrap_or_else(|| "<none>".to_owned()),
                    ),
                );
            }
            Err(error) => app.push_command_error(format!("Sandbox inspect failed: {error}")),
        },
        InteractiveGateway::Remote(_) => app.push_command_error(
            "Remote sandbox inspect is not available from the TUI yet; use local CLI sandbox commands against the workspace.",
        ),
    }
}

fn handle_tui_sandbox_rebuild(app: &mut App, context: &InteractiveSessionContext, env_id: &str) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => match gateway.sandbox_rebuild_env(env_id) {
            Ok(record) => app.push_system_entry(
                "Sandbox env rebuilt",
                format!(
                    "env_id={}\nstatus={}\ntransition={}\nenv_dir={}",
                    record.env_id,
                    record.status.label(),
                    record.last_transition,
                    record.env_dir.display()
                ),
            ),
            Err(error) => app.push_command_error(format!("Sandbox rebuild failed: {error}")),
        },
        InteractiveGateway::Remote(_) => app.push_command_error(
            "Remote sandbox rebuild is not available from the TUI yet; use local CLI sandbox commands against the workspace.",
        ),
    }
}

fn handle_tui_sandbox_clean(app: &mut App, context: &InteractiveSessionContext) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => match gateway.sandbox_clean() {
            Ok(report) => app.push_system_entry(
                "Sandbox clean",
                format!(
                    "removed_run_workdirs={}\nremoved_attachment_workdirs={}",
                    report.removed_run_workdirs, report.removed_attachment_workdirs
                ),
            ),
            Err(error) => app.push_command_error(format!("Sandbox clean failed: {error}")),
        },
        InteractiveGateway::Remote(_) => app.push_command_error(
            "Remote sandbox clean is not available from the TUI yet; use local CLI sandbox commands against the workspace.",
        ),
    }
}

async fn forward_gateway_runtime_events(
    mut receiver: tokio::sync::broadcast::Receiver<mosaic_gateway::GatewayEventEnvelope>,
    event_buffer: TuiEventBuffer,
    gateway_link: Arc<AtomicBool>,
    session_filter: Option<Arc<Mutex<String>>>,
) {
    loop {
        let Ok(envelope) = receiver.recv().await else {
            break;
        };

        if !gateway_link.load(Ordering::Relaxed) {
            continue;
        }

        if let Some(session_id) = session_filter.as_ref().map(current_session_id_value) {
            if envelope.session_id.as_deref() != Some(session_id.as_str()) {
                continue;
            }
        }

        if let GatewayEvent::Runtime(event) = envelope.event {
            event_buffer.push(event);
        }
    }
}

fn refresh_interactive_session_from_gateway(
    app: &mut App,
    context: &InteractiveSessionContext,
    session_id: &str,
) {
    match &context.gateway {
        InteractiveGateway::Local(gateway) => {
            if let Ok(sessions) = gateway.list_sessions() {
                let summaries = sessions
                    .iter()
                    .map(local_session_summary_to_ui)
                    .collect::<Vec<_>>();
                app.sync_session_catalog(summaries, session_id);
            }
            if let Ok(Some(session)) = gateway.load_session(session_id) {
                app.sync_runtime_session_with_origin(&session, "Local");
            }
            refresh_local_gateway_state(app, gateway);
            refresh_local_node_state(app, gateway, session_id);
        }
        InteractiveGateway::Remote(client) => {
            if let Ok(sessions) = context.runtime_handle.block_on(client.list_sessions()) {
                let summaries = sessions
                    .iter()
                    .map(remote_session_summary_to_ui)
                    .collect::<Vec<_>>();
                app.sync_session_catalog(summaries, session_id);
            }
            if let Ok(Some(session)) = context
                .runtime_handle
                .block_on(client.get_session(session_id))
            {
                let stored = remote_session_to_stored(&session);
                app.sync_runtime_session_with_origin(&stored, "Remote");
            }
            refresh_remote_gateway_state(app, context, client);
            app.set_node_state(Some("Nodes remote status unavailable".to_owned()), None);
        }
    }
}

fn refresh_local_gateway_state(app: &mut App, gateway: &GatewayHandle) {
    let health = gateway.health();
    let readiness = gateway.readiness();
    apply_gateway_state(app, &health, &readiness);
}

fn refresh_remote_gateway_state(
    app: &mut App,
    context: &InteractiveSessionContext,
    client: &GatewayClient,
) {
    match (
        context.runtime_handle.block_on(client.health()),
        context.runtime_handle.block_on(client.readiness()),
    ) {
        (Ok(health), Ok(readiness)) => apply_gateway_state(app, &health, &readiness),
        (Err(err), _) | (_, Err(err)) => {
            app.gateway_connected = false;
            app.set_gateway_state(
                Some("Gateway status unavailable".to_owned()),
                Some(format!(
                    "Remote control-plane status request failed: {}",
                    err
                )),
            );
        }
    }
}

fn apply_gateway_state(app: &mut App, health: &HealthResponse, readiness: &ReadinessResponse) {
    app.gateway_connected = health.status == "ok" && health.transport != "offline";
    app.set_gateway_state(
        Some(format!(
            "Gateway {} transport={} auth={} deployment={} sessions={}",
            health.status,
            health.transport,
            health.auth_mode,
            health.deployment_profile,
            health.session_count,
        )),
        Some(format!(
            "Readiness {} audit={} replay={}/{} lag-threshold={}",
            readiness.status,
            readiness.audit_ready,
            readiness.replay_events_buffered,
            readiness.event_replay_window,
            readiness.slow_consumer_lag_threshold,
        )),
    );
}

fn refresh_local_node_state(app: &mut App, gateway: &GatewayHandle, session_id: &str) {
    let nodes = match gateway.list_nodes() {
        Ok(nodes) => nodes,
        Err(err) => {
            app.set_node_state(Some(format!("Nodes error: {}", err)), None);
            return;
        }
    };

    if nodes.is_empty() {
        app.set_node_state(Some("Nodes none registered".to_owned()), None);
        return;
    }

    let mut online = 0usize;
    let mut stale = 0usize;
    let mut offline = 0usize;
    for node in &nodes {
        match node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS) {
            mosaic_node_protocol::NodeHealth::Online => online += 1,
            mosaic_node_protocol::NodeHealth::Stale => stale += 1,
            mosaic_node_protocol::NodeHealth::Offline => offline += 1,
        }
    }

    let summary = format!(
        "Nodes online={} stale={} offline={}",
        online, stale, offline
    );
    let detail = match gateway.node_affinity(Some(session_id)) {
        Ok(Some(node_id)) => nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .map(|node| {
                format!(
                    "Session node: {} [{}] caps={}",
                    node.node_id,
                    node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS).label(),
                    node.capabilities
                        .iter()
                        .map(|cap| cap.name.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .or_else(|| Some(format!("Session node: {} [missing]", node_id))),
        Ok(None) => nodes.first().map(|node| {
            format!(
                "Default node candidate: {} [{}] caps={}",
                node.node_id,
                node.health(Utc::now(), DEFAULT_STALE_AFTER_SECS).label(),
                node.capabilities
                    .iter()
                    .map(|cap| cap.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }),
        Err(err) => Some(format!("Node affinity error: {}", err)),
    };

    app.set_node_state(Some(summary), detail);
}

async fn forward_remote_runtime_events(
    client: GatewayClient,
    event_buffer: TuiEventBuffer,
    gateway_link: Arc<AtomicBool>,
    session_filter: Option<Arc<Mutex<String>>>,
) {
    let mut stream = match client.subscribe_events().await {
        Ok(stream) => stream,
        Err(err) => {
            event_buffer.push(RunEvent::RunFailed {
                run_id: "gateway-stream".to_owned(),
                error: err.to_string(),
                failure_kind: Some("gateway".to_owned()),
                failure_origin: Some("gateway".to_owned()),
            });
            return;
        }
    };

    loop {
        let envelope = match stream.next_event().await {
            Ok(Some(envelope)) => envelope,
            Ok(None) => break,
            Err(err) => {
                event_buffer.push(RunEvent::RunFailed {
                    run_id: "gateway-stream".to_owned(),
                    error: err.to_string(),
                    failure_kind: Some("gateway".to_owned()),
                    failure_origin: Some("gateway".to_owned()),
                });
                break;
            }
        };

        if !gateway_link.load(Ordering::Relaxed) {
            continue;
        }

        if let Some(session_id) = session_filter.as_ref().map(current_session_id_value) {
            if envelope.session_id.as_deref() != Some(session_id.as_str()) {
                continue;
            }
        }

        if let GatewayEvent::Runtime(event) = envelope.event {
            event_buffer.push(event);
        }
    }
}

fn local_session_summary_to_ui(summary: &SessionSummary) -> UiSessionRecord {
    UiSessionRecord {
        id: summary.id.clone(),
        title: summary.title.clone(),
        origin: "Local".to_owned(),
        modified: summary.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        created: summary.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        channel: summary
            .channel_context
            .channel
            .clone()
            .unwrap_or_else(|| "control".to_owned()),
        actor: summary
            .channel_context
            .actor_name
            .clone()
            .or(summary.channel_context.actor_id.clone()),
        thread: summary
            .channel_context
            .thread_title
            .clone()
            .or(summary.channel_context.thread_id.clone()),
        route: summary.session_route.clone(),
        runtime: summary.provider_type.clone(),
        model: summary.model.clone(),
        state: session_state_from_run_label(summary.run.status.label()),
        unread: 0,
        draft: String::new(),
                cursor_pos: 0,
        transcript_len: 0,
        current_run_id: summary.run.current_run_id.clone(),
        current_gateway_run_id: summary.run.current_gateway_run_id.clone(),
        last_gateway_run_id: summary.last_gateway_run_id.clone(),
        memory_summary: summary.memory_summary_preview.clone(),
        compressed_context: None,
        references: if summary.reference_count == 0 {
            Vec::new()
        } else {
            vec![format!("{} session references", summary.reference_count)]
        },
        streaming_preview: None,
        streaming_run_id: None,
        active_turn: None,
        timeline: Vec::new(),
    }
}

fn remote_session_summary_to_ui(summary: &SessionSummaryDto) -> UiSessionRecord {
    UiSessionRecord {
        id: summary.id.clone(),
        title: summary.title.clone(),
        origin: "Remote".to_owned(),
        modified: summary.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        created: summary.updated_at.format("%Y-%m-%d %H:%M").to_string(),
        channel: summary
            .channel_context
            .channel
            .clone()
            .unwrap_or_else(|| "control".to_owned()),
        actor: summary
            .channel_context
            .actor_name
            .clone()
            .or(summary.channel_context.actor_id.clone()),
        thread: summary
            .channel_context
            .thread_title
            .clone()
            .or(summary.channel_context.thread_id.clone()),
        route: summary.session_route.clone(),
        runtime: summary.provider_type.clone(),
        model: summary.model.clone(),
        state: session_state_from_run_label(summary.run.status.label()),
        unread: 0,
        draft: String::new(),
                cursor_pos: 0,
        transcript_len: 0,
        current_run_id: summary.run.current_run_id.clone(),
        current_gateway_run_id: summary.run.current_gateway_run_id.clone(),
        last_gateway_run_id: summary.last_gateway_run_id.clone(),
        memory_summary: summary.memory_summary_preview.clone(),
        compressed_context: None,
        references: if summary.reference_count == 0 {
            Vec::new()
        } else {
            vec![format!("{} session references", summary.reference_count)]
        },
        streaming_preview: None,
        streaming_run_id: None,
        active_turn: None,
        timeline: Vec::new(),
    }
}

fn remote_session_to_stored(session: &SessionDetailDto) -> StoredSessionRecord {
    StoredSessionRecord {
        id: session.id.clone(),
        title: session.title.clone(),
        created_at: session.created_at,
        updated_at: session.updated_at,
        provider_profile: session.provider_profile.clone(),
        provider_type: session.provider_type.clone(),
        model: session.model.clone(),
        last_run_id: session.last_run_id.clone(),
        gateway: SessionGatewayMetadata {
            route: session.gateway.route.clone(),
            last_gateway_run_id: session.gateway.last_gateway_run_id.clone(),
            last_correlation_id: session.gateway.last_correlation_id.clone(),
        },
        run: mosaic_session_core::SessionRunMetadata {
            current_run_id: session.run.current_run_id.clone(),
            current_gateway_run_id: session.run.current_gateway_run_id.clone(),
            current_correlation_id: session.run.current_correlation_id.clone(),
            status: session.run.status.clone(),
            last_error: session.run.last_error.clone(),
            last_failure_kind: session.run.last_failure_kind.clone(),
            updated_at: session.run.updated_at,
        },
        channel_context: mosaic_session_core::SessionChannelMetadata {
            ingress_kind: session.channel_context.ingress_kind.clone(),
            channel: session.channel_context.channel.clone(),
            adapter: session.channel_context.adapter.clone(),
            bot_name: session.channel_context.bot_name.clone(),
            bot_route: session.channel_context.bot_route.clone(),
            bot_profile: session.channel_context.bot_profile.clone(),
            bot_token_env: session.channel_context.bot_token_env.clone(),
            source: session.channel_context.source.clone(),
            actor_id: session.channel_context.actor_id.clone(),
            actor_name: session.channel_context.actor_name.clone(),
            conversation_id: session.channel_context.conversation_id.clone(),
            thread_id: session.channel_context.thread_id.clone(),
            thread_title: session.channel_context.thread_title.clone(),
            reply_target: session.channel_context.reply_target.clone(),
            last_message_id: session.channel_context.last_message_id.clone(),
            last_delivery_id: session.channel_context.last_delivery_id.clone(),
            last_delivery_status: session.channel_context.last_delivery_status.clone(),
            last_delivery_error: session.channel_context.last_delivery_error.clone(),
            last_delivery_at: session.channel_context.last_delivery_at,
        },
        memory: mosaic_session_core::SessionMemoryMetadata {
            latest_summary: session.memory_summary.clone(),
            compressed_context: session.compressed_context.clone(),
            last_memory_write_at: None,
            memory_entry_count: 0,
            compression_count: usize::from(session.compressed_context.is_some()),
        },
        references: session
            .references
            .iter()
            .map(|reference| mosaic_session_core::SessionReference {
                session_id: reference.session_id.clone(),
                reason: reference.reason.clone(),
                created_at: reference.created_at,
            })
            .collect(),
        transcript: session
            .transcript
            .iter()
            .map(|message| TranscriptMessage {
                role: match message.role {
                    TranscriptRoleDto::System => TranscriptRole::System,
                    TranscriptRoleDto::User => TranscriptRole::User,
                    TranscriptRoleDto::Assistant => TranscriptRole::Assistant,
                    TranscriptRoleDto::Tool => TranscriptRole::Tool,
                },
                content: message.content.clone(),
                tool_call_id: message.tool_call_id.clone(),
                created_at: message.created_at,
            })
            .collect(),
    }
}

#[cfg(test)]
fn refresh_interactive_session(app: &mut App, session_store: &dyn SessionStore, session_id: &str) {
    if let Ok(Some(session)) = session_store.load(session_id) {
        app.sync_runtime_session(&session);
    }
}

fn drain_run_events(app: &mut App, event_buffer: &TuiEventBuffer) -> bool {
    let mut saw_terminal_event = false;

    for event in event_buffer.drain() {
        if matches!(
            event,
            RunEvent::RunFinished { .. }
                | RunEvent::RunFailed { .. }
                | RunEvent::RunCanceled { .. }
        ) {
            saw_terminal_event = true;
        }

        app.apply_run_event(event);
    }

    saw_terminal_event
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use mosaic_control_protocol::{
        HealthResponse, ReadinessResponse, SessionDetailDto, SessionGatewayDto, SessionRunDto,
        TranscriptMessageDto, TranscriptRoleDto,
    };
    use mosaic_runtime::events::{RunEvent, RunEventSink};
    use mosaic_session_core::{SessionRecord, SessionStore, SessionSummary, TranscriptRole};

    use crate::app::App;

    use super::{
        TuiEventBuffer, TuiEventSink, apply_gateway_state, build_app, drain_run_events,
        refresh_interactive_session, remote_session_to_stored,
    };

    struct MemorySessionStore {
        session: Option<SessionRecord>,
    }

    impl SessionStore for MemorySessionStore {
        fn load(&self, _id: &str) -> anyhow::Result<Option<SessionRecord>> {
            Ok(self.session.clone())
        }

        fn save(&self, _session: &SessionRecord) -> anyhow::Result<()> {
            Ok(())
        }

        fn list(&self) -> anyhow::Result<Vec<SessionSummary>> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn build_app_keeps_chat_first_surface_even_with_resume_flag() {
        let app = build_app("/tmp/mosaic".into(), true);

        assert_eq!(
            app.active_session()
                .timeline
                .last()
                .map(|entry| entry.title.as_str()),
            Some("Chat-first TUI")
        );
    }

    #[test]
    fn tui_event_sink_buffers_and_drains_events() {
        let buffer = TuiEventBuffer::default();
        let sink = TuiEventSink::new(buffer.clone());

        sink.emit(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });

        assert_eq!(
            buffer.drain(),
            vec![RunEvent::RunStarted {
                run_id: "run-1".to_owned(),
                input: "hello".to_owned(),
            }]
        );
        assert!(buffer.drain().is_empty());
    }

    #[test]
    fn drain_run_events_marks_terminal_run_events_for_auto_exit() {
        let mut app = build_app("/tmp/mosaic".into(), false);
        let buffer = TuiEventBuffer::default();

        buffer.push(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });
        buffer.push(RunEvent::RunFinished {
            run_id: "run-1".to_owned(),
            output_preview: "done".to_owned(),
        });

        assert!(drain_run_events(&mut app, &buffer));
        assert_eq!(app.runtime_status, "idle");
    }

    #[test]
    fn refresh_interactive_session_appends_transcript_into_chat_timeline() {
        let store = MemorySessionStore {
            session: Some({
                let mut session = SessionRecord::new("demo", "Demo", "mock", "mock", "mock");
                session.append_message(TranscriptRole::User, "hello", None);
                session.append_message(TranscriptRole::Assistant, "world", None);
                session
            }),
        };
        let mut app = crate::app::App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "mock".to_owned(),
            "mock".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );

        refresh_interactive_session(&mut app, &store, "demo");

        assert_eq!(app.active_session().timeline.len(), 2);
        assert_eq!(app.active_session().title, "Demo");
    }

    #[test]
    fn gateway_status_treats_http_sse_transport_as_connected() {
        let mut app = build_app("/tmp/mosaic".into(), false);
        let health = HealthResponse {
            status: "ok".to_owned(),
            active_profile: "mock".to_owned(),
            session_count: 1,
            transport: "http+sse".to_owned(),
            deployment_profile: "local".to_owned(),
            auth_mode: "disabled".to_owned(),
            event_replay_window: 256,
            capability_inventory: mosaic_control_protocol::CapabilityInventorySummaryDto::default(),
            reload_boundaries: mosaic_control_protocol::ReloadBoundaryDto::default(),
        };
        let readiness = ReadinessResponse {
            status: "ready".to_owned(),
            transport: "http+sse".to_owned(),
            deployment_profile: "local".to_owned(),
            auth_mode: "disabled".to_owned(),
            session_store_ready: true,
            audit_ready: true,
            extension_count: 0,
            session_count: 1,
            replay_events_buffered: 0,
            event_replay_window: 256,
            slow_consumer_lag_threshold: 32,
        };

        apply_gateway_state(&mut app, &health, &readiness);

        assert!(app.gateway_connected);
    }

    #[test]
    fn remote_session_refresh_updates_ui_origin_and_timeline() {
        let now = Utc::now();
        let session = SessionDetailDto {
            id: "remote-demo".to_owned(),
            title: "Remote Demo".to_owned(),
            created_at: now,
            updated_at: now,
            provider_profile: "mock".to_owned(),
            provider_type: "mock".to_owned(),
            model: "mock".to_owned(),
            last_run_id: Some("run-1".to_owned()),
            run: SessionRunDto {
                current_run_id: Some("run-1".to_owned()),
                current_gateway_run_id: Some("gateway-run-1".to_owned()),
                current_correlation_id: Some("corr-1".to_owned()),
                status: Default::default(),
                last_error: None,
                last_failure_kind: None,
                updated_at: Some(now),
            },
            channel_context: mosaic_control_protocol::SessionChannelDto {
                ingress_kind: Some("telegram_webhook".to_owned()),
                channel: Some("telegram".to_owned()),
                adapter: Some("telegram_webhook".to_owned()),
                bot_name: None,
                bot_route: None,
                bot_profile: None,
                bot_token_env: None,
                source: Some("telegram-webhook".to_owned()),
                actor_id: Some("ios-primary".to_owned()),
                actor_name: Some("iOS Primary".to_owned()),
                conversation_id: Some("telegram:chat:42".to_owned()),
                thread_id: Some("ops-7".to_owned()),
                thread_title: Some("Ops Thread".to_owned()),
                reply_target: Some("telegram:chat:42:thread:ops-7".to_owned()),
                last_message_id: Some("11".to_owned()),
                last_delivery_id: Some("delivery-1".to_owned()),
                last_delivery_status: Some("delivered".to_owned()),
                last_delivery_error: None,
                last_delivery_at: Some(now),
            },
            gateway: SessionGatewayDto {
                route: "gateway.remote/remote-demo".to_owned(),
                last_gateway_run_id: Some("gateway-run-1".to_owned()),
                last_correlation_id: Some("corr-1".to_owned()),
            },
            node_binding: None,
            memory_summary: Some("Remote summary".to_owned()),
            compressed_context: Some("Remote compressed context".to_owned()),
            references: vec![mosaic_control_protocol::SessionReferenceDto {
                session_id: "other-session".to_owned(),
                reason: "explicit_session_reference".to_owned(),
                created_at: now,
            }],
            transcript: vec![
                TranscriptMessageDto {
                    role: TranscriptRoleDto::User,
                    content: "hello remote".to_owned(),
                    tool_call_id: None,
                    created_at: now,
                },
                TranscriptMessageDto {
                    role: TranscriptRoleDto::Assistant,
                    content: "remote reply".to_owned(),
                    tool_call_id: None,
                    created_at: now,
                },
            ],
        };
        let stored = remote_session_to_stored(&session);
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "remote-demo".to_owned(),
            "mock".to_owned(),
            "mock".to_owned(),
            Vec::new(),
            Vec::new(),
            true,
        );

        app.sync_runtime_session_with_origin(&stored, "Remote");

        assert_eq!(app.active_session().origin, "Remote");
        assert_eq!(app.active_session().route, "gateway.remote/remote-demo");
        assert_eq!(app.active_session().timeline.len(), 2);
        assert_eq!(app.active_session().title, "Remote Demo");
        assert_eq!(
            app.active_session().memory_summary.as_deref(),
            Some("Remote summary")
        );
        assert_eq!(
            app.active_session().compressed_context.as_deref(),
            Some("Remote compressed context")
        );
        assert_eq!(app.active_session().references.len(), 1);
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn detect_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}
