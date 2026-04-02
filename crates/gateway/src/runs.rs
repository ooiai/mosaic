use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use mosaic_control_protocol::{IngressTrace, RunDetailDto, RunSummaryDto};
use mosaic_inspect::RunLifecycleStatus;
use mosaic_runtime::{AgentRuntime, RunRequest};
use tokio::sync::watch;
use uuid::Uuid;

use super::*;

impl GatewayHandle {
    pub fn submit_command(&self, command: GatewayCommand) -> Result<GatewaySubmittedRun> {
        match command {
            GatewayCommand::SubmitRun(request) => self.submit_run(request),
        }
    }

    pub fn list_runs(&self) -> Result<Vec<RunSummaryDto>> {
        Ok(self
            .inner
            .run_store
            .list()?
            .into_iter()
            .map(|record| record.summary_dto())
            .collect())
    }

    pub fn load_run(&self, identifier: &str) -> Result<Option<RunDetailDto>> {
        Ok(self
            .inner
            .run_store
            .resolve(identifier)?
            .map(|record| record.detail_dto()))
    }

    pub fn cancel_run(&self, identifier: &str) -> Result<RunDetailDto> {
        let mut record = self
            .inner
            .run_store
            .resolve(identifier)?
            .ok_or_else(|| anyhow!("run not found: {identifier}"))?;
        if record.status.is_terminal() {
            bail!("run is already terminal: {}", record.gateway_run_id);
        }

        let handle = self
            .inner
            .active_runs
            .lock()
            .expect("active run lock should not be poisoned")
            .get(&record.gateway_run_id)
            .cloned()
            .ok_or_else(|| anyhow!("run is not active: {}", record.gateway_run_id))?;
        let _ = handle.cancel.send(true);
        record.set_status(RunLifecycleStatus::CancelRequested);
        record.set_error(
            Some("cancel requested by operator".to_owned()),
            Some("canceled".to_owned()),
            Some("gateway".to_owned()),
        );
        self.inner.run_store.save(&record)?;
        sync_session_run_state(
            self.inner.as_ref(),
            &record,
            record.status,
            Some(record.run_id.clone()),
            record.error.clone(),
            record.failure_kind.clone(),
        );
        self.record_audit_event(
            "run.cancel_requested",
            "accepted",
            format!("cancel requested for {}", record.gateway_run_id),
            record.session_id.clone(),
            Some(record.gateway_run_id.clone()),
            Some(record.correlation_id.clone()),
            record.ingress.as_ref(),
            record.trace_path.clone(),
            false,
        );
        self.emit(run_record_envelope(&record));
        Ok(record.detail_dto())
    }

    pub fn retry_run(&self, identifier: &str) -> Result<GatewaySubmittedRun> {
        let record = self
            .inner
            .run_store
            .resolve(identifier)?
            .ok_or_else(|| anyhow!("run not found: {identifier}"))?;
        if !record.status.is_terminal() {
            bail!("run is still active: {}", record.gateway_run_id);
        }
        self.submit_run_with_retry_of(record.submission.clone(), Some(record.gateway_run_id))
    }

    pub fn submit_run(&self, request: GatewayRunRequest) -> Result<GatewaySubmittedRun> {
        self.submit_run_with_retry_of(request, None)
    }

    fn submit_run_with_retry_of(
        &self,
        request: GatewayRunRequest,
        retry_of: Option<String>,
    ) -> Result<GatewaySubmittedRun> {
        let gateway_run_id = Uuid::new_v4().to_string();
        let correlation_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let components = self.snapshot_components();
        let session_route =
            self.resolve_session_route(request.session_id.as_deref(), request.ingress.as_ref())?;
        let resolved_profile = request
            .profile
            .clone()
            .unwrap_or_else(|| components.profiles.active_profile_name().to_owned());
        let meta = GatewayRunMeta {
            gateway_run_id: gateway_run_id.clone(),
            correlation_id: correlation_id.clone(),
            run_id: run_id.clone(),
            session_id: request.session_id.clone(),
            session_route: session_route.clone(),
            ingress: request.ingress.clone(),
        };
        let record =
            StoredRunRecord::new(&meta, &request, Some(resolved_profile.clone()), retry_of);
        self.inner.run_store.save(&record)?;
        sync_session_run_state(
            self.inner.as_ref(),
            &record,
            RunLifecycleStatus::Queued,
            Some(run_id.clone()),
            None,
            None,
        );

        self.record_audit_event(
            "run.submitted",
            "accepted",
            redact_audit_input(&request.input, components.audit.redact_inputs),
            request.session_id.clone(),
            Some(gateway_run_id.clone()),
            Some(correlation_id.clone()),
            request.ingress.as_ref(),
            Some(session_route.clone()),
            components.audit.redact_inputs,
        );
        self.emit(run_record_envelope(&record));
        if let Some(ingress) = request.ingress.clone() {
            self.record_audit_event(
                "channel.inbound_received",
                "accepted",
                redact_audit_input(&request.input, components.audit.redact_inputs),
                request.session_id.clone(),
                Some(gateway_run_id.clone()),
                Some(correlation_id.clone()),
                Some(&ingress),
                ingress
                    .conversation_id
                    .clone()
                    .or_else(|| ingress.reply_target.clone()),
                components.audit.redact_inputs,
            );
            self.emit(meta.envelope(GatewayEvent::InboundReceived {
                ingress,
                text_preview: truncate_preview(&request.input, 120),
            }));
        }
        self.emit(meta.envelope(GatewayEvent::RunSubmitted {
            input: request.input.clone(),
            profile: resolved_profile,
            ingress: request.ingress.clone(),
        }));

        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.inner
            .active_runs
            .lock()
            .expect("active run lock should not be poisoned")
            .insert(
                gateway_run_id.clone(),
                ActiveRunHandle { cancel: cancel_tx },
            );

        let state = self.inner.clone();
        let session_id_for_handle = request.session_id.clone();
        let request_for_task = request.clone();
        let join = self.inner.runtime_handle.spawn(async move {
            let event_sink: SharedRunEventSink = Arc::new(GatewayRunEventSink {
                state: state.clone(),
                meta: meta.clone(),
            });
            let components = state
                .components
                .lock()
                .expect("gateway components lock should not be poisoned")
                .clone();
            let prepared_request =
                crate::attachments::prepare_submission_attachments(&components, request_for_task)
                    .await;
            let mut task_meta = meta.clone();
            task_meta.ingress = prepared_request.ingress.clone();
            let runtime = AgentRuntime::new(components.runtime_context(event_sink));
            let run_request = RunRequest {
                run_id: Some(task_meta.run_id.clone()),
                system: prepared_request.system.clone(),
                input: prepared_request.input.clone(),
                tool: prepared_request.tool.clone(),
                skill: prepared_request.skill.clone(),
                workflow: prepared_request.workflow.clone(),
                session_id: task_meta.session_id.clone(),
                profile: prepared_request.profile.clone(),
                ingress: task_meta.ingress.clone(),
            };

            let outcome = tokio::select! {
                _ = wait_for_cancellation(cancel_rx) => finalize_canceled(state.clone(), task_meta.clone(), &prepared_request).await,
                result = runtime.run(run_request) => finalize_run(state.clone(), task_meta.clone(), result).await,
            };
            if let Some(record) = update_run_record(state.as_ref(), &task_meta.gateway_run_id, |record| {
                record.ingress = task_meta.ingress.clone();
                record.submission = prepared_request.clone();
            }) {
                broadcast_envelope(state.as_ref(), run_record_envelope(&record));
            }
            state
                .active_runs
                .lock()
                .expect("active run lock should not be poisoned")
                .remove(&task_meta.gateway_run_id);
            outcome
        });

        Ok(GatewaySubmittedRun {
            gateway_run_id,
            correlation_id,
            session_id: session_id_for_handle,
            session_route,
            join,
        })
    }

    pub(crate) fn submit_control_response(
        &self,
        request: GatewayRunRequest,
        response_text: String,
        route_decision: RouteDecisionTrace,
        profile_override: Option<String>,
    ) -> Result<GatewaySubmittedRun> {
        let gateway_run_id = Uuid::new_v4().to_string();
        let correlation_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let components = self.snapshot_components();
        let session_route =
            self.resolve_session_route(request.session_id.as_deref(), request.ingress.as_ref())?;
        let resolved_profile = profile_override
            .clone()
            .or_else(|| request.profile.clone())
            .unwrap_or_else(|| components.profiles.active_profile_name().to_owned());
        let meta = GatewayRunMeta {
            gateway_run_id: gateway_run_id.clone(),
            correlation_id: correlation_id.clone(),
            run_id: run_id.clone(),
            session_id: request.session_id.clone(),
            session_route: session_route.clone(),
            ingress: request.ingress.clone(),
        };
        let record = StoredRunRecord::new(&meta, &request, Some(resolved_profile), None);
        self.inner.run_store.save(&record)?;
        sync_session_run_state(
            self.inner.as_ref(),
            &record,
            RunLifecycleStatus::Queued,
            Some(run_id),
            None,
            None,
        );

        self.record_audit_event(
            "run.submitted",
            "accepted",
            redact_audit_input(&request.input, components.audit.redact_inputs),
            request.session_id.clone(),
            Some(gateway_run_id.clone()),
            Some(correlation_id.clone()),
            request.ingress.as_ref(),
            Some(session_route.clone()),
            components.audit.redact_inputs,
        );
        self.emit(run_record_envelope(&record));
        if let Some(ingress) = request.ingress.clone() {
            self.record_audit_event(
                "channel.inbound_received",
                "accepted",
                redact_audit_input(&request.input, components.audit.redact_inputs),
                request.session_id.clone(),
                Some(gateway_run_id.clone()),
                Some(correlation_id.clone()),
                Some(&ingress),
                ingress
                    .conversation_id
                    .clone()
                    .or_else(|| ingress.reply_target.clone()),
                components.audit.redact_inputs,
            );
            self.emit(meta.envelope(GatewayEvent::InboundReceived {
                ingress,
                text_preview: truncate_preview(&request.input, 120),
            }));
        }
        self.emit(
            meta.envelope(GatewayEvent::RunSubmitted {
                input: request.input.clone(),
                profile: request
                    .profile
                    .clone()
                    .unwrap_or_else(|| components.profiles.active_profile_name().to_owned()),
                ingress: request.ingress.clone(),
            }),
        );

        let state = self.inner.clone();
        let session_id = request.session_id.clone();
        let session_route = session_route.clone();
        let join = self.inner.runtime_handle.spawn(async move {
            finalize_control_response(
                state,
                meta,
                request,
                response_text,
                route_decision,
                profile_override,
            )
            .await
        });

        Ok(GatewaySubmittedRun {
            gateway_run_id,
            correlation_id,
            session_id,
            session_route,
            join,
        })
    }

    pub(crate) fn resolve_session_route(
        &self,
        session_id: Option<&str>,
        ingress: Option<&IngressTrace>,
    ) -> Result<String> {
        match session_id {
            Some(id) => {
                if let Some(session) = self.load_session(id)? {
                    let current = if session.gateway.route.is_empty() {
                        session_route_for_id(id)
                    } else {
                        session.gateway.route
                    };
                    if current != session_route_for_id(id) || ingress.is_none() {
                        return Ok(current);
                    }
                }

                Ok(ingress_route(Some(id), ingress).unwrap_or_else(|| session_route_for_id(id)))
            }
            None => Ok(ingress_route(None, ingress)
                .unwrap_or_else(|| "gateway.local/ephemeral".to_owned())),
        }
    }
}

async fn finalize_control_response(
    state: Arc<GatewayState>,
    meta: GatewayRunMeta,
    request: RunSubmission,
    response_text: String,
    mut route_decision: RouteDecisionTrace,
    profile_override: Option<String>,
) -> Result<GatewayRunResult, GatewayRunError> {
    let components = state.snapshot_components();
    let resolved_profile_name = profile_override
        .clone()
        .or_else(|| request.profile.clone())
        .unwrap_or_else(|| components.profiles.active_profile_name().to_owned());
    let resolved_profile = components
        .profiles
        .resolve(Some(&resolved_profile_name))
        .map_err(|source| GatewayRunError {
            source,
            trace: RunTrace::new_with_id(meta.run_id.clone(), request.input.clone()),
            trace_path: PathBuf::new(),
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            session_route: meta.session_route.clone(),
        })?;
    route_decision.profile_used = Some(resolved_profile.name.clone());

    let mut trace = RunTrace::new_with_id(meta.run_id.clone(), request.input.clone());
    if let Some(session_id) = meta.session_id.clone() {
        trace.bind_session(session_id);
    }
    if let Some(ingress) = meta.ingress.clone() {
        trace.bind_ingress(ingress);
    }
    trace.bind_route_decision(route_decision);
    trace.bind_extensions(components.extensions.iter().map(extension_trace).collect());
    trace.bind_gateway_context(
        meta.gateway_run_id.clone(),
        meta.correlation_id.clone(),
        meta.session_route.clone(),
    );
    trace.bind_governance(GovernanceTrace {
        deployment_profile: components.deployment.profile.clone(),
        workspace_name: components.deployment.workspace_name.clone(),
        auth_mode: operator_auth_mode(&components.auth),
        audit_retention_days: components.audit.retention_days,
        event_replay_window: components.audit.event_replay_window,
        redact_inputs: components.audit.redact_inputs,
    });
    trace.mark_running();

    if let Some(session_id) = meta.session_id.as_deref() {
        let mut session = match components.session_store.load(session_id) {
            Ok(Some(session)) => session,
            Ok(None) => SessionRecord::new(
                session_id,
                session_title_from_input(&request.input),
                resolved_profile.name.clone(),
                resolved_profile.provider_type.clone(),
                resolved_profile.model.clone(),
            ),
            Err(source) => {
                return Err(GatewayRunError {
                    source,
                    trace,
                    trace_path: PathBuf::new(),
                    gateway_run_id: meta.gateway_run_id.clone(),
                    correlation_id: meta.correlation_id.clone(),
                    session_route: meta.session_route.clone(),
                });
            }
        };
        session.set_runtime_binding(
            resolved_profile.name.clone(),
            resolved_profile.provider_type.clone(),
            resolved_profile.model.clone(),
        );
        session.set_last_run_id(meta.run_id.clone());
        if let Some(ingress) = meta.ingress.as_ref() {
            session.bind_ingress_context(ingress);
        }
        session.append_message(TranscriptRole::User, request.input.clone(), None);
        session.append_message(TranscriptRole::Assistant, response_text.clone(), None);
        if let Err(source) = components.session_store.save(&session) {
            return Err(GatewayRunError {
                source,
                trace,
                trace_path: PathBuf::new(),
                gateway_run_id: meta.gateway_run_id.clone(),
                correlation_id: meta.correlation_id.clone(),
                session_route: meta.session_route.clone(),
            });
        }
    }

    trace.record_output_chunk();
    trace.finish_ok(response_text.clone());
    let outbound_deliveries =
        dispatch_outbound_replies(state.as_ref(), &meta, &response_text).await;
    let last_delivery = outbound_deliveries.last().cloned();
    for delivery in outbound_deliveries {
        trace.add_outbound_delivery(delivery.clone());
        record_channel_delivery_outcome(state.as_ref(), &meta, &delivery);
    }

    let trace_path = trace
        .save_to_dir(&components.runs_dir)
        .map_err(|source| GatewayRunError {
            source,
            trace: trace.clone(),
            trace_path: PathBuf::new(),
            gateway_run_id: meta.gateway_run_id.clone(),
            correlation_id: meta.correlation_id.clone(),
            session_route: meta.session_route.clone(),
        })?;

    if let Some(record) = update_run_record(state.as_ref(), &meta.gateway_run_id, |record| {
        record.update_from_trace(&trace, Some(&trace_path));
    }) {
        sync_session_run_state(
            state.as_ref(),
            &record,
            RunLifecycleStatus::Success,
            Some(trace.run_id.clone()),
            None,
            None,
        );
        broadcast_envelope(state.as_ref(), run_record_envelope(&record));
    }
    let session_summary = update_gateway_session_metadata(
        &state,
        &meta,
        Some(trace.run_id.clone()),
        RunLifecycleStatus::Success,
        None,
        None,
        last_delivery.as_ref(),
    );
    increment_metric(state.as_ref(), |metrics| metrics.completed_runs_total += 1);
    record_audit_event(
        state.as_ref(),
        "run.completed",
        "success",
        truncate_preview(&response_text, 160),
        meta.session_id.clone(),
        Some(meta.gateway_run_id.clone()),
        Some(meta.correlation_id.clone()),
        meta.ingress.as_ref(),
        Some(trace_path.display().to_string()),
        false,
    );
    broadcast_envelope(
        state.as_ref(),
        meta.envelope(GatewayEvent::RunCompleted {
            output_preview: truncate_preview(&response_text, 120),
        }),
    );
    if let Some(summary) = session_summary.clone() {
        broadcast_envelope(
            state.as_ref(),
            meta.envelope(GatewayEvent::SessionUpdated {
                summary: session_summary_dto(&summary),
            }),
        );
    }

    Ok(GatewayRunResult {
        gateway_run_id: meta.gateway_run_id,
        correlation_id: meta.correlation_id,
        session_route: meta.session_route,
        output: response_text,
        trace,
        trace_path,
        session_summary,
    })
}
