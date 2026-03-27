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
            let runtime = AgentRuntime::new(components.runtime_context(event_sink));
            let run_request = RunRequest {
                run_id: Some(meta.run_id.clone()),
                system: request.system,
                input: request.input,
                skill: request.skill,
                workflow: request.workflow,
                session_id: meta.session_id.clone(),
                profile: request.profile,
                ingress: meta.ingress.clone(),
            };

            let outcome = tokio::select! {
                _ = wait_for_cancellation(cancel_rx) => finalize_canceled(state.clone(), meta.clone(), &request_for_task),
                result = runtime.run(run_request) => finalize_run(state.clone(), meta.clone(), result),
            };
            state
                .active_runs
                .lock()
                .expect("active run lock should not be poisoned")
                .remove(&meta.gateway_run_id);
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

    fn resolve_session_route(
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
