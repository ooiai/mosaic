use mosaic_core::error::Result;
use mosaic_core::session::SessionStore;

use crate::commands::TuiInputCommand;
use crate::render::compose_status_line;
use crate::state::{InspectorLine, TuiAction, TuiState};
use crate::{SwitchedTuiRuntime, TuiLocalCommand, TuiRuntime};

pub(crate) fn load_selected_session(
    state: &mut TuiState,
    session_store: &SessionStore,
    runtime: &mut TuiRuntime,
    session_id: &str,
) -> Result<()> {
    let switched = match (runtime.load_session_runtime)(Some(session_id)) {
        Ok(switched) => switched,
        Err(err) => {
            state.status = format!("error: {err}");
            state.inspector.push(InspectorLine {
                kind: "error".to_string(),
                detail: err.to_string(),
            });
            return Ok(());
        }
    };
    apply_switched_runtime(runtime, switched);
    state.load_session(session_store, Some(session_id.to_string()))?;
    state.dismiss_startup_surface();
    let active_agent = runtime.agent_id.as_deref().unwrap_or("<none>");
    state.status = format!("resumed session={session_id} | agent={active_agent}");
    Ok(())
}

pub(crate) async fn handle_input_command(
    state: &mut TuiState,
    session_store: &SessionStore,
    runtime: &mut TuiRuntime,
    command: TuiInputCommand<'_>,
) -> Result<()> {
    match command {
        TuiInputCommand::Help => {
            state.reduce(TuiAction::ToggleHelp);
        }
        TuiInputCommand::Agent => {
            state.status = format!("agent={}", runtime.agent_id.as_deref().unwrap_or("<none>"));
        }
        TuiInputCommand::Agents => {
            toggle_agent_picker(state, runtime)?;
        }
        TuiInputCommand::AgentSet(agent_id) => {
            switch_active_agent(state, runtime, agent_id)?;
        }
        TuiInputCommand::Session => {
            state.status = format!(
                "session={}",
                state.active_session_id.as_deref().unwrap_or("<new>")
            );
        }
        TuiInputCommand::SessionSet(session_id) => {
            load_selected_session(state, session_store, runtime, session_id)?;
        }
        TuiInputCommand::NewSession => {
            state.reduce(TuiAction::NewSession);
        }
        TuiInputCommand::Status => {
            state.status = compose_status_line(
                "runtime",
                &runtime.profile_name,
                runtime.agent_id.as_deref(),
                state.active_session_id.as_deref(),
                &runtime.policy_summary,
            );
        }
        TuiInputCommand::Models => {
            run_local_command(state, runtime, TuiLocalCommand::Models).await;
        }
        TuiInputCommand::Skills => {
            run_local_command(state, runtime, TuiLocalCommand::Skills).await;
        }
        TuiInputCommand::Docs => {
            run_local_command(state, runtime, TuiLocalCommand::Docs).await;
        }
        TuiInputCommand::Logs => {
            run_local_command(state, runtime, TuiLocalCommand::Logs).await;
        }
        TuiInputCommand::Doctor => {
            run_local_command(state, runtime, TuiLocalCommand::Doctor).await;
        }
        TuiInputCommand::Memory => {
            run_local_command(state, runtime, TuiLocalCommand::Memory).await;
        }
        TuiInputCommand::Knowledge => {
            run_local_command(state, runtime, TuiLocalCommand::Knowledge).await;
        }
        TuiInputCommand::Plugins => {
            run_local_command(state, runtime, TuiLocalCommand::Plugins).await;
        }
    }
    Ok(())
}

async fn run_local_command(state: &mut TuiState, runtime: &TuiRuntime, command: TuiLocalCommand) {
    match (runtime.run_local_command)(command).await {
        Ok(output) => state.apply_local_command_output(command, output),
        Err(err) => state.apply_local_command_error(command, err.to_string()),
    }
}

pub(crate) fn toggle_agent_picker(state: &mut TuiState, runtime: &TuiRuntime) -> Result<()> {
    if state.show_agent_picker {
        state.show_agent_picker = false;
        return Ok(());
    }

    let agents = (runtime.list_agents)()?;
    if agents.is_empty() {
        state.status = "no configured agents".to_string();
        return Ok(());
    }
    state.selected_agent = runtime
        .agent_id
        .as_ref()
        .and_then(|active| agents.iter().position(|entry| &entry.id == active))
        .unwrap_or(0);
    state.agents = agents;
    state.show_session_picker = false;
    state.show_agent_picker = true;
    state.status = "agent picker".to_string();
    Ok(())
}

pub(crate) fn toggle_session_picker(
    state: &mut TuiState,
    session_store: &SessionStore,
) -> Result<()> {
    if state.show_session_picker {
        state.show_session_picker = false;
        return Ok(());
    }

    state.refresh_sessions(session_store)?;
    if state.sessions.is_empty() {
        state.status = "no sessions".to_string();
        return Ok(());
    }
    state.show_agent_picker = false;
    state.show_session_picker = true;
    state.status = "session picker".to_string();
    Ok(())
}

pub(crate) fn switch_active_agent(
    state: &mut TuiState,
    runtime: &mut TuiRuntime,
    agent_id: &str,
) -> Result<()> {
    if runtime.agent_id.as_deref() == Some(agent_id) {
        state.status = format!("agent unchanged={agent_id}");
        return Ok(());
    }

    let switched = match (runtime.switch_agent)(agent_id) {
        Ok(switched) => switched,
        Err(err) => {
            state.status = format!("error: {err}");
            state.inspector.push(InspectorLine {
                kind: "error".to_string(),
                detail: err.to_string(),
            });
            return Ok(());
        }
    };

    let had_session = state.active_session_id.is_some();
    if had_session {
        state.reduce(TuiAction::NewSession);
    }
    apply_switched_runtime(runtime, switched);

    let active_agent = runtime.agent_id.as_deref().unwrap_or("<none>");
    state.status = if had_session {
        format!("agent switched={active_agent} | session reset")
    } else {
        format!("agent switched={active_agent}")
    };
    state.inspector.push(InspectorLine {
        kind: "agent".to_string(),
        detail: if had_session {
            format!("switched to {active_agent}; new session started")
        } else {
            format!("switched to {active_agent}")
        },
    });
    Ok(())
}

fn apply_switched_runtime(runtime: &mut TuiRuntime, switched: SwitchedTuiRuntime) {
    runtime.agent = switched.agent;
    runtime.profile_name = switched.profile_name;
    runtime.agent_id = switched.agent_id;
    runtime.model_name = switched.model_name;
    runtime.session_metadata = switched.session_metadata;
    runtime.policy_summary = switched.policy_summary;
}
