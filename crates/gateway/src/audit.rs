use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use mosaic_control_protocol::{GatewayAuditEventDto, IncidentBundleDto, ReplayWindowResponse};

use super::*;

impl GatewayHandle {
    pub fn replay_window(&self, limit: usize) -> ReplayWindowResponse {
        self.inner
            .replay_window
            .lock()
            .expect("replay window lock should not be poisoned")
            .snapshot(limit)
    }

    pub fn audit_events(&self, limit: usize) -> Vec<GatewayAuditEventDto> {
        self.inner.audit_log.recent(limit)
    }

    pub fn incident_bundle(&self, identifier: &str) -> Result<(IncidentBundleDto, PathBuf)> {
        let components = self.snapshot_components();
        let trace = load_incident_trace(&components.runs_dir, identifier)?;
        let bundle = IncidentBundleDto {
            identifier: identifier.to_owned(),
            generated_at: Utc::now(),
            deployment_profile: components.deployment.profile.clone(),
            auth_mode: operator_auth_mode(&components.auth),
            redaction_policy: if components.audit.redact_inputs {
                "inputs_redacted".to_owned()
            } else {
                "full_inputs".to_owned()
            },
            audit_events: self.inner.audit_log.incident_events_for(&trace),
            metrics: self.metrics(),
            run: self.load_run(identifier)?,
            trace,
        };
        let path = self.inner.audit_log.save_incident_bundle(&bundle)?;
        Ok((bundle, path))
    }

    pub(crate) fn record_audit_event(
        &self,
        kind: &str,
        outcome: &str,
        summary: String,
        session_id: Option<String>,
        gateway_run_id: Option<String>,
        correlation_id: Option<String>,
        ingress: Option<&IngressTrace>,
        target: Option<String>,
        redacted: bool,
    ) {
        record_audit_event(
            self.inner.as_ref(),
            kind,
            outcome,
            summary,
            session_id,
            gateway_run_id,
            correlation_id,
            ingress,
            target,
            redacted,
        );
    }
}
