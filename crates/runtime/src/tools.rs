use super::*;

impl AgentRuntime {
    pub(crate) async fn maybe_dispatch_tool_via_node(
        &self,
        session_id: Option<&str>,
        tool_name: &str,
        tool_input: &serde_json::Value,
        metadata: &ToolMetadata,
        timeout: Duration,
    ) -> Result<NodeToolDispatchOutcome> {
        let Some(router) = self.ctx.node_router.as_ref() else {
            return Ok(NodeToolDispatchOutcome::NotHandled);
        };
        let Some(capability) = metadata.capability.node.capability.clone() else {
            return Ok(NodeToolDispatchOutcome::NotHandled);
        };

        router
            .dispatch(NodeToolExecutionRequest {
                session_id: session_id.map(ToOwned::to_owned),
                tool_name: tool_name.to_owned(),
                capability,
                input: tool_input.clone(),
                timeout,
            })
            .await
    }

    pub(crate) fn node_failure_status(message: &str) -> &'static str {
        if message.contains("permission denied") {
            "node_permission_denied"
        } else if message.contains("node stale") {
            "node_stale"
        } else if message.contains("node unavailable") || message.contains("no node is available") {
            "node_unavailable"
        } else {
            "failed"
        }
    }

    pub(crate) async fn invoke_tool_with_guardrails(
        &self,
        session_id: Option<&str>,
        tool_name: String,
        call_id: String,
        tool_input: serde_json::Value,
    ) -> std::result::Result<ToolExecutionOutcome, ToolExecutionFailure> {
        self.emit(RunEvent::ToolCalling {
            name: tool_name.clone(),
            call_id: call_id.clone(),
        });

        let tool = match self.ctx.tools.get(&tool_name) {
            Some(tool) => tool,
            None => {
                let error = anyhow!("tool not found: {}", tool_name);
                self.emit(RunEvent::ToolFailed {
                    name: tool_name,
                    call_id,
                    error: error.to_string(),
                });
                return Err(ToolExecutionFailure {
                    error,
                    tool_trace: None,
                    capability_trace: None,
                });
            }
        };

        let metadata = tool.metadata().clone();
        let started_at = Utc::now();
        let job_id = Uuid::new_v4().to_string();
        self.emit(RunEvent::CapabilityJobQueued {
            job_id: job_id.clone(),
            name: tool_name.clone(),
            kind: metadata.capability.kind.label().to_owned(),
            risk: metadata.capability.risk.label().to_owned(),
            permission_scopes: Self::permission_scope_labels(&metadata),
        });

        if !metadata.capability.authorized {
            let error = anyhow!("tool '{}' is not authorized for execution", tool_name);
            self.emit(RunEvent::PermissionCheckFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                reason: error.to_string(),
            });
            self.emit(RunEvent::CapabilityJobFailed {
                job_id: job_id.clone(),
                name: tool_name.clone(),
                error: error.to_string(),
            });
            self.emit(RunEvent::ToolFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                error: error.to_string(),
            });
            return Err(self.build_tool_failure(
                error, job_id, call_id, tool_name, &metadata, tool_input, started_at, None, None,
                None, "rejected",
            ));
        }

        if !metadata.capability.healthy {
            let error = anyhow!("tool '{}' is not healthy", tool_name);
            self.emit(RunEvent::PermissionCheckFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                reason: error.to_string(),
            });
            self.emit(RunEvent::CapabilityJobFailed {
                job_id: job_id.clone(),
                name: tool_name.clone(),
                error: error.to_string(),
            });
            self.emit(RunEvent::ToolFailed {
                name: tool_name.clone(),
                call_id: call_id.clone(),
                error: error.to_string(),
            });
            return Err(self.build_tool_failure(
                error, job_id, call_id, tool_name, &metadata, tool_input, started_at, None, None,
                None, "rejected",
            ));
        }

        self.emit(RunEvent::CapabilityJobStarted {
            job_id: job_id.clone(),
            name: tool_name.clone(),
        });

        let attempts = usize::from(metadata.capability.execution.retry_limit) + 1;
        let timeout = Duration::from_millis(metadata.capability.execution.timeout_ms.max(1));

        if metadata.capability.routes_via_node() {
            match self
                .maybe_dispatch_tool_via_node(
                    session_id,
                    &tool_name,
                    &tool_input,
                    &metadata,
                    timeout,
                )
                .await
            {
                Ok(NodeToolDispatchOutcome::Completed(execution)) => {
                    let finished_at = Utc::now();
                    let node_trace = NodeTraceContext {
                        node_id: Some(execution.node_id),
                        capability_route: Some(execution.route),
                        disconnect_context: execution.disconnect_context,
                    };
                    let result = execution.result;
                    let output = result.content.clone();
                    let tool_trace = ToolTrace {
                        call_id: Some(call_id.clone()),
                        name: tool_name.clone(),
                        source: metadata.source.clone(),
                        input: tool_input,
                        output: Some(output.clone()),
                        node_id: node_trace.node_id.clone(),
                        capability_route: node_trace.capability_route.clone(),
                        disconnect_context: node_trace.disconnect_context.clone(),
                        started_at,
                        finished_at: Some(finished_at),
                    };
                    let capability_trace = Self::capability_trace(
                        &job_id,
                        &call_id,
                        &tool_name,
                        &metadata,
                        result.audit.as_ref(),
                        started_at,
                        finished_at,
                        "success",
                        None,
                        Some(output.as_str()),
                        Some(&node_trace),
                    );
                    self.emit(RunEvent::CapabilityJobFinished {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        status: "success".to_owned(),
                        summary: capability_trace.summary.clone(),
                    });
                    self.emit(RunEvent::ToolFinished {
                        name: tool_name,
                        call_id,
                    });
                    return Ok(ToolExecutionOutcome {
                        output,
                        tool_trace,
                        capability_trace,
                    });
                }
                Ok(NodeToolDispatchOutcome::Failed(node_error)) => {
                    let status = Self::node_failure_status(&node_error.message);
                    let error = anyhow!(node_error.message.clone());
                    let node_trace = NodeTraceContext {
                        node_id: node_error.node_id,
                        capability_route: node_error.route,
                        disconnect_context: node_error.disconnect_context,
                    };
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: error.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: error.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        error,
                        job_id,
                        call_id,
                        tool_name,
                        &metadata,
                        tool_input,
                        started_at,
                        None,
                        None,
                        Some(node_trace),
                        status,
                    ));
                }
                Ok(NodeToolDispatchOutcome::NotHandled) => {
                    if metadata.capability.node.require_node {
                        let capability = metadata
                            .capability
                            .node
                            .capability
                            .as_deref()
                            .unwrap_or(tool_name.as_str());
                        let error = anyhow!(
                            "node route required for capability '{}' but no node is available",
                            capability
                        );
                        self.emit(RunEvent::CapabilityJobFailed {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            error: error.to_string(),
                        });
                        self.emit(RunEvent::ToolFailed {
                            name: tool_name.clone(),
                            call_id: call_id.clone(),
                            error: error.to_string(),
                        });
                        return Err(self.build_tool_failure(
                            error,
                            job_id,
                            call_id,
                            tool_name,
                            &metadata,
                            tool_input,
                            started_at,
                            None,
                            None,
                            None,
                            "node_unavailable",
                        ));
                    }
                }
                Err(err) => {
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: err.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: err.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        err, job_id, call_id, tool_name, &metadata, tool_input, started_at, None,
                        None, None, "failed",
                    ));
                }
            }
        }

        for attempt in 1..=attempts {
            let attempt_result = tokio::time::timeout(timeout, tool.call(tool_input.clone())).await;
            match attempt_result {
                Ok(Ok(result)) if !result.is_error => {
                    let finished_at = Utc::now();
                    let output = result.content.clone();
                    let tool_trace = ToolTrace {
                        call_id: Some(call_id.clone()),
                        name: tool_name.clone(),
                        source: metadata.source.clone(),
                        input: tool_input,
                        output: Some(output.clone()),
                        node_id: None,
                        capability_route: None,
                        disconnect_context: None,
                        started_at,
                        finished_at: Some(finished_at),
                    };
                    let capability_trace = Self::capability_trace(
                        &job_id,
                        &call_id,
                        &tool_name,
                        &metadata,
                        result.audit.as_ref(),
                        started_at,
                        finished_at,
                        "success",
                        None,
                        Some(output.as_str()),
                        None,
                    );
                    self.emit(RunEvent::CapabilityJobFinished {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        status: "success".to_owned(),
                        summary: capability_trace.summary.clone(),
                    });
                    self.emit(RunEvent::ToolFinished {
                        name: tool_name,
                        call_id,
                    });
                    return Ok(ToolExecutionOutcome {
                        output,
                        tool_trace,
                        capability_trace,
                    });
                }
                Ok(Ok(result)) => {
                    let error = anyhow!(result.content.clone());
                    if attempt < attempts {
                        self.emit(RunEvent::CapabilityJobRetried {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            attempt: attempt as u8,
                            error: error.to_string(),
                        });
                        continue;
                    }
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: error.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: error.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        error,
                        job_id,
                        call_id,
                        tool_name,
                        &metadata,
                        tool_input,
                        started_at,
                        Some(result.content),
                        result.audit.as_ref(),
                        None,
                        "failed",
                    ));
                }
                Ok(Err(err)) => {
                    if attempt < attempts {
                        self.emit(RunEvent::CapabilityJobRetried {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            attempt: attempt as u8,
                            error: err.to_string(),
                        });
                        continue;
                    }
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: err.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: err.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        err, job_id, call_id, tool_name, &metadata, tool_input, started_at, None,
                        None, None, "failed",
                    ));
                }
                Err(_) => {
                    let error = anyhow!(
                        "tool '{}' timed out after {}ms",
                        tool_name,
                        metadata.capability.execution.timeout_ms.max(1)
                    );
                    if attempt < attempts {
                        self.emit(RunEvent::CapabilityJobRetried {
                            job_id: job_id.clone(),
                            name: tool_name.clone(),
                            attempt: attempt as u8,
                            error: error.to_string(),
                        });
                        continue;
                    }
                    self.emit(RunEvent::CapabilityJobFailed {
                        job_id: job_id.clone(),
                        name: tool_name.clone(),
                        error: error.to_string(),
                    });
                    self.emit(RunEvent::ToolFailed {
                        name: tool_name.clone(),
                        call_id: call_id.clone(),
                        error: error.to_string(),
                    });
                    return Err(self.build_tool_failure(
                        error,
                        job_id,
                        call_id,
                        tool_name,
                        &metadata,
                        tool_input,
                        started_at,
                        None,
                        None,
                        None,
                        "timed_out",
                    ));
                }
            }
        }

        unreachable!("tool attempts should always return success or failure")
    }

    pub(crate) fn build_tool_failure(
        &self,
        error: anyhow::Error,
        job_id: String,
        call_id: String,
        tool_name: String,
        metadata: &ToolMetadata,
        tool_input: serde_json::Value,
        started_at: chrono::DateTime<Utc>,
        output: Option<String>,
        audit: Option<&CapabilityAudit>,
        node_trace: Option<NodeTraceContext>,
        status: &str,
    ) -> ToolExecutionFailure {
        let finished_at = Utc::now();
        let tool_trace = ToolTrace {
            call_id: Some(call_id.clone()),
            name: tool_name.clone(),
            source: metadata.source.clone(),
            input: tool_input,
            output: output
                .clone()
                .or_else(|| Some(format!("[runtime tool failure] {}", error))),
            node_id: node_trace.as_ref().and_then(|trace| trace.node_id.clone()),
            capability_route: node_trace
                .as_ref()
                .and_then(|trace| trace.capability_route.clone()),
            disconnect_context: node_trace
                .as_ref()
                .and_then(|trace| trace.disconnect_context.clone()),
            started_at,
            finished_at: Some(finished_at),
        };
        let capability_trace = Self::capability_trace(
            &job_id,
            &call_id,
            &tool_name,
            metadata,
            audit,
            started_at,
            finished_at,
            status,
            Some(error.to_string()),
            output.as_deref(),
            node_trace.as_ref(),
        );

        ToolExecutionFailure {
            error,
            tool_trace: Some(tool_trace),
            capability_trace: Some(capability_trace),
        }
    }

    pub(crate) fn capability_trace(
        job_id: &str,
        call_id: &str,
        tool_name: &str,
        metadata: &ToolMetadata,
        audit: Option<&CapabilityAudit>,
        started_at: chrono::DateTime<Utc>,
        finished_at: chrono::DateTime<Utc>,
        status: &str,
        error: Option<String>,
        fallback_summary: Option<&str>,
        node_trace: Option<&NodeTraceContext>,
    ) -> CapabilityInvocationTrace {
        let summary = audit
            .map(|audit| audit.side_effect_summary.clone())
            .or_else(|| fallback_summary.map(|value| Self::truncate_preview(value, 180)))
            .unwrap_or_else(|| format!("{} {}", tool_name, status));

        CapabilityInvocationTrace {
            job_id: job_id.to_owned(),
            call_id: Some(call_id.to_owned()),
            tool_name: tool_name.to_owned(),
            kind: metadata.capability.kind.clone(),
            permission_scopes: metadata.capability.permission_scopes.clone(),
            risk: metadata.capability.risk.clone(),
            status: status.to_owned(),
            summary,
            target: audit.and_then(|audit| audit.target.clone()),
            node_id: node_trace.and_then(|trace| trace.node_id.clone()),
            capability_route: node_trace.and_then(|trace| trace.capability_route.clone()),
            disconnect_context: node_trace.and_then(|trace| trace.disconnect_context.clone()),
            started_at,
            finished_at: Some(finished_at),
            error,
        }
    }

    pub(crate) fn permission_scope_labels(metadata: &ToolMetadata) -> Vec<String> {
        metadata
            .capability
            .permission_scopes
            .iter()
            .map(|scope| scope.label().to_owned())
            .collect()
    }

    pub(crate) fn collect_tool_definitions(
        &self,
        allowlist: Option<&[String]>,
        channel: Option<&str>,
        bot_name: Option<&str>,
    ) -> Result<Vec<ToolDefinition>> {
        let telegram_bot: Option<&mosaic_config::TelegramBotConfig> =
            self.telegram_bot_for(channel, bot_name);
        match allowlist {
            Some(names) => names
                .iter()
                .map(|name| {
                    let tool = self
                        .ctx
                        .tools
                        .get(name)
                        .ok_or_else(|| anyhow!("tool not found: {}", name))?;
                    let metadata = tool.metadata();
                    if !tool_is_visible_to_model(metadata)
                        || !metadata.exposure.allows_conversational(channel)
                    {
                        bail!("tool is not visible to this session: {}", name);
                    }
                    if let Some(bot) = telegram_bot {
                        if !bot.allows_tool(name) {
                            bail!(
                                "tool is not visible to telegram bot '{}': {}",
                                bot_name.unwrap_or("unknown"),
                                name
                            );
                        }
                    }
                    Ok(tool_definition_from_metadata(metadata))
                })
                .collect(),
            None => Ok(self
                .ctx
                .tools
                .iter()
                .filter_map(|tool| {
                    let metadata = tool.metadata();
                    (tool_is_visible_to_model(metadata)
                        && metadata.exposure.allows_conversational(channel))
                    .then_some((
                        metadata.name.clone(),
                        tool_definition_from_metadata(metadata),
                    ))
                })
                .filter(|(name, _)| {
                    telegram_bot
                        .map(|bot| bot.allows_tool(name))
                        .unwrap_or(true)
                })
                .map(|(_, definition)| definition)
                .collect()),
        }
    }
}
