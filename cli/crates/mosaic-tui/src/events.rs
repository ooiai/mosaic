use mosaic_agent::{AgentEvent, AgentRunResult};
use mosaic_core::error::Result;

#[derive(Debug)]
pub(crate) enum AppEvent {
    Agent(AgentEvent),
    AskDone(Result<AgentRunResult>),
}
