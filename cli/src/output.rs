use mosaic_runtime::events::{RunEvent, RunEventSink};

pub struct CliEventSink;

fn format_run_event(event: &RunEvent) -> String {
    match event {
        RunEvent::RunStarted { .. } => "[run] starting".to_owned(),
        RunEvent::SkillStarted { name } => format!("[run] executing skill: {}", name),
        RunEvent::SkillFinished { name } => format!("[run] skill finished: {}", name),
        RunEvent::SkillFailed { name, error } => {
            format!("[run] skill failed: {} error={}", name, error)
        }
        RunEvent::ProviderRequest {
            tool_count,
            message_count,
        } => {
            format!(
                "[run] provider=request tools={} messages={}",
                tool_count, message_count
            )
        }
        RunEvent::ToolCalling { name, call_id } => {
            format!("[run] calling tool: {} (call_id={})", name, call_id)
        }
        RunEvent::ToolFinished { name, call_id } => {
            format!("[run] tool finished: {} (call_id={})", name, call_id)
        }
        RunEvent::ToolFailed {
            name,
            call_id,
            error,
        } => {
            format!(
                "[run] tool failed: {} (call_id={}) error={}",
                name, call_id, error
            )
        }
        RunEvent::FinalAnswerReady => "[run] final answer ready".to_owned(),
        RunEvent::RunFinished { .. } => "[run] finished".to_owned(),
        RunEvent::RunFailed { error } => format!("[run] failed: {}", error),
    }
}

impl RunEventSink for CliEventSink {
    fn emit(&self, event: RunEvent) {
        println!("{}", format_run_event(&event));
    }
}

#[cfg(test)]
mod tests {
    use mosaic_runtime::events::RunEvent;

    use super::format_run_event;

    #[test]
    fn formats_provider_requests_with_stable_field_order() {
        let line = format_run_event(&RunEvent::ProviderRequest {
            tool_count: 2,
            message_count: 3,
        });

        assert_eq!(line, "[run] provider=request tools=2 messages=3");
    }

    #[test]
    fn formats_tool_failures_with_call_id_before_error() {
        let line = format_run_event(&RunEvent::ToolFailed {
            name: "read_file".to_owned(),
            call_id: "call_123".to_owned(),
            error: "permission denied".to_owned(),
        });

        assert_eq!(
            line,
            "[run] tool failed: read_file (call_id=call_123) error=permission denied"
        );
    }

    #[test]
    fn formats_run_failure_lines() {
        let line = format_run_event(&RunEvent::RunFailed {
            error: "provider failure".to_owned(),
        });

        assert_eq!(line, "[run] failed: provider failure");
    }
}
