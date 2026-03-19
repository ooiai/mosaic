//! Mosaic terminal UI crate.
//! This crate owns terminal rendering, keyboard interaction, local operator
//! view-state, and the first event loop boundary for the control-plane console.

mod app;
mod mock;
mod ui;

use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use self::app::{App, AppAction};

pub fn run() -> io::Result<()> {
    let start_in_resume = std::env::args().skip(1).any(|arg| arg == "--resume");
    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, start_in_resume);
    restore_terminal(&mut terminal)?;
    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    start_in_resume: bool,
) -> io::Result<()> {
    let workspace_path = std::env::current_dir()?;
    let mut app = if start_in_resume {
        App::new_with_resume(workspace_path, true)
    } else {
        App::new(workspace_path)
    };

    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

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

#[cfg(test)]
mod tests {
    #[test]
    fn detects_resume_flag_from_cli_args() {
        let args = ["mosaic", "--resume"];

        assert!(args.into_iter().skip(1).any(|arg| arg == "--resume"));
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
