//! Mosaic terminal UI crate.
//! This crate owns terminal rendering, keyboard interaction, local operator
//! view-state, and the first event loop boundary for the control-plane console.

pub mod app;
pub mod mock;
pub mod ui;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mosaic_gateway::{GatewayEvent, GatewayHandle, GatewayRunRequest};
use mosaic_runtime::events::{RunEvent, RunEventSink};
#[cfg(test)]
use mosaic_session_core::SessionStore;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::runtime::Handle;

use self::app::{App, AppAction, ProfileOption};

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
    pub gateway: GatewayHandle,
    pub runtime_handle: Handle,
    pub event_buffer: TuiEventBuffer,
    pub session_id: String,
    pub system: Option<String>,
    pub active_profile: String,
    pub active_model: String,
    pub available_profiles: Vec<ProfileOption>,
}

pub fn build_tui_event_buffer() -> TuiEventBuffer {
    TuiEventBuffer::default()
}

pub fn build_tui_event_sink(buffer: TuiEventBuffer) -> Arc<dyn RunEventSink> {
    Arc::new(TuiEventSink::new(buffer))
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
    let gateway_link = Arc::new(AtomicBool::new(true));
    refresh_interactive_session_from_gateway(&mut app, &context.gateway, &context.session_id);

    let event_forwarder = context.runtime_handle.spawn(forward_gateway_runtime_events(
        context.gateway.subscribe(),
        context.event_buffer.clone(),
        gateway_link.clone(),
        Some(context.session_id.clone()),
    ));

    loop {
        drain_run_events(&mut app, &context.event_buffer);
        if gateway_link.load(Ordering::Relaxed) {
            refresh_interactive_session_from_gateway(&mut app, &context.gateway, &context.session_id);
        }

        terminal.draw(|frame| ui::render(frame, &app))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key) {
                    AppAction::Quit => break,
                    AppAction::Continue => {}
                    AppAction::Submit(input) => {
                        if gateway_link.load(Ordering::Relaxed) {
                            spawn_interactive_run(&context, app.active_profile().to_owned(), input);
                        } else {
                            app.push_command_error("Gateway is disconnected for this TUI session");
                        }
                    }
                    AppAction::GatewayConnect => {
                        gateway_link.store(true, Ordering::Relaxed);
                        refresh_interactive_session_from_gateway(
                            &mut app,
                            &context.gateway,
                            &context.session_id,
                        );
                    }
                    AppAction::GatewayDisconnect => {
                        gateway_link.store(false, Ordering::Relaxed);
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

fn spawn_interactive_run(
    context: &InteractiveSessionContext,
    profile: String,
    input: String,
) {
    let gateway = context.gateway.clone();
    let request = GatewayRunRequest {
        system: context.system.clone(),
        input,
        skill: None,
        session_id: Some(context.session_id.clone()),
        profile: Some(profile),
    };
    let event_buffer = context.event_buffer.clone();

    context.runtime_handle.spawn(async move {
        match gateway.submit_run(request) {
            Ok(submitted) => {
                if let Err(err) = submitted.wait().await {
                    event_buffer.push(RunEvent::RunFailed {
                        error: err.to_string(),
                    });
                }
            }
            Err(err) => {
                event_buffer.push(RunEvent::RunFailed {
                    error: err.to_string(),
                });
            }
        }
    });
}

async fn forward_gateway_runtime_events(
    mut receiver: tokio::sync::broadcast::Receiver<mosaic_gateway::GatewayEventEnvelope>,
    event_buffer: TuiEventBuffer,
    gateway_link: Arc<AtomicBool>,
    session_filter: Option<String>,
) {
    loop {
        let Ok(envelope) = receiver.recv().await else {
            break;
        };

        if !gateway_link.load(Ordering::Relaxed) {
            continue;
        }

        if let Some(session_id) = session_filter.as_deref() {
            if envelope.session_id.as_deref() != Some(session_id) {
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
    gateway: &GatewayHandle,
    session_id: &str,
) {
    if let Ok(Some(session)) = gateway.load_session(session_id) {
        app.sync_runtime_session(&session);
    }
}

#[cfg(test)]
fn refresh_interactive_session(
    app: &mut App,
    session_store: &dyn SessionStore,
    session_id: &str,
) {
    if let Ok(Some(session)) = session_store.load(session_id) {
        app.sync_runtime_session(&session);
    }
}

fn drain_run_events(app: &mut App, event_buffer: &TuiEventBuffer) -> bool {
    let mut saw_terminal_event = false;

    for event in event_buffer.drain() {
        if matches!(
            event,
            RunEvent::RunFinished { .. } | RunEvent::RunFailed { .. }
        ) {
            saw_terminal_event = true;
        }

        app.apply_run_event(event);
    }

    saw_terminal_event
}

#[cfg(test)]
mod tests {
    use mosaic_runtime::events::{RunEvent, RunEventSink};
    use mosaic_session_core::{SessionRecord, SessionStore, SessionSummary, TranscriptRole};

    use crate::app::Surface;

    use super::{TuiEventBuffer, TuiEventSink, build_app, drain_run_events, refresh_interactive_session};

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
            input: "hello".to_owned(),
        });

        assert_eq!(
            buffer.drain(),
            vec![RunEvent::RunStarted {
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
            input: "hello".to_owned(),
        });
        buffer.push(RunEvent::RunFinished {
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
