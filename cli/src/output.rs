use anyhow::Result;
use mosaic_config::{
    DoctorReport, LoadedMosaicConfig, ProviderUsage, RedactedMosaicConfig, ValidationReport,
};
use mosaic_inspect::RunTrace;
use mosaic_runtime::events::{RunEvent, RunEventSink};
use tracing::info;

pub struct CliEventSink;

#[derive(Debug, Clone, PartialEq, Eq)]
struct OnboardingView {
    state: &'static str,
    active_profile_usage: &'static str,
    message: String,
}

fn format_run_event(event: &RunEvent) -> String {
    match event {
        RunEvent::RunStarted { run_id, .. } => format!("[run] starting run_id={}", run_id),
        RunEvent::WorkflowStarted { name, step_count } => {
            format!("[run] workflow started: {} steps={}", name, step_count)
        }
        RunEvent::WorkflowStepStarted {
            workflow,
            step,
            kind,
        } => format!(
            "[run] workflow step started: {}.{} kind={}",
            workflow, step, kind
        ),
        RunEvent::WorkflowStepFinished { workflow, step } => {
            format!("[run] workflow step finished: {}.{}", workflow, step)
        }
        RunEvent::WorkflowStepFailed {
            workflow,
            step,
            error,
        } => format!(
            "[run] workflow step failed: {}.{} error={}",
            workflow, step, error
        ),
        RunEvent::WorkflowFinished { name } => {
            format!("[run] workflow finished: {}", name)
        }
        RunEvent::SkillStarted { name } => format!("[run] executing skill: {}", name),
        RunEvent::SkillFinished { name } => format!("[run] skill finished: {}", name),
        RunEvent::SkillFailed { name, error } => {
            format!("[run] skill failed: {} error={}", name, error)
        }
        RunEvent::ProviderRequest {
            provider_type,
            profile,
            model,
            tool_count,
            message_count,
            max_attempts,
        } => {
            format!(
                "[run] provider=request provider={} profile={} model={} tools={} messages={} attempts={}",
                provider_type, profile, model, tool_count, message_count, max_attempts
            )
        }
        RunEvent::ProviderRetry {
            provider_type,
            profile,
            model,
            attempt,
            max_attempts,
            kind,
            status_code,
            error,
            ..
        } => format!(
            "[run] provider retry: provider={} profile={} model={} attempt={}/{} kind={} status={:?} error={}",
            provider_type, profile, model, attempt, max_attempts, kind, status_code, error
        ),
        RunEvent::ProviderFailed {
            provider_type,
            profile,
            model,
            kind,
            status_code,
            error,
            ..
        } => format!(
            "[run] provider failed: provider={} profile={} model={} kind={} status={:?} error={}",
            provider_type, profile, model, kind, status_code, error
        ),
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
        RunEvent::CapabilityJobQueued {
            name, kind, risk, ..
        } => format!(
            "[run] capability queued: {} kind={} risk={}",
            name, kind, risk
        ),
        RunEvent::CapabilityJobStarted { name, job_id } => {
            format!("[run] capability started: {} (job_id={})", name, job_id)
        }
        RunEvent::CapabilityJobRetried {
            name,
            attempt,
            error,
            ..
        } => format!(
            "[run] capability retry: {} attempt={} error={}",
            name, attempt, error
        ),
        RunEvent::CapabilityJobFinished {
            name,
            status,
            summary,
            ..
        } => format!(
            "[run] capability finished: {} status={} summary={}",
            name, status, summary
        ),
        RunEvent::CapabilityJobFailed { name, error, .. } => {
            format!("[run] capability failed: {} error={}", name, error)
        }
        RunEvent::PermissionCheckFailed { name, reason, .. } => {
            format!("[run] permission check failed: {} reason={}", name, reason)
        }
        RunEvent::OutputDelta {
            run_id,
            chunk,
            accumulated_chars,
        } => format!(
            "[run] streaming: run_id={} chars={} chunk={}",
            run_id,
            accumulated_chars,
            truncate(&single_line(chunk), 48)
        ),
        RunEvent::FinalAnswerReady { run_id } => {
            format!("[run] final answer ready run_id={}", run_id)
        }
        RunEvent::RunFinished { run_id, .. } => format!("[run] finished run_id={}", run_id),
        RunEvent::RunFailed {
            run_id,
            error,
            failure_kind,
        } => match failure_kind {
            Some(kind) => format!(
                "[run] failed run_id={} kind={} error={}",
                run_id, kind, error
            ),
            None => format!("[run] failed run_id={} error={}", run_id, error),
        },
        RunEvent::RunCanceled { run_id, reason } => {
            format!("[run] canceled run_id={} reason={}", run_id, reason)
        }
    }
}

impl RunEventSink for CliEventSink {
    fn emit(&self, event: RunEvent) {
        info!(event = %format_run_event(&event), "runtime event");
    }
}

pub fn render_next_steps<I, S>(steps: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let collected = steps
        .into_iter()
        .map(|step| step.as_ref().trim().to_owned())
        .filter(|step| !step.is_empty())
        .collect::<Vec<_>>();
    if collected.is_empty() {
        return String::new();
    }

    let mut out = String::from("next:\n");
    for step in collected {
        out.push_str(&format!("  {}\n", step));
    }
    out
}

pub fn render_config_sources(loaded: &LoadedMosaicConfig, validation: &ValidationReport) -> String {
    let redacted = mosaic_config::redact_mosaic_config(&loaded.config);
    let onboarding = derive_onboarding(&redacted, validation);
    let rows = vec![
        ("active_profile", loaded.config.active_profile.clone()),
        (
            "active_profile_usage",
            onboarding.active_profile_usage.to_owned(),
        ),
        ("onboarding_state", onboarding.state.to_owned()),
        ("source_layers", loaded.sources.len().to_string()),
        (
            "workspace_config",
            loaded.workspace_config_path.display().to_string(),
        ),
        (
            "user_config",
            loaded
                .user_config_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<none>".to_owned()),
        ),
    ];
    let source_lines = loaded
        .sources
        .iter()
        .enumerate()
        .map(|(idx, source)| {
            format!(
                "{}. {} | {}",
                idx + 1,
                config_source_label(&source.kind),
                source.detail
            )
        })
        .collect::<Vec<_>>();

    join_blocks([
        render_key_value_block("config source summary", rows),
        render_key_value_block(
            "onboarding",
            vec![
                ("state", onboarding.state.to_owned()),
                (
                    "active_profile_usage",
                    onboarding.active_profile_usage.to_owned(),
                ),
                ("message", onboarding.message),
            ],
        ),
        render_list_block("config sources", source_lines),
        render_list_block(
            "config precedence",
            vec![
                "Later layers override earlier ones.".to_owned(),
                "Order: defaults -> user -> workspace -> env -> cli.".to_owned(),
            ],
        ),
    ])
}

pub fn render_config_show(loaded: &LoadedMosaicConfig, validation: &ValidationReport) -> String {
    let redacted = mosaic_config::redact_mosaic_config(&loaded.config);
    render_redacted_config(&redacted, validation, Some(&loaded.workspace_config_path))
}

pub fn render_onboarding_json(
    loaded: &LoadedMosaicConfig,
    validation: &ValidationReport,
) -> serde_json::Value {
    let redacted = mosaic_config::redact_mosaic_config(&loaded.config);
    let onboarding = derive_onboarding(&redacted, validation);
    serde_json::json!({
        "state": onboarding.state,
        "active_profile_usage": onboarding.active_profile_usage,
        "message": onboarding.message,
    })
}

pub fn render_doctor_report(doctor: &DoctorReport, workspace: &RedactedMosaicConfig) -> String {
    let summary = doctor.summary();
    let onboarding = derive_onboarding(workspace, &doctor.validation);
    let status = if doctor.has_errors() {
        "issues found"
    } else if summary.warnings > 0 || !doctor.validation.issues.is_empty() {
        "attention needed"
    } else {
        "ok"
    };
    let category_lines = summary
        .categories
        .iter()
        .map(|entry| {
            format!(
                "{} | ok={} warning={} error={}",
                entry.category.label(),
                entry.ok,
                entry.warnings,
                entry.errors,
            )
        })
        .collect::<Vec<_>>();
    let check_lines = doctor
        .checks
        .iter()
        .map(|check| {
            format!(
                "[{}] {}: {}",
                doctor_status_label(&check.status),
                check.category.label(),
                check.message,
            )
        })
        .collect::<Vec<_>>();

    let mut blocks = vec![render_key_value_block(
        "doctor summary",
        vec![
            ("status", status.to_owned()),
            (
                "validation_errors",
                validation_error_count(&doctor.validation).to_string(),
            ),
            (
                "validation_warnings",
                validation_warning_count(&doctor.validation).to_string(),
            ),
            ("checks", doctor.checks.len().to_string()),
            ("ok", summary.ok.to_string()),
            ("warnings", summary.warnings.to_string()),
            ("errors", summary.errors.to_string()),
            ("onboarding_state", onboarding.state.to_owned()),
        ],
    )];

    blocks.push(render_key_value_block(
        "onboarding",
        vec![
            ("state", onboarding.state.to_owned()),
            (
                "active_profile_usage",
                onboarding.active_profile_usage.to_owned(),
            ),
            ("message", onboarding.message),
        ],
    ));

    if !category_lines.is_empty() {
        blocks.push(render_list_block("doctor categories", category_lines));
    }

    if !doctor.validation.issues.is_empty() {
        let validation_lines = doctor
            .validation
            .issues
            .iter()
            .map(|issue| {
                format!(
                    "[{}] {}: {}",
                    validation_level_label(&issue.level),
                    issue.field,
                    issue.message,
                )
            })
            .collect::<Vec<_>>();
        blocks.push(render_list_block("validation issues", validation_lines));
    }

    if !check_lines.is_empty() {
        blocks.push(render_list_block("doctor checks", check_lines));
    }

    join_blocks(blocks)
}

pub fn render_gateway_status(
    health: &mosaic_control_protocol::HealthResponse,
    readiness: &mosaic_control_protocol::ReadinessResponse,
    metrics: &mosaic_control_protocol::MetricsResponse,
) -> String {
    join_blocks([
        render_key_value_block(
            "gateway summary",
            vec![
                ("status", health.status.clone()),
                ("active_profile", health.active_profile.clone()),
                ("deployment_profile", health.deployment_profile.clone()),
                ("auth_mode", health.auth_mode.clone()),
                ("transport", health.transport.clone()),
                ("sessions", health.session_count.to_string()),
                ("replay_window", health.event_replay_window.to_string()),
            ],
        ),
        render_key_value_block(
            "gateway readiness",
            vec![
                ("status", readiness.status.clone()),
                (
                    "session_store_ready",
                    readiness.session_store_ready.to_string(),
                ),
                ("audit_ready", readiness.audit_ready.to_string()),
                (
                    "replay_events_buffered",
                    readiness.replay_events_buffered.to_string(),
                ),
                (
                    "slow_consumer_threshold",
                    readiness.slow_consumer_lag_threshold.to_string(),
                ),
                ("extensions", readiness.extension_count.to_string()),
            ],
        ),
        render_key_value_block(
            "gateway metrics",
            vec![
                ("queued_runs", metrics.queued_run_count.to_string()),
                ("running_runs", metrics.running_run_count.to_string()),
                ("completed_runs", metrics.completed_runs_total.to_string()),
                ("failed_runs", metrics.failed_runs_total.to_string()),
                ("canceled_runs", metrics.canceled_runs_total.to_string()),
                (
                    "capability_jobs_total",
                    metrics.capability_jobs_total.to_string(),
                ),
                ("audit_events_total", metrics.audit_events_total.to_string()),
                ("auth_denials_total", metrics.auth_denials_total.to_string()),
                (
                    "lagged_events_total",
                    metrics.broadcast_lag_events_total.to_string(),
                ),
                ("open_jobs", metrics.capability_job_count.to_string()),
            ],
        ),
    ])
}

pub fn render_inspect_report(
    trace: &RunTrace,
    workspace: Option<&RedactedMosaicConfig>,
    verbose: bool,
) -> Result<String> {
    let summary = trace.summary();
    let mut blocks = vec![render_key_value_block(
        "run summary",
        vec![
            ("run_id", trace.run_id.clone()),
            ("status", summary.status.clone()),
            ("duration_ms", option_i64(summary.duration_ms)),
            (
                "gateway_run_id",
                option_string(trace.gateway_run_id.clone()),
            ),
            (
                "correlation_id",
                option_string(trace.correlation_id.clone()),
            ),
            ("session_id", option_string(trace.session_id.clone())),
            ("session_route", option_string(trace.session_route.clone())),
            ("workflow_name", option_string(trace.workflow_name.clone())),
            ("started_at", trace.started_at.to_rfc3339()),
            ("finished_at", option_datetime(trace.finished_at)),
            ("input_preview", truncate(&trace.input, 120)),
            (
                "output_preview",
                option_preview(trace.output.as_deref(), 120),
            ),
            ("error", option_preview(trace.error.as_deref(), 120)),
            ("failure_kind", option_string(summary.failure_kind.clone())),
            (
                "lifecycle_status",
                trace.lifecycle_status.label().to_owned(),
            ),
        ],
    )];

    blocks.push(render_key_value_block(
        "run activity",
        vec![
            ("tool_calls", summary.tool_calls.to_string()),
            (
                "capability_invocations",
                summary.capability_invocations.to_string(),
            ),
            ("skill_calls", summary.skill_calls.to_string()),
            ("workflow_steps", summary.workflow_steps.to_string()),
            ("provider_attempts", summary.provider_attempts.to_string()),
            ("model_selections", summary.model_selections.to_string()),
            ("memory_reads", summary.memory_reads.to_string()),
            ("memory_writes", summary.memory_writes.to_string()),
            ("active_extensions", summary.active_extensions.to_string()),
            ("used_extensions", summary.used_extensions.to_string()),
            ("compression", yes_no(summary.has_compression).to_owned()),
            ("output_chunks", summary.output_chunks.to_string()),
            ("integrity_warnings", summary.integrity_warnings.to_string()),
        ],
    ));

    if let Some(profile) = &trace.effective_profile {
        blocks.push(render_key_value_block(
            "effective profile",
            vec![
                ("profile", profile.profile.clone()),
                ("provider_type", profile.provider_type.clone()),
                ("model", profile.model.clone()),
                ("base_url", option_string(profile.base_url.clone())),
                ("api_key_env", option_string(profile.api_key_env.clone())),
                ("api_key_present", profile.api_key_present.to_string()),
                ("timeout_ms", profile.timeout_ms.to_string()),
                ("max_retries", profile.max_retries.to_string()),
                ("retry_backoff_ms", profile.retry_backoff_ms.to_string()),
                ("api_version", option_string(profile.api_version.clone())),
                (
                    "version_header",
                    option_string(profile.version_header.clone()),
                ),
                (
                    "custom_header_keys",
                    if profile.custom_header_keys.is_empty() {
                        "<none>".to_owned()
                    } else {
                        profile.custom_header_keys.join(", ")
                    },
                ),
                ("supports_tools", profile.supports_tools.to_string()),
                ("supports_vision", profile.supports_vision.to_string()),
                (
                    "supports_tool_call_shadow_messages",
                    profile.supports_tool_call_shadow_messages.to_string(),
                ),
            ],
        ));
    }

    if let Some(route) = &trace.route_decision {
        blocks.push(render_key_value_block(
            "route decision",
            vec![
                ("route_mode", route.route_mode.label().to_owned()),
                (
                    "selected_capability_type",
                    option_string(route.selected_capability_type.clone()),
                ),
                (
                    "selected_capability_name",
                    option_string(route.selected_capability_name.clone()),
                ),
                ("selected_tool", option_string(route.selected_tool.clone())),
                (
                    "selected_skill",
                    option_string(route.selected_skill.clone()),
                ),
                (
                    "selected_workflow",
                    option_string(route.selected_workflow.clone()),
                ),
                ("selection_reason", route.selection_reason.clone()),
                (
                    "capability_source",
                    option_string(route.capability_source.clone()),
                ),
                ("profile_used", option_string(route.profile_used.clone())),
                (
                    "selected_category",
                    option_string(route.selected_category.clone()),
                ),
                ("catalog_scope", option_string(route.catalog_scope.clone())),
            ],
        ));
    }

    if let Some(route) = &trace.attachment_route {
        blocks.push(render_key_value_block(
            "attachment route",
            vec![
                ("mode", route.mode.label().to_owned()),
                ("selection_reason", route.selection_reason.clone()),
                (
                    "provider_profile",
                    option_string(route.provider_profile.clone()),
                ),
                (
                    "provider_model",
                    option_string(route.provider_model.clone()),
                ),
                ("processor", option_string(route.processor.clone())),
                ("attachment_count", route.attachment_count.to_string()),
                (
                    "attachment_kinds",
                    if route.attachment_kinds.is_empty() {
                        "<none>".to_owned()
                    } else {
                        route.attachment_kinds.join(", ")
                    },
                ),
                (
                    "attachment_filenames",
                    if route.attachment_filenames.is_empty() {
                        "<none>".to_owned()
                    } else {
                        route.attachment_filenames.join(", ")
                    },
                ),
            ],
        ));
    }

    if let Some(runtime_policy) = &trace.runtime_policy {
        blocks.push(render_key_value_block(
            "runtime policy",
            vec![
                (
                    "max_provider_round_trips",
                    runtime_policy.max_provider_round_trips.to_string(),
                ),
                (
                    "max_workflow_provider_round_trips",
                    runtime_policy.max_workflow_provider_round_trips.to_string(),
                ),
                (
                    "continue_after_tool_error",
                    runtime_policy.continue_after_tool_error.to_string(),
                ),
            ],
        ));
    }

    if let Some(ingress) = &trace.ingress {
        blocks.push(render_key_value_block(
            "ingress",
            vec![
                ("kind", ingress.kind.clone()),
                ("channel", option_string(ingress.channel.clone())),
                ("adapter", option_string(ingress.adapter.clone())),
                ("source", option_string(ingress.source.clone())),
                ("actor_id", option_string(ingress.actor_id.clone())),
                ("display_name", option_string(ingress.display_name.clone())),
                (
                    "conversation_id",
                    option_string(ingress.conversation_id.clone()),
                ),
                ("thread_id", option_string(ingress.thread_id.clone())),
                ("reply_target", option_string(ingress.reply_target.clone())),
                ("message_id", option_string(ingress.message_id.clone())),
                ("received_at", option_datetime(ingress.received_at)),
                ("raw_event_id", option_string(ingress.raw_event_id.clone())),
                ("session_hint", option_string(ingress.session_hint.clone())),
                ("profile_hint", option_string(ingress.profile_hint.clone())),
                (
                    "control_command",
                    option_string(ingress.control_command.clone()),
                ),
                (
                    "original_text",
                    option_preview(ingress.original_text.as_deref(), 120),
                ),
                ("attachments", ingress.attachments.len().to_string()),
                (
                    "attachment_failures",
                    ingress.attachment_failures.len().to_string(),
                ),
                ("gateway_url", option_string(ingress.gateway_url.clone())),
            ],
        ));
    }

    if let Some(failure) = &trace.failure {
        blocks.push(render_key_value_block(
            "run failure",
            vec![
                ("kind", failure.kind.clone()),
                ("stage", failure.stage.clone()),
                ("retryable", failure.retryable.to_string()),
                ("message", truncate(&failure.message, 160)),
            ],
        ));
    }

    if let Some(side_effect_summary) = &trace.side_effect_summary {
        blocks.push(render_key_value_block(
            "side-effect summary",
            vec![
                ("total", side_effect_summary.total.to_string()),
                ("failed", side_effect_summary.failed.to_string()),
                ("high_risk", side_effect_summary.high_risk.to_string()),
                (
                    "capability_kinds",
                    if side_effect_summary.capability_kinds.is_empty() {
                        "<none>".to_owned()
                    } else {
                        side_effect_summary.capability_kinds.join(", ")
                    },
                ),
            ],
        ));
    }

    if !verbose {
        return Ok(join_blocks(blocks));
    }

    if !trace.integrity_warnings.is_empty() {
        blocks.push(render_list_block(
            "integrity warnings",
            trace.integrity_warnings.clone(),
        ));
    }

    if let Some(provider_failure) = &trace.provider_failure {
        blocks.push(render_key_value_block(
            "provider failure",
            vec![
                ("kind", provider_failure.kind.clone()),
                ("status_code", option_u16(provider_failure.status_code)),
                ("retryable", provider_failure.retryable.to_string()),
                ("message", provider_failure.message.clone()),
            ],
        ));
    }

    if let Some(governance) = &trace.governance {
        blocks.push(render_key_value_block(
            "governance",
            vec![
                ("deployment_profile", governance.deployment_profile.clone()),
                ("workspace_name", governance.workspace_name.clone()),
                ("auth_mode", governance.auth_mode.clone()),
                (
                    "audit_retention_days",
                    governance.audit_retention_days.to_string(),
                ),
                (
                    "event_replay_window",
                    governance.event_replay_window.to_string(),
                ),
                ("redact_inputs", governance.redact_inputs.to_string()),
            ],
        ));
    }

    if !trace.active_extensions.is_empty() {
        blocks.push(render_list_block(
            "active extensions",
            trace
                .active_extensions
                .iter()
                .map(|extension| {
                    format!(
                        "{}@{} | source={} | enabled={} | active={} | error={}",
                        extension.name,
                        extension.version,
                        extension.source,
                        extension.enabled,
                        extension.active,
                        option_string(extension.error.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }

    if !trace.used_extensions.is_empty() {
        blocks.push(render_list_block(
            "used extensions",
            trace
                .used_extensions
                .iter()
                .map(|usage| {
                    format!(
                        "{}@{} | {}:{}",
                        usage.name, usage.version, usage.component_kind, usage.component_name
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }

    if !trace.provider_attempts.is_empty() {
        blocks.push(render_list_block(
            "provider attempts",
            trace
                .provider_attempts
                .iter()
                .map(|attempt| format!(
                    "attempt={}/{} | status={} | error_kind={} | status_code={} | retryable={} | message={}",
                    attempt.attempt,
                    attempt.max_attempts,
                    attempt.status,
                    option_string(attempt.error_kind.clone()),
                    option_u16(attempt.status_code),
                    attempt.retryable,
                    option_string(attempt.message.clone()),
                ))
                .collect::<Vec<_>>(),
        ));
    }

    if !trace.outbound_deliveries.is_empty() {
        blocks.push(render_list_block(
            "outbound deliveries",
            trace
                .outbound_deliveries
                .iter()
                .map(|delivery| {
                    format!(
                        "channel={} | adapter={} | target={} | status={} | retries={} | provider_message_id={} | error_kind={} | error={} | delivered_at={}",
                        delivery.message.channel,
                        delivery.message.adapter,
                        delivery.message.reply_target,
                        delivery.result.status.label(),
                        delivery.result.retry_count,
                        option_string(delivery.result.provider_message_id.clone()),
                        option_string(delivery.result.error_kind.clone()),
                        option_string(delivery.result.error.clone()),
                        option_datetime(delivery.result.delivered_at),
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }

    if let Some(ingress) = &trace.ingress {
        if !ingress.attachments.is_empty() {
            blocks.push(render_list_block(
                "attachments",
                ingress
                    .attachments
                    .iter()
                    .map(|attachment| {
                        format!(
                            "id={} | kind={} | filename={} | mime_type={} | size_bytes={} | source_ref={} | local_cache_path={}",
                            attachment.id,
                            attachment.kind.label(),
                            option_string(attachment.filename.clone()),
                            option_string(attachment.mime_type.clone()),
                            attachment
                                .size_bytes
                                .map(|size| size.to_string())
                                .unwrap_or_else(|| "<none>".to_owned()),
                            option_string(attachment.source_ref.clone()),
                            option_string(attachment.local_cache_path.clone()),
                        )
                    })
                    .collect::<Vec<_>>(),
            ));
        }
        if !ingress.attachment_failures.is_empty() {
            blocks.push(render_list_block(
                "attachment failures",
                ingress
                    .attachment_failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "attachment_id={} | stage={} | kind={} | message={}",
                            failure.attachment_id, failure.stage, failure.kind, failure.message
                        )
                    })
                    .collect::<Vec<_>>(),
            ));
        }
    }

    if !trace.model_selections.is_empty() {
        blocks.push(render_list_block(
            "model selections",
            trace
                .model_selections
                .iter()
                .map(|selection| format!(
                    "scope={} | requested={} | selected_profile={} | selected_model={} | reason={} | context_window_chars={} | budget_tier={}",
                    selection.scope,
                    option_string(selection.requested_profile.clone()),
                    selection.selected_profile,
                    selection.selected_model,
                    selection.reason,
                    selection.context_window_chars,
                    selection.budget_tier,
                ))
                .collect::<Vec<_>>(),
        ));
    }

    if !trace.memory_reads.is_empty() {
        blocks.push(render_list_block(
            "memory reads",
            trace
                .memory_reads
                .iter()
                .map(|read| {
                    format!(
                        "session={} | source={} | tags={} | preview={}",
                        read.session_id,
                        read.source,
                        tags_or_none(&read.tags),
                        truncate(&read.preview, 160),
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }

    if let Some(compression) = &trace.compression {
        blocks.push(render_key_value_block(
            "compression",
            vec![
                (
                    "original_message_count",
                    compression.original_message_count.to_string(),
                ),
                (
                    "kept_recent_count",
                    compression.kept_recent_count.to_string(),
                ),
                (
                    "summary_preview",
                    truncate(&compression.summary_preview, 160),
                ),
            ],
        ));
    }

    if !trace.capability_invocations.is_empty() {
        blocks.push(render_list_block(
            "capability invocations",
            trace
                .capability_invocations
                .iter()
                .map(|invocation| format!(
                    "job_id={} | tool={} | kind={} | risk={} | status={} | scopes={} | target={} | node_id={} | route={} | duration_ms={} | error={} | summary={}",
                    invocation.job_id,
                    invocation.tool_name,
                    invocation.kind.label(),
                    invocation.risk.label(),
                    invocation.status,
                    if invocation.permission_scopes.is_empty() {
                        "<none>".to_owned()
                    } else {
                        invocation
                            .permission_scopes
                            .iter()
                            .map(|scope| scope.label().to_owned())
                            .collect::<Vec<_>>()
                            .join(", ")
                    },
                    option_string(invocation.target.clone()),
                    option_string(invocation.node_id.clone()),
                    option_string(invocation.capability_route.clone()),
                    option_i64(invocation.duration_ms()),
                    option_string(invocation.error.clone()),
                    truncate(&invocation.summary, 120),
                ))
                .collect::<Vec<_>>(),
        ));
    }

    if !trace.memory_writes.is_empty() {
        blocks.push(render_list_block(
            "memory writes",
            trace
                .memory_writes
                .iter()
                .map(|write| {
                    format!(
                        "session={} | kind={} | tags={} | preview={}",
                        write.session_id,
                        write.kind,
                        tags_or_none(&write.tags),
                        truncate(&write.preview, 160),
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }

    if let Some(workspace) = workspace {
        blocks.push(render_redacted_config(
            workspace,
            &ValidationReport::default(),
            None,
        ));
    }

    if !trace.tool_calls.is_empty() {
        let mut lines = Vec::new();
        for call in &trace.tool_calls {
            let input = serde_json::to_string_pretty(&call.input)?;
            lines.push(format!(
                "call_id={} | name={} | source={} | server={} | remote_tool={} | node_id={} | route={} | duration_ms={} | input={} | output_preview={}",
                option_string(call.call_id.clone()),
                call.name,
                call.source.label(),
                option_str(call.source.server_name()),
                option_str(call.source.remote_tool_name()),
                option_string(call.node_id.clone()),
                option_string(call.capability_route.clone()),
                option_i64(call.duration_ms()),
                single_line(&input),
                option_preview(call.output.as_deref(), 120),
            ));
        }
        blocks.push(render_list_block("tool calls", lines));
    }

    if !trace.skill_calls.is_empty() {
        let mut lines = Vec::new();
        for call in &trace.skill_calls {
            let input = serde_json::to_string_pretty(&call.input)?;
            lines.push(format!(
                "name={} | duration_ms={} | input={} | output_preview={}",
                call.name,
                option_i64(call.duration_ms()),
                single_line(&input),
                option_preview(call.output.as_deref(), 120),
            ));
        }
        blocks.push(render_list_block("skill calls", lines));
    }

    if !trace.step_traces.is_empty() {
        blocks.push(render_list_block(
            "workflow steps",
            trace
                .step_traces
                .iter()
                .map(|step| format!(
                    "name={} | kind={} | status={} | duration_ms={} | input_preview={} | output_preview={} | error={}",
                    step.name,
                    step.kind,
                    step.status(),
                    option_i64(step.duration_ms()),
                    truncate(&step.input, 120),
                    option_preview(step.output.as_deref(), 120),
                    option_preview(step.error.as_deref(), 120),
                ))
                .collect::<Vec<_>>(),
        ));
    }

    Ok(join_blocks(blocks))
}

fn render_redacted_config(
    redacted: &RedactedMosaicConfig,
    validation: &ValidationReport,
    workspace_path: Option<&std::path::Path>,
) -> String {
    let onboarding = derive_onboarding(redacted, validation);
    let mut blocks = vec![render_key_value_block(
        "config summary",
        vec![
            ("schema_version", redacted.schema_version.to_string()),
            ("active_profile", redacted.active_profile.clone()),
            (
                "active_profile_usage",
                onboarding.active_profile_usage.to_owned(),
            ),
            ("onboarding_state", onboarding.state.to_owned()),
            ("profile_count", redacted.profiles.len().to_string()),
            (
                "validation_errors",
                validation_error_count(validation).to_string(),
            ),
            (
                "validation_warnings",
                validation_warning_count(validation).to_string(),
            ),
            (
                "workspace_config",
                workspace_path
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "<not-loaded>".to_owned()),
            ),
        ],
    )];

    blocks.push(render_key_value_block(
        "onboarding",
        vec![
            ("state", onboarding.state.to_owned()),
            (
                "active_profile_usage",
                onboarding.active_profile_usage.to_owned(),
            ),
            ("message", onboarding.message),
        ],
    ));

    blocks.push(render_key_value_block(
        "deployment",
        vec![
            ("profile", redacted.deployment.profile.clone()),
            ("workspace_name", redacted.deployment.workspace_name.clone()),
        ],
    ));
    blocks.push(render_key_value_block(
        "storage",
        vec![
            (
                "session_store_root_dir",
                redacted.session_store_root_dir.clone(),
            ),
            ("inspect_runs_dir", redacted.inspect_runs_dir.clone()),
            ("audit_root_dir", redacted.audit.root_dir.clone()),
            (
                "audit_retention_days",
                redacted.audit.retention_days.to_string(),
            ),
            (
                "event_replay_window",
                redacted.audit.event_replay_window.to_string(),
            ),
            ("redact_inputs", redacted.audit.redact_inputs.to_string()),
        ],
    ));
    blocks.push(render_key_value_block(
        "auth",
        vec![
            (
                "operator_token_env",
                option_string(redacted.auth.operator_token_env.clone()),
            ),
            (
                "operator_token_present",
                redacted.auth.operator_token_present.to_string(),
            ),
            (
                "webchat_secret_env",
                option_string(redacted.auth.webchat_shared_secret_env.clone()),
            ),
            (
                "webchat_secret_present",
                redacted.auth.webchat_shared_secret_present.to_string(),
            ),
            (
                "telegram_secret_env",
                option_string(redacted.auth.telegram_secret_token_env.clone()),
            ),
            (
                "telegram_secret_present",
                redacted.auth.telegram_secret_token_present.to_string(),
            ),
        ],
    ));
    blocks.push(render_key_value_block(
        "observability",
        vec![
            ("metrics", redacted.observability.enable_metrics.to_string()),
            (
                "readiness",
                redacted.observability.enable_readiness.to_string(),
            ),
            (
                "slow_consumer_lag_threshold",
                redacted
                    .observability
                    .slow_consumer_lag_threshold
                    .to_string(),
            ),
        ],
    ));
    blocks.push(render_key_value_block(
        "policies",
        vec![
            ("allow_exec", redacted.policies.allow_exec.to_string()),
            ("allow_webhook", redacted.policies.allow_webhook.to_string()),
            ("allow_cron", redacted.policies.allow_cron.to_string()),
            ("allow_mcp", redacted.policies.allow_mcp.to_string()),
            (
                "hot_reload_enabled",
                redacted.policies.hot_reload_enabled.to_string(),
            ),
            (
                "extension_manifest_count",
                redacted.extension_manifest_count.to_string(),
            ),
        ],
    ));
    blocks.push(render_key_value_block(
        "provider defaults",
        vec![
            (
                "timeout_ms",
                option_u64(redacted.provider_defaults.timeout_ms),
            ),
            (
                "max_retries",
                option_u8(redacted.provider_defaults.max_retries),
            ),
            (
                "retry_backoff_ms",
                option_u64(redacted.provider_defaults.retry_backoff_ms),
            ),
        ],
    ));
    blocks.push(render_key_value_block(
        "runtime policy",
        vec![
            (
                "max_provider_round_trips",
                redacted.runtime.max_provider_round_trips.to_string(),
            ),
            (
                "max_workflow_provider_round_trips",
                redacted
                    .runtime
                    .max_workflow_provider_round_trips
                    .to_string(),
            ),
            (
                "continue_after_tool_error",
                redacted.runtime.continue_after_tool_error.to_string(),
            ),
        ],
    ));
    blocks.push(render_list_block(
        "profiles",
        redacted
            .profiles
            .iter()
            .map(|profile| {
                format!(
                    "{} | usage={} | type={} | model={} | base_url={} | api_key_env={} | api_key_present={} | timeout_ms={} | max_retries={} | retry_backoff_ms={} | allow_custom_headers={} | custom_headers={} | azure_api_version={} | anthropic_version={}",
                    profile.name,
                    profile.usage.label(),
                    profile.provider_type,
                    profile.model,
                    option_string(profile.base_url.clone()),
                    option_string(profile.api_key_env.clone()),
                    profile.api_key_present,
                    option_u64(profile.timeout_ms),
                    option_u8(profile.max_retries),
                    option_u64(profile.retry_backoff_ms),
                    profile.allow_custom_headers,
                    if profile.custom_header_keys.is_empty() {
                        "<none>".to_owned()
                    } else {
                        profile.custom_header_keys.join(", ")
                    },
                    option_string(profile.azure_api_version.clone()),
                    option_string(profile.anthropic_version.clone()),
                )
            })
            .collect::<Vec<_>>(),
    ));

    join_blocks(blocks)
}

fn render_key_value_block(title: &str, rows: Vec<(&str, String)>) -> String {
    let mut out = format!("{}:\n", title);
    for (label, value) in rows {
        out.push_str(&format!("  {}: {}\n", label, value));
    }
    out.trim_end().to_owned()
}

fn render_list_block(title: &str, rows: Vec<String>) -> String {
    let mut out = format!("{}:\n", title);
    if rows.is_empty() {
        out.push_str("  <none>");
    } else {
        for row in rows {
            out.push_str(&format!("  - {}\n", row));
        }
    }
    out.trim_end().to_owned()
}

fn join_blocks<I>(blocks: I) -> String
where
    I: IntoIterator<Item = String>,
{
    blocks
        .into_iter()
        .filter(|block| !block.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn option_string(value: Option<String>) -> String {
    value.unwrap_or_else(|| "<none>".to_owned())
}

fn derive_onboarding(
    redacted: &RedactedMosaicConfig,
    validation: &ValidationReport,
) -> OnboardingView {
    let Some(active_profile) = redacted
        .profiles
        .iter()
        .find(|profile| profile.name == redacted.active_profile)
    else {
        return OnboardingView {
            state: "invalid",
            active_profile_usage: "<unknown>",
            message: format!(
                "active profile '{}' does not resolve to a configured provider profile",
                redacted.active_profile
            ),
        };
    };

    if active_profile.usage == ProviderUsage::DevOnlyMock {
        return OnboardingView {
            state: "dev-mock",
            active_profile_usage: active_profile.usage.label(),
            message: format!(
                "active profile '{}' is dev-only mock; use it for local smoke tests, not onboarding or release evidence",
                active_profile.name
            ),
        };
    }

    if let Some(issue) = validation.issues.iter().find(|issue| {
        issue.field == "active_profile"
            || issue
                .field
                .starts_with(&format!("profiles.{}.", active_profile.name))
    }) {
        return OnboardingView {
            state: "pending-provider-configuration",
            active_profile_usage: active_profile.usage.label(),
            message: issue.message.clone(),
        };
    }

    if let Some(api_key_env) = active_profile.api_key_env.as_deref() {
        if !active_profile.api_key_present {
            return OnboardingView {
                state: "pending-provider-credentials",
                active_profile_usage: active_profile.usage.label(),
                message: format!(
                    "active real profile '{}' expects {} to be set before runs can use the configured provider",
                    active_profile.name, api_key_env
                ),
            };
        }
    }

    OnboardingView {
        state: "ready",
        active_profile_usage: active_profile.usage.label(),
        message: format!(
            "active profile '{}' is ready for real provider runs",
            active_profile.name
        ),
    }
}

fn option_str(value: Option<&str>) -> String {
    value.unwrap_or("<none>").to_owned()
}

fn option_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<none>".to_owned())
}

fn option_u16(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<none>".to_owned())
}

fn option_u8(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<none>".to_owned())
}

fn option_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<none>".to_owned())
}

fn option_datetime(value: Option<chrono::DateTime<chrono::Utc>>) -> String {
    value
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "<none>".to_owned())
}

fn option_preview(value: Option<&str>, limit: usize) -> String {
    value
        .map(|value| truncate(value, limit))
        .unwrap_or_else(|| "<none>".to_owned())
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }

    let truncated: String = value.chars().take(limit).collect();
    format!("{}...", truncated)
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tags_or_none(tags: &[String]) -> String {
    if tags.is_empty() {
        "<none>".to_owned()
    } else {
        tags.join(", ")
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn config_source_label(kind: &mosaic_config::ConfigSourceKind) -> &'static str {
    match kind {
        mosaic_config::ConfigSourceKind::Defaults => "defaults",
        mosaic_config::ConfigSourceKind::User => "user",
        mosaic_config::ConfigSourceKind::Workspace => "workspace",
        mosaic_config::ConfigSourceKind::Env => "env",
        mosaic_config::ConfigSourceKind::Cli => "cli",
    }
}

fn validation_level_label(level: &mosaic_config::ValidationLevel) -> &'static str {
    match level {
        mosaic_config::ValidationLevel::Error => "error",
        mosaic_config::ValidationLevel::Warning => "warning",
    }
}

fn doctor_status_label(status: &mosaic_config::DoctorStatus) -> &'static str {
    match status {
        mosaic_config::DoctorStatus::Ok => "ok",
        mosaic_config::DoctorStatus::Warning => "warning",
        mosaic_config::DoctorStatus::Error => "error",
    }
}

fn validation_error_count(report: &ValidationReport) -> usize {
    report.errors().len()
}

fn validation_warning_count(report: &ValidationReport) -> usize {
    report.warnings().len()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use mosaic_config::{
        DoctorCategory, DoctorCheck, DoctorReport, DoctorStatus, LoadConfigOptions, ProviderUsage,
        RedactedMosaicConfig, ValidationLevel, ValidationReport, load_mosaic_config,
    };
    use mosaic_inspect::{RunTrace, ToolTrace};
    use mosaic_runtime::events::RunEvent;
    use mosaic_tool_core::ToolSource;

    use super::{
        format_run_event, render_config_show, render_doctor_report, render_inspect_report,
    };

    #[test]
    fn formats_provider_requests_with_stable_field_order() {
        let line = format_run_event(&RunEvent::ProviderRequest {
            provider_type: "openai".to_owned(),
            profile: "gpt-5.4".to_owned(),
            model: "gpt-5.4".to_owned(),
            tool_count: 2,
            message_count: 3,
            max_attempts: 3,
        });

        assert_eq!(
            line,
            "[run] provider=request provider=openai profile=gpt-5.4 model=gpt-5.4 tools=2 messages=3 attempts=3"
        );
    }

    #[test]
    fn formats_provider_retries_with_attempt_metadata() {
        let line = format_run_event(&RunEvent::ProviderRetry {
            provider_type: "openai".to_owned(),
            profile: "gpt-5.4".to_owned(),
            model: "gpt-5.4".to_owned(),
            attempt: 1,
            max_attempts: 3,
            kind: "timeout".to_owned(),
            status_code: Some(504),
            retryable: true,
            error: "timed out".to_owned(),
        });

        assert_eq!(
            line,
            "[run] provider retry: provider=openai profile=gpt-5.4 model=gpt-5.4 attempt=1/3 kind=timeout status=Some(504) error=timed out"
        );
    }

    #[test]
    fn formats_workflow_step_failures_with_workflow_prefix() {
        let line = format_run_event(&RunEvent::WorkflowStepFailed {
            workflow: "research_brief".to_owned(),
            step: "draft".to_owned(),
            error: "provider failure".to_owned(),
        });

        assert_eq!(
            line,
            "[run] workflow step failed: research_brief.draft error=provider failure"
        );
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
            run_id: "run-1".to_owned(),
            error: "provider failure".to_owned(),
            failure_kind: Some("provider".to_owned()),
        });

        assert_eq!(
            line,
            "[run] failed run_id=run-1 kind=provider error=provider failure"
        );
    }

    #[test]
    fn renders_config_show_with_summary_first() {
        let loaded = load_mosaic_config(&LoadConfigOptions::default()).expect("config should load");
        let rendered = render_config_show(&loaded, &ValidationReport::default());

        assert!(rendered.starts_with("config summary:"));
        assert!(rendered.contains("onboarding:"));
        assert!(rendered.contains("deployment:"));
        assert!(rendered.contains("profiles:"));
    }

    #[test]
    fn renders_doctor_report_with_categories() {
        let report = DoctorReport {
            validation: ValidationReport {
                issues: vec![mosaic_config::ValidationIssue {
                    level: ValidationLevel::Warning,
                    field: "profiles.demo".to_owned(),
                    message: "demo warning".to_owned(),
                }],
            },
            checks: vec![
                DoctorCheck {
                    status: DoctorStatus::Warning,
                    category: DoctorCategory::Auth,
                    message: "operator token missing".to_owned(),
                },
                DoctorCheck {
                    status: DoctorStatus::Ok,
                    category: DoctorCategory::Storage,
                    message: "session store ready".to_owned(),
                },
            ],
        };

        let rendered = render_doctor_report(
            &report,
            &RedactedMosaicConfig {
                schema_version: 1,
                active_profile: "mock".to_owned(),
                profiles: vec![mosaic_config::RedactedProfileView {
                    name: "mock".to_owned(),
                    provider_type: "mock".to_owned(),
                    usage: ProviderUsage::DevOnlyMock,
                    model: "mock".to_owned(),
                    base_url: None,
                    api_key_env: None,
                    api_key_present: false,
                    timeout_ms: Some(0),
                    max_retries: Some(0),
                    retry_backoff_ms: Some(0),
                    custom_header_keys: Vec::new(),
                    allow_custom_headers: false,
                    azure_api_version: None,
                    anthropic_version: None,
                }],
                provider_defaults: mosaic_config::RedactedProviderDefaultsView {
                    timeout_ms: None,
                    max_retries: None,
                    retry_backoff_ms: None,
                },
                deployment: mosaic_config::RedactedDeploymentView {
                    profile: "local".to_owned(),
                    workspace_name: "default".to_owned(),
                },
                auth: mosaic_config::RedactedAuthView {
                    operator_token_env: None,
                    operator_token_present: false,
                    webchat_shared_secret_env: None,
                    webchat_shared_secret_present: false,
                    telegram_secret_token_env: None,
                    telegram_secret_token_present: false,
                },
                session_store_root_dir: ".mosaic/sessions".to_owned(),
                inspect_runs_dir: ".mosaic/runs".to_owned(),
                audit: mosaic_config::RedactedAuditView {
                    root_dir: ".mosaic/audit".to_owned(),
                    retention_days: 14,
                    event_replay_window: 256,
                    redact_inputs: true,
                },
                observability: mosaic_config::RedactedObservabilityView {
                    enable_metrics: true,
                    enable_readiness: true,
                    slow_consumer_lag_threshold: 128,
                },
                runtime: mosaic_config::RedactedRuntimePolicyView {
                    max_provider_round_trips: 8,
                    max_workflow_provider_round_trips: 8,
                    continue_after_tool_error: false,
                },
                attachments: mosaic_config::RedactedAttachmentView {
                    enabled: true,
                    cache_dir: ".mosaic/attachments".to_owned(),
                    max_size_bytes: 10 * 1024 * 1024,
                    download_timeout_ms: 15_000,
                    cleanup_after_hours: 24,
                    allowed_mime_types: vec![
                        "image/".to_owned(),
                        "text/".to_owned(),
                        "application/pdf".to_owned(),
                    ],
                    default_route_mode: mosaic_config::AttachmentRouteModeConfig::ProviderNative,
                },
                extension_manifest_count: 0,
                policies: mosaic_config::RedactedPolicyView {
                    allow_exec: false,
                    allow_webhook: true,
                    allow_cron: true,
                    allow_mcp: true,
                    hot_reload_enabled: false,
                },
            },
        );
        assert!(rendered.starts_with("doctor summary:"));
        assert!(rendered.contains("onboarding:"));
        assert!(rendered.contains("doctor categories:"));
        assert!(rendered.contains("auth | ok=0 warning=1 error=0"));
        assert!(rendered.contains("[warning] auth: operator token missing"));
    }

    #[test]
    fn renders_inspect_summary_and_verbose_views() {
        let started_at = Utc::now();
        let finished_at = started_at + Duration::milliseconds(12);
        let trace = RunTrace {
            run_id: "run-1".to_owned(),
            gateway_run_id: Some("gateway-1".to_owned()),
            correlation_id: Some("corr-1".to_owned()),
            session_id: Some("session-1".to_owned()),
            session_route: Some("gateway.local/session-1".to_owned()),
            ingress: None,
            route_decision: None,
            outbound_deliveries: vec![],
            workflow_name: Some("research_brief".to_owned()),
            started_at,
            finished_at: Some(finished_at),
            input: "hello".to_owned(),
            output: Some("world".to_owned()),
            effective_profile: None,
            runtime_policy: Some(mosaic_inspect::RuntimePolicyTrace {
                max_provider_round_trips: 8,
                max_workflow_provider_round_trips: 8,
                continue_after_tool_error: false,
            }),
            lifecycle_status: mosaic_inspect::RunLifecycleStatus::Success,
            failure: None,
            output_chunks: 1,
            integrity_warnings: Vec::new(),
            provider_failure: None,
            provider_attempts: vec![],
            governance: None,
            model_selections: vec![],
            memory_reads: vec![],
            memory_writes: vec![],
            compression: None,
            attachment_route: None,
            tool_calls: vec![ToolTrace {
                call_id: Some("call-1".to_owned()),
                name: "echo".to_owned(),
                source: ToolSource::Builtin,
                input: serde_json::json!({"text": "hello"}),
                output: Some("hello".to_owned()),
                node_id: None,
                capability_route: None,
                disconnect_context: None,
                started_at,
                finished_at: Some(finished_at),
            }],
            capability_invocations: vec![],
            side_effect_summary: None,
            active_extensions: vec![],
            used_extensions: vec![],
            skill_calls: vec![],
            step_traces: vec![],
            error: None,
        };

        let summary =
            render_inspect_report(&trace, None, false).expect("summary render should work");
        let verbose =
            render_inspect_report(&trace, None, true).expect("verbose render should work");

        assert!(summary.starts_with("run summary:"));
        assert!(summary.contains("run activity:"));
        assert!(summary.contains("runtime policy:"));
        assert!(!summary.contains("tool calls:"));
        assert!(verbose.contains("tool calls:"));
    }
}
