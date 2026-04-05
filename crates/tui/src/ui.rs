use ratatui::Frame;

use crate::{app::App, shell_view::ShellView};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    ShellView::new(app.shell_snapshot(), frame.area()).render(frame);
}

#[cfg(test)]
mod tests {
    use mosaic_runtime::events::RunEvent;
    use ratatui::{Terminal, backend::TestBackend};

    use super::render;
    use crate::app::App;

    fn render_to_text(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal should initialize");
        terminal
            .draw(|frame| render(frame, app))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = buffer.cell((x, y)).expect("cell should exist");
                text.push_str(cell.symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn chat_shell_renders_single_column_header_transcript_and_footer() {
        let app = App::new("/tmp/mosaic".into());
        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Mosaic"));
        assert!(screen.contains("idle"));
        assert!(screen.contains("ready"));
        assert!(screen.contains("sess-gateway-001"));
        assert!(screen.contains("/ commands"));
        assert!(screen.contains("Esc"));
    }

    #[test]
    fn slash_input_renders_command_popup() {
        let mut app = App::new("/tmp/mosaic".into());
        app.active_session_mut().draft = "/".to_owned();

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Commands"));
        assert!(screen.contains("/help"));
        assert!(screen.contains("/new"));
    }

    #[test]
    fn transcript_renders_streaming_preview_inline() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.active_session_mut().streaming_preview = Some("partial assistant reply".to_owned());

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("assistant"));
        assert!(screen.contains("partial assistant reply"));
    }

    #[test]
    fn status_row_hides_during_streaming_turn() {
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        app.apply_run_event(RunEvent::RunStarted {
            run_id: "run-1".to_owned(),
            input: "hello".to_owned(),
        });
        app.apply_run_event(RunEvent::OutputDelta {
            run_id: "run-1".to_owned(),
            chunk: "partial assistant reply".to_owned(),
            accumulated_chars: 23,
        });

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("streaming"));
        assert!(screen.contains("partial assistant reply"));
        assert!(!screen.contains("status streaming"));
    }

    #[test]
    fn execution_cards_render_as_collapsed_exec_blocks() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::ToolCalling {
            name: "read_file".to_owned(),
            call_id: "call-1".to_owned(),
            summary: Some(
                "route=capability_match\nsource=builtin_tool\nexecution_target=local\norchestration_owner=tool_loop"
                    .to_owned(),
            ),
        });

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("assistant"));
        assert!(screen.contains("capability-active"));
        assert!(screen.contains("[tool]Tool call: read_file"));
        assert!(screen.contains("route=capability_match"));
        assert!(screen.contains("Ctrl+O detail"));
    }

    #[test]
    fn failure_cards_render_next_action_guidance_inline() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::RunFailed {
            run_id: "run-1".to_owned(),
            error: "mcp server socket closed".to_owned(),
            failure_kind: Some("transport".to_owned()),
            failure_origin: Some("mcp".to_owned()),
        });

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("assistant"));
        assert!(screen.contains("failed"));
        assert!(screen.contains("[failure]Run failed"));
        assert!(screen.contains("Ctrl+O detail"));
    }

    #[test]
    fn ctrl_o_opens_turn_detail_overlay() {
        let mut app = App::new("/tmp/mosaic".into());
        app.apply_run_event(RunEvent::WorkflowStepStarted {
            workflow: "ops_review".to_owned(),
            step: "fanout".to_owned(),
            kind: "tool".to_owned(),
            summary: Some(
                "target=read_file\norchestration_owner=workflow_engine\nstep_timeout=30s"
                    .to_owned(),
            ),
        });
        let _ = app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('o'),
            crossterm::event::KeyModifiers::CONTROL,
        ));

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Turn detail"));
        assert!(screen.contains("ops_review"));
        assert!(screen.contains("Workflow step: fanout"));
        assert!(screen.contains("step_timeout=30s"));
        assert!(screen.contains("Esc closes"));
    }

    #[test]
    fn busy_composer_renders_send_disabled_state() {
        let mut app = App::new("/tmp/mosaic".into());
        app.runtime_status = "running".to_owned();
        app.active_session_mut().current_gateway_run_id = Some("gw-run-1".to_owned());

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("busy"));
        assert!(screen.contains("send disabled · /run stop (gw-run-1)"));
        assert!(screen.contains("/ commands"));
    }

    #[test]
    fn ctrl_t_opens_transcript_overlay() {
        let mut app = App::new("/tmp/mosaic".into());
        let _ = app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('t'),
            crossterm::event::KeyModifiers::CONTROL,
        ));

        let screen = render_to_text(&app, 120, 28);

        assert!(screen.contains("Transcript"));
        assert!(screen.contains("Ctrl+T toggles transcript view"));
    }

    #[test]
    fn typing_hello_appears_in_rendered_screen() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new("/tmp/mosaic".into());
        for ch in "hello".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert_eq!(app.active_session().draft, "hello");
        let screen = render_to_text(&app, 120, 28);
        assert!(
            screen.contains("hello"),
            "typed 'hello' must appear in rendered screen; got:\n{}",
            screen
        );
    }

    #[test]
    fn submitted_message_appears_in_chat() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut app = App::new_interactive(
            "/tmp/mosaic".into(),
            "demo".to_owned(),
            "openai".to_owned(),
            "gpt-5.4-mini".to_owned(),
            Vec::new(),
            Vec::new(),
            false,
        );
        for ch in "hello world".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Draft is cleared after submit
        assert!(app.active_session().draft.is_empty(), "draft must clear after submit");
        // Message must appear in the transcript timeline (body contains the actual text)
        let found_in_timeline = app.visible_timeline().iter()
            .any(|e| e.body.contains("hello world") || e.title.contains("hello world"));
        assert!(
            found_in_timeline,
            "submitted message must appear in timeline; entries = {:?}",
            app.visible_timeline().iter().map(|e| (e.title.clone(), e.body.clone())).collect::<Vec<_>>()
        );
        // And must render on screen
        let screen = render_to_text(&app, 120, 28);
        assert!(
            screen.contains("hello world"),
            "submitted message must appear on screen; got:\n{}", screen
        );
    }
}
