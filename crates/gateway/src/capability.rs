use super::*;

impl GatewayHandle {
    pub async fn run_exec_job(&self, request: ExecJobRequest) -> Result<CapabilityJobDto> {
        execute_capability_tool(
            self.inner.clone(),
            "exec_command",
            request.session_id,
            serde_json::json!({
                "command": request.command,
                "args": request.args,
                "cwd": request.cwd,
            }),
        )
        .await
    }

    pub async fn run_webhook_job(&self, request: WebhookJobRequest) -> Result<CapabilityJobDto> {
        execute_capability_tool(
            self.inner.clone(),
            "webhook_call",
            request.session_id,
            serde_json::json!({
                "url": request.url,
                "method": request.method,
                "body": request.body,
                "headers": request.headers,
            }),
        )
        .await
    }
}

pub(crate) fn update_runtime_capability_job(
    jobs: &Arc<Mutex<BTreeMap<String, CapabilityJobDto>>>,
    meta: &GatewayRunMeta,
    event: &RunEvent,
) -> Option<CapabilityJobDto> {
    let mut jobs = jobs
        .lock()
        .expect("capability jobs lock should not be poisoned");

    match event {
        RunEvent::CapabilityJobQueued {
            job_id,
            name,
            kind,
            risk,
            permission_scopes,
        } => {
            let job = CapabilityJobDto {
                id: job_id.clone(),
                name: name.clone(),
                kind: kind.clone(),
                risk: risk.clone(),
                permission_scopes: permission_scopes.clone(),
                status: "queued".to_owned(),
                summary: None,
                target: None,
                session_id: meta.session_id.clone(),
                gateway_run_id: Some(meta.gateway_run_id.clone()),
                correlation_id: Some(meta.correlation_id.clone()),
                started_at: Utc::now(),
                finished_at: None,
                error: None,
            };
            jobs.insert(job_id.clone(), job.clone());
            Some(job)
        }
        RunEvent::CapabilityJobStarted { job_id, name } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: "running".to_owned(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = "running".to_owned();
            job.error = None;
            Some(job.clone())
        }
        RunEvent::CapabilityJobRetried {
            job_id,
            name,
            attempt,
            error,
        } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: "retrying".to_owned(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = "retrying".to_owned();
            job.summary = Some(format!("retry attempt {}", attempt));
            job.error = Some(error.clone());
            Some(job.clone())
        }
        RunEvent::CapabilityJobFinished {
            job_id,
            name,
            status,
            summary,
        } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: status.clone(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = status.clone();
            job.summary = Some(summary.clone());
            job.finished_at = Some(Utc::now());
            job.error = None;
            Some(job.clone())
        }
        RunEvent::CapabilityJobFailed {
            job_id,
            name,
            error,
        } => {
            let job = jobs
                .entry(job_id.clone())
                .or_insert_with(|| CapabilityJobDto {
                    id: job_id.clone(),
                    name: name.clone(),
                    kind: "unknown".to_owned(),
                    risk: "unknown".to_owned(),
                    permission_scopes: Vec::new(),
                    status: "failed".to_owned(),
                    summary: None,
                    target: None,
                    session_id: meta.session_id.clone(),
                    gateway_run_id: Some(meta.gateway_run_id.clone()),
                    correlation_id: Some(meta.correlation_id.clone()),
                    started_at: Utc::now(),
                    finished_at: None,
                    error: None,
                });
            job.status = "failed".to_owned();
            job.error = Some(error.clone());
            job.finished_at = Some(Utc::now());
            Some(job.clone())
        }
        _ => None,
    }
}

pub(crate) async fn execute_capability_tool(
    state: Arc<GatewayState>,
    tool_name: &str,
    session_id: Option<String>,
    input: serde_json::Value,
) -> Result<CapabilityJobDto> {
    let tool = state
        .snapshot_components()
        .tools
        .get(tool_name)
        .ok_or_else(|| anyhow!("tool not found: {}", tool_name))?;
    let metadata = tool.metadata().clone();
    let mut job = CapabilityJobDto {
        id: Uuid::new_v4().to_string(),
        name: metadata.name.clone(),
        kind: metadata.capability.kind.label().to_owned(),
        risk: metadata.capability.risk.label().to_owned(),
        permission_scopes: metadata
            .capability
            .permission_scopes
            .iter()
            .map(|scope| scope.label().to_owned())
            .collect(),
        status: "queued".to_owned(),
        summary: None,
        target: None,
        session_id,
        gateway_run_id: None,
        correlation_id: None,
        started_at: Utc::now(),
        finished_at: None,
        error: None,
    };
    job = store_and_broadcast_capability_job(&state, job);

    if !metadata.capability.authorized {
        job.status = "failed".to_owned();
        job.error = Some(format!(
            "tool '{}' is not authorized for execution",
            metadata.name
        ));
        job.finished_at = Some(Utc::now());
        store_and_broadcast_capability_job(&state, job.clone());
        bail!(
            job.error
                .clone()
                .unwrap_or_else(|| "capability execution failed".to_owned())
        );
    }
    if !metadata.capability.healthy {
        job.status = "failed".to_owned();
        job.error = Some(format!("tool '{}' is not healthy", metadata.name));
        job.finished_at = Some(Utc::now());
        store_and_broadcast_capability_job(&state, job.clone());
        bail!(
            job.error
                .clone()
                .unwrap_or_else(|| "capability execution failed".to_owned())
        );
    }

    job.status = "running".to_owned();
    job = store_and_broadcast_capability_job(&state, job);

    let attempts = usize::from(metadata.capability.execution.retry_limit) + 1;
    let timeout = Duration::from_millis(metadata.capability.execution.timeout_ms.max(1));
    for attempt in 1..=attempts {
        match tokio::time::timeout(
            timeout,
            tool.call(input.clone(), &mosaic_tool_core::ToolContext::default()),
        )
        .await
        {
            Ok(Ok(result)) if !result.is_error => {
                job.status = "success".to_owned();
                job.summary = result
                    .audit
                    .as_ref()
                    .map(|audit| audit.side_effect_summary.clone())
                    .or_else(|| Some(truncate_preview(&result.content, 180)));
                job.target = result.audit.as_ref().and_then(|audit| audit.target.clone());
                job.finished_at = Some(Utc::now());
                job.error = None;
                return Ok(store_and_broadcast_capability_job(&state, job));
            }
            Ok(Ok(result)) => {
                let error = result.content.clone();
                if attempt < attempts {
                    job.status = "retrying".to_owned();
                    job.summary = Some(format!("retry attempt {}", attempt));
                    job.error = Some(error);
                    job = store_and_broadcast_capability_job(&state, job);
                    continue;
                }
                job.status = "failed".to_owned();
                job.summary = result
                    .audit
                    .as_ref()
                    .map(|audit| audit.side_effect_summary.clone())
                    .or_else(|| Some(truncate_preview(&result.content, 180)));
                job.target = result.audit.as_ref().and_then(|audit| audit.target.clone());
                job.error = Some(error.clone());
                job.finished_at = Some(Utc::now());
                store_and_broadcast_capability_job(&state, job.clone());
                bail!(error);
            }
            Ok(Err(err)) => {
                if attempt < attempts {
                    job.status = "retrying".to_owned();
                    job.summary = Some(format!("retry attempt {}", attempt));
                    job.error = Some(err.to_string());
                    job = store_and_broadcast_capability_job(&state, job);
                    continue;
                }
                job.status = "failed".to_owned();
                job.error = Some(err.to_string());
                job.finished_at = Some(Utc::now());
                store_and_broadcast_capability_job(&state, job.clone());
                return Err(err);
            }
            Err(_) => {
                let error = format!(
                    "tool '{}' timed out after {}ms",
                    metadata.name,
                    metadata.capability.execution.timeout_ms.max(1)
                );
                if attempt < attempts {
                    job.status = "retrying".to_owned();
                    job.summary = Some(format!("retry attempt {}", attempt));
                    job.error = Some(error.clone());
                    job = store_and_broadcast_capability_job(&state, job);
                    continue;
                }
                job.status = "failed".to_owned();
                job.error = Some(error.clone());
                job.finished_at = Some(Utc::now());
                store_and_broadcast_capability_job(&state, job.clone());
                bail!(error);
            }
        }
    }

    unreachable!("capability execution should return success or failure")
}

pub(crate) fn store_and_broadcast_capability_job(
    state: &GatewayState,
    job: CapabilityJobDto,
) -> CapabilityJobDto {
    state
        .capability_jobs
        .lock()
        .expect("capability jobs lock should not be poisoned")
        .insert(job.id.clone(), job.clone());

    if job.status == "queued" {
        increment_metric(state, |metrics| metrics.capability_jobs_total += 1);
    }
    maybe_record_capability_job_audit(state, &job, None);
    broadcast_envelope(
        state,
        GatewayEventEnvelope {
            gateway_run_id: job
                .gateway_run_id
                .clone()
                .unwrap_or_else(|| format!("capability-{}", job.id)),
            correlation_id: job
                .correlation_id
                .clone()
                .unwrap_or_else(|| format!("capability-{}", job.id)),
            session_id: job.session_id.clone(),
            session_route: job
                .session_id
                .as_deref()
                .map(session_route_for_id)
                .unwrap_or_else(|| "gateway.local/capabilities".to_owned()),
            emitted_at: Utc::now(),
            event: GatewayEvent::CapabilityJobUpdated { job: job.clone() },
        },
    );

    job
}
