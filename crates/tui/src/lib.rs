//! Mosaic terminal UI crate.
//! This crate owns terminal rendering, keyboard interaction, local operator
//! view-state, and the first event loop boundary for the control-plane console.

pub mod app;
pub mod mock;
pub mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{io, time::Duration};

use chrono::Utc;
use crossterm::{
    event::{self, Event, KeyEventKind},
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

use self::app::{App, AppAction, ProfileOption, SessionRecord as UiSessionRecord, SessionState};

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

    loop {
        let saw_terminal_event = drain_run_events(&mut app, &event_buffer);

        terminal.draw(|frame| ui::render(frame, &app))?;

        if exit_on_run_completion && saw_terminal_event {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if app.handle_key(key) == AppAction::Quit {
                        break;
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
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
        start_in_resume,
    );
    app.set_extension_state(
        context.extension_summary.clone(),
        context.extension_policy_summary.clone(),
        context.extension_errors.clone(),
    );
    app.set_gateway_state(None, None);
    app.set_node_state(None, None);
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

        terminal.draw(|frame| ui::render(frame, &app))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key) {
                    AppAction::Quit => break,
                    AppAction::Continue => {}
                    AppAction::Submit(input) => {
                        if gateway_link.load(Ordering::Relaxed) {
                            let session_id = current_session_id_value(&current_session_id);
                            spawn_interactive_run(
                                &context,
                                session_id,
                                app.active_profile().to_owned(),
                                input,
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
                    AppAction::SwitchSession(session_id) => {
                        set_current_session_id(&current_session_id, &session_id);
                        if gateway_link.load(Ordering::Relaxed) {
                            refresh_interactive_session_from_gateway(
                                &mut app,
                                &context,
                                &session_id,
                            );
                        }
                    }
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        app.tick();
    }

    event_forwarder.abort();

    Ok(())
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
    input: String,
) {
    let event_buffer = context.event_buffer.clone();

    match context.gateway.clone() {
        InteractiveGateway::Local(gateway) => {
            let request = GatewayRunRequest {
                system: context.system.clone(),
                input,
                tool: None,
                skill: None,
                workflow: None,
                session_id: Some(session_id.clone()),
                profile: Some(profile.clone()),
                ingress: Some(IngressTrace {
                    kind: "local_tui".to_owned(),
                    channel: Some("tui".to_owned()),
                    adapter: Some("tui_local".to_owned()),
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
                            });
                        }
                    }
                    Err(err) => {
                        event_buffer.push(RunEvent::RunFailed {
                            run_id: session_id.clone(),
                            error: err.to_string(),
                            failure_kind: Some("gateway".to_owned()),
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
                tool: None,
                skill: None,
                workflow: None,
                session_id: Some(session_id.clone()),
                profile: Some(profile.clone()),
                ingress: Some(IngressTrace {
                    kind: "remote_operator".to_owned(),
                    channel: Some("tui".to_owned()),
                    adapter: Some("tui_remote".to_owned()),
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
                    gateway_url: Some(gateway_url),
                }),
            };

            context.runtime_handle.spawn(async move {
                if let Err(err) = client.submit_run(request).await {
                    event_buffer.push(RunEvent::RunFailed {
                        run_id: session_id.clone(),
                        error: err.to_string(),
                        failure_kind: Some("gateway".to_owned()),
                    });
                }
            });
        }
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
        memory_summary: summary.memory_summary_preview.clone(),
        compressed_context: None,
        references: if summary.reference_count == 0 {
            Vec::new()
        } else {
            vec![format!("{} session references", summary.reference_count)]
        },
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
        memory_summary: summary.memory_summary_preview.clone(),
        compressed_context: None,
        references: if summary.reference_count == 0 {
            Vec::new()
        } else {
            vec![format!("{} session references", summary.reference_count)]
        },
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

    use crate::app::{App, Surface};

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
    fn build_app_uses_explicit_resume_flag() {
        let app = build_app("/tmp/mosaic".into(), true);

        assert!(matches!(app.surface, Surface::Resume));
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
    fn refresh_interactive_session_rebuilds_timeline_from_transcript() {
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
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
