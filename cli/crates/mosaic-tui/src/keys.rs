use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mosaic_core::error::Result;
use mosaic_core::session::SessionStore;
use tokio::sync::mpsc;

use crate::commands::{command_palette_items, parse_input_command, selected_command_palette_item};
use crate::events::AppEvent;
use crate::pickers::{
    handle_input_command, load_selected_session, switch_active_agent, toggle_agent_picker,
    toggle_session_picker,
};
use crate::state::{TuiAction, TuiState};
use crate::{TuiFocus, TuiOptions, TuiRuntime};

pub(crate) async fn handle_key(
    state: &mut TuiState,
    session_store: &SessionStore,
    key: KeyEvent,
    runtime: &mut TuiRuntime,
    options: &TuiOptions,
    app_tx: &mpsc::UnboundedSender<AppEvent>,
) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Ok(true);
    }

    if state.show_help {
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
            state.reduce(TuiAction::ToggleHelp);
        }
        return Ok(false);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('a') {
        toggle_agent_picker(state, runtime)?;
        return Ok(false);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
        toggle_session_picker(state, session_store)?;
        return Ok(false);
    }

    if state.show_agent_picker {
        match key.code {
            KeyCode::Esc => {
                state.show_agent_picker = false;
            }
            KeyCode::Down => {
                if !state.agents.is_empty() {
                    state.selected_agent = (state.selected_agent + 1) % state.agents.len();
                }
            }
            KeyCode::Up => {
                if !state.agents.is_empty() {
                    if state.selected_agent == 0 {
                        state.selected_agent = state.agents.len() - 1;
                    } else {
                        state.selected_agent -= 1;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(agent) = state.agents.get(state.selected_agent).cloned() {
                    state.show_agent_picker = false;
                    switch_active_agent(state, runtime, &agent.id)?;
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    if state.show_session_picker {
        match key.code {
            KeyCode::Esc => {
                state.show_session_picker = false;
            }
            KeyCode::Down => state.reduce(TuiAction::SelectNextSession),
            KeyCode::Up => state.reduce(TuiAction::SelectPrevSession),
            KeyCode::Enter => {
                if let Some(entry) = state.sessions.get(state.selected_session) {
                    let session_id = entry.session_id.clone();
                    state.show_session_picker = false;
                    load_selected_session(state, session_store, runtime, &session_id)?;
                }
            }
            _ => {}
        }
        return Ok(false);
    }

    if (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('i'))
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Tab)
    {
        state.reduce(TuiAction::ToggleInspector);
        return Ok(false);
    }
    if key.code == KeyCode::BackTab {
        state.focus = state.focus.prev(state.show_inspector);
        if state.focus != TuiFocus::Input {
            state.dismiss_startup_surface();
        }
        return Ok(false);
    }
    if key.code == KeyCode::Tab
        && state.focus == TuiFocus::Input
        && state.input.trim_start().starts_with('/')
    {
        if let Some(item) = selected_command_palette_item(&state.input, state.command_palette_index)
        {
            state.input = item.insert_text.to_string();
            state.reset_command_palette_selection();
        }
        return Ok(false);
    }
    if key.code == KeyCode::Tab {
        state.reduce(TuiAction::CycleFocus);
        if state.focus != TuiFocus::Input {
            state.dismiss_startup_surface();
        }
        return Ok(false);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
        state.reduce(TuiAction::NewSession);
        return Ok(false);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        state.refresh_sessions(session_store)?;
        state.status = "session list refreshed".to_string();
        return Ok(false);
    }
    if key.code == KeyCode::Char('?') {
        state.reduce(TuiAction::ToggleHelp);
        return Ok(false);
    }
    if key.code == KeyCode::Char('q') && key.modifiers.is_empty() {
        return Ok(true);
    }

    match state.focus {
        TuiFocus::Sessions => match key.code {
            KeyCode::Down => state.reduce(TuiAction::SelectNextSession),
            KeyCode::Up => state.reduce(TuiAction::SelectPrevSession),
            KeyCode::Enter => {
                if let Some(entry) = state.sessions.get(state.selected_session) {
                    let session_id = entry.session_id.clone();
                    load_selected_session(state, session_store, runtime, &session_id)?;
                }
            }
            _ => {}
        },
        TuiFocus::Input => match key.code {
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.input.push('\n');
                state.reset_command_palette_selection();
            }
            KeyCode::Backspace => {
                state.input.pop();
                state.reset_command_palette_selection();
            }
            KeyCode::Down if state.input.trim_start().starts_with('/') => {
                state.select_next_command(command_palette_items(&state.input).len());
            }
            KeyCode::Up if state.input.trim_start().starts_with('/') => {
                state.select_prev_command(command_palette_items(&state.input).len());
            }
            KeyCode::Enter => {
                if !state.running {
                    let prompt = state.input.trim().to_string();
                    if !prompt.is_empty() {
                        if let Some(command) = parse_input_command(&prompt) {
                            state.input.clear();
                            state.reset_command_palette_selection();
                            handle_input_command(state, session_store, runtime, command)?;
                        } else if prompt.starts_with('/') {
                            if let Some(item) =
                                selected_command_palette_item(&prompt, state.command_palette_index)
                            {
                                state.status = if item.implemented {
                                    format!("complete command arguments for {}", item.insert_text)
                                } else {
                                    format!(
                                        "command not implemented in mosaic tui yet: {}",
                                        item.insert_text.trim_end()
                                    )
                                };
                            } else {
                                state.status = format!("unknown slash command: {prompt}");
                            }
                        } else {
                            state.input.clear();
                            state.reset_command_palette_selection();
                            state.dismiss_startup_surface();
                            state.running = true;
                            state.status = "running".to_string();
                            crate::spawn_agent_task(state, runtime, options, app_tx, prompt);
                        }
                    }
                }
            }
            KeyCode::Char(ch) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    state.input.push(ch);
                    state.reset_command_palette_selection();
                }
            }
            _ => {}
        },
        TuiFocus::Messages | TuiFocus::Inspector => {}
    }

    Ok(false)
}
