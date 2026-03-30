use super::*;

pub(crate) fn extension_trace(status: &ExtensionStatus) -> ExtensionTrace {
    ExtensionTrace {
        name: status.name.clone(),
        version: status.version.clone(),
        source: status.source.clone(),
        enabled: status.enabled,
        active: status.active,
        error: status.error.clone(),
    }
}

pub(crate) fn extension_status_dto(status: &ExtensionStatus) -> ExtensionStatusDto {
    ExtensionStatusDto {
        name: status.name.clone(),
        version: status.version.clone(),
        source: status.source.clone(),
        enabled: status.enabled,
        active: status.active,
        tools: status.tools.clone(),
        skills: status.skills.clone(),
        workflows: status.workflows.clone(),
        mcp_servers: status.mcp_servers.clone(),
        error: status.error.clone(),
    }
}

pub(crate) fn extension_policy_dto(policy: &PolicyConfig) -> ExtensionPolicyDto {
    ExtensionPolicyDto {
        allow_exec: policy.allow_exec,
        allow_webhook: policy.allow_webhook,
        allow_cron: policy.allow_cron,
        allow_mcp: policy.allow_mcp,
        hot_reload_enabled: policy.hot_reload_enabled,
    }
}

pub fn session_summary_dto(summary: &SessionSummary) -> SessionSummaryDto {
    SessionSummaryDto {
        id: summary.id.clone(),
        title: summary.title.clone(),
        updated_at: summary.updated_at,
        provider_profile: summary.provider_profile.clone(),
        provider_type: summary.provider_type.clone(),
        model: summary.model.clone(),
        session_route: summary.session_route.clone(),
        channel_context: session_channel_dto(&summary.channel_context),
        run: session_run_dto(&summary.run),
        last_gateway_run_id: summary.last_gateway_run_id.clone(),
        last_correlation_id: summary.last_correlation_id.clone(),
        message_count: summary.message_count,
        last_message_preview: summary.last_message_preview.clone(),
        memory_summary_preview: summary.memory_summary_preview.clone(),
        reference_count: summary.reference_count,
    }
}

pub(crate) fn session_run_dto(metadata: &mosaic_session_core::SessionRunMetadata) -> SessionRunDto {
    SessionRunDto {
        current_run_id: metadata.current_run_id.clone(),
        current_gateway_run_id: metadata.current_gateway_run_id.clone(),
        current_correlation_id: metadata.current_correlation_id.clone(),
        status: metadata.status,
        last_error: metadata.last_error.clone(),
        last_failure_kind: metadata.last_failure_kind.clone(),
        updated_at: metadata.updated_at,
    }
}

pub(crate) fn session_channel_dto(metadata: &SessionChannelMetadata) -> SessionChannelDto {
    SessionChannelDto {
        ingress_kind: metadata.ingress_kind.clone(),
        channel: metadata.channel.clone(),
        adapter: metadata.adapter.clone(),
        bot_name: metadata.bot_name.clone(),
        bot_route: metadata.bot_route.clone(),
        bot_profile: metadata.bot_profile.clone(),
        bot_token_env: metadata.bot_token_env.clone(),
        source: metadata.source.clone(),
        actor_id: metadata.actor_id.clone(),
        actor_name: metadata.actor_name.clone(),
        conversation_id: metadata.conversation_id.clone(),
        thread_id: metadata.thread_id.clone(),
        thread_title: metadata.thread_title.clone(),
        reply_target: metadata.reply_target.clone(),
        last_message_id: metadata.last_message_id.clone(),
        last_delivery_id: metadata.last_delivery_id.clone(),
        last_delivery_status: metadata.last_delivery_status.clone(),
        last_delivery_error: metadata.last_delivery_error.clone(),
        last_delivery_at: metadata.last_delivery_at,
    }
}

pub fn cron_registration_dto(registration: &CronRegistration) -> CronRegistrationDto {
    CronRegistrationDto {
        id: registration.id.clone(),
        schedule: registration.schedule.clone(),
        input: registration.input.clone(),
        session_id: registration.session_id.clone(),
        profile: registration.profile.clone(),
        skill: registration.skill.clone(),
        workflow: registration.workflow.clone(),
        created_at: registration.created_at,
        last_triggered_at: registration.last_triggered_at,
    }
}

pub fn session_detail_dto(session: &SessionRecord) -> SessionDetailDto {
    SessionDetailDto {
        id: session.id.clone(),
        title: session.title.clone(),
        created_at: session.created_at,
        updated_at: session.updated_at,
        provider_profile: session.provider_profile.clone(),
        provider_type: session.provider_type.clone(),
        model: session.model.clone(),
        last_run_id: session.last_run_id.clone(),
        channel_context: session_channel_dto(&session.channel_context),
        run: session_run_dto(&session.run),
        gateway: SessionGatewayDto {
            route: session.gateway.route.clone(),
            last_gateway_run_id: session.gateway.last_gateway_run_id.clone(),
            last_correlation_id: session.gateway.last_correlation_id.clone(),
        },
        memory_summary: session.memory.latest_summary.clone(),
        compressed_context: session.memory.compressed_context.clone(),
        references: session
            .references
            .iter()
            .map(|reference| mosaic_control_protocol::SessionReferenceDto {
                session_id: reference.session_id.clone(),
                reason: reference.reason.clone(),
                created_at: reference.created_at,
            })
            .collect(),
        transcript: session
            .transcript
            .iter()
            .map(|message| TranscriptMessageDto {
                role: match message.role {
                    TranscriptRole::System => TranscriptRoleDto::System,
                    TranscriptRole::User => TranscriptRoleDto::User,
                    TranscriptRole::Assistant => TranscriptRoleDto::Assistant,
                    TranscriptRole::Tool => TranscriptRoleDto::Tool,
                },
                content: message.content.clone(),
                tool_call_id: message.tool_call_id.clone(),
                created_at: message.created_at,
            })
            .collect(),
    }
}

pub fn run_response(result: GatewayRunResult) -> RunResponse {
    RunResponse {
        gateway_run_id: result.gateway_run_id,
        correlation_id: result.correlation_id,
        session_route: result.session_route,
        output: result.output,
        trace: result.trace,
        session_summary: result.session_summary.as_ref().map(session_summary_dto),
    }
}
