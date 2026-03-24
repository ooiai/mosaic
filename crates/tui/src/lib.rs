//! Mosaic terminal UI crate.
//! This crate owns terminal rendering, keyboard interaction, local operator
//! view-state, and the first event loop boundary for the control-plane console.

pub mod app;
pub mod mock;
pub mod ui;

use std::sync::{Arc, Mutex};
use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use mosaic_runtime::events::{RunEvent, RunEventSink};
use ratatui::{Terminal, backend::CrosstermBackend};

use self::app::{App, AppAction};

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

    use crate::app::Surface;

    use super::{TuiEventBuffer, TuiEventSink, build_app, drain_run_events};

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
