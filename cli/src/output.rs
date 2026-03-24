use mosaic_runtime::events::{RunEvent, RunEventSink};

pub struct CliEventSink;

impl RunEventSink for CliEventSink {
    fn emit(&self, event: RunEvent) {
        match event {
            RunEvent::RunStarted { .. } => {
                println!("[run] starting");
            }
            RunEvent::SkillStarted { name } => {
                println!("[run] executing skill: {}", name);
            }
            RunEvent::SkillFinished { name } => {
                println!("[run] skill finished: {}", name);
            }
            RunEvent::SkillFailed { name, error } => {
                println!("[run] skill failed: {} error={}", name, error);
            }
            RunEvent::ProviderRequest {
                tool_count,
                message_count,
            } => {
                println!(
                    "[run] provider=request tools={} messages={}",
                    tool_count, message_count
                );
            }
            RunEvent::ToolCalling { name, call_id } => {
                println!("[run] calling tool: {} (call_id={})", name, call_id);
            }
            RunEvent::ToolFinished { name, call_id } => {
                println!("[run] tool finished: {} (call_id={})", name, call_id);
            }
            RunEvent::ToolFailed {
                name,
                call_id,
                error,
            } => {
                println!(
                    "[run] tool failed: {} (call_id={}) error={}",
                    name, call_id, error
                );
            }
            RunEvent::FinalAnswerReady => {
                println!("[run] final answer ready");
            }
            RunEvent::RunFinished { .. } => {
                println!("[run] finished");
            }
            RunEvent::RunFailed { error } => {
                println!("[run] failed: {}", error);
            }
        }
    }
}
