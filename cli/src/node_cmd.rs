use crate::*;

pub(crate) async fn node_cmd(command: NodeCliCommand) -> Result<()> {
    match command {
        NodeCliCommand::Serve { id, label } => serve_local_node(id, label).await,
        NodeCliCommand::List => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            print_node_list(&gateway.list_nodes()?, &gateway.list_node_affinities()?)
        }
        NodeCliCommand::Attach { node_id, session } => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            gateway.attach_node(&node_id, session.as_deref())?;
            match session {
                Some(session) => println!("attached node {} to session {}", node_id, session),
                None => println!("attached node {} as the default node route", node_id),
            }
            Ok(())
        }
        NodeCliCommand::Detach { session, default } => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            let detached = match (session.as_deref(), default) {
                (Some(session_id), false) => {
                    let detached = gateway.detach_node(Some(session_id))?;
                    if detached {
                        println!("detached node affinity from session {}", session_id);
                    } else {
                        println!("no node affinity found for session {}", session_id);
                    }
                    detached
                }
                (None, true) => {
                    let detached = gateway.detach_node(None)?;
                    if detached {
                        println!("detached default node affinity");
                    } else {
                        println!("no default node affinity found");
                    }
                    detached
                }
                _ => anyhow::bail!("use exactly one of `--session <id>` or `--default`"),
            };
            if detached {
                crate::print_next_steps(["mosaic node list"]);
            }
            Ok(())
        }
        NodeCliCommand::Prune { stale } => {
            if !stale {
                anyhow::bail!("use `mosaic node prune --stale` to remove offline or stale nodes");
            }
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            let removed = gateway.prune_stale_nodes()?;
            if removed.is_empty() {
                println!("no offline or stale node registrations found");
                return Ok(());
            }

            println!("pruned node registrations:");
            for node in &removed {
                println!(
                    "  - {} | health={} | last_disconnect_reason={}",
                    node.node_id,
                    node.health(
                        chrono::Utc::now(),
                        mosaic_node_protocol::DEFAULT_STALE_AFTER_SECS,
                    )
                    .label(),
                    node.last_disconnect_reason.as_deref().unwrap_or("<none>"),
                );
            }

            let removed_ids = removed
                .iter()
                .map(|node| node.node_id.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            let dangling = gateway
                .list_node_affinities()?
                .into_iter()
                .filter(|record| removed_ids.contains(record.node_id.as_str()))
                .collect::<Vec<_>>();
            if !dangling.is_empty() {
                println!("dangling affinities:");
                for record in dangling {
                    println!(
                        "  - {} -> {}",
                        affinity_scope_label(&record.session_id),
                        record.node_id,
                    );
                }
                println!(
                    "operator_hint: use `mosaic node detach --session <id>` or `mosaic node detach --default` to clear dangling affinity records"
                );
            }

            crate::print_next_steps(["mosaic node list"]);
            Ok(())
        }
        NodeCliCommand::Capabilities { node_id } => {
            let loaded = ensure_loaded_config(None)?;
            let gateway = build_gateway_handle(&loaded, None)?;
            match node_id {
                Some(node_id) => {
                    print_node_capabilities(&node_id, &gateway.node_capabilities(&node_id)?)
                }
                None => {
                    let nodes = gateway.list_nodes()?;
                    if nodes.is_empty() {
                        println!("no nodes found");
                        return Ok(());
                    }
                    for node in nodes {
                        print_node_capabilities(&node.node_id, &node.capabilities)?;
                    }
                    Ok(())
                }
            }
        }
    }
}

async fn serve_local_node(id: Option<String>, label: Option<String>) -> Result<()> {
    let node_store =
        mosaic_node_protocol::FileNodeStore::new(resolve_workspace_relative_path(".mosaic/nodes")?);
    let id = id.unwrap_or_else(|| "local-headless".to_owned());
    let label = label.unwrap_or_else(|| "Local Headless Node".to_owned());
    let (read_file_tool, exec_tool) = build_headless_node_tools()?;
    let registration = mosaic_node_protocol::NodeRegistration::new(
        id.clone(),
        label.clone(),
        "file-bus",
        "headless",
        vec![
            tool_node_capability(&read_file_tool),
            tool_node_capability(&exec_tool),
        ],
    );
    node_store.register_node(&registration)?;
    let _ = node_store.heartbeat(&id)?;
    println!("headless node ready");
    println!("node_id: {}", id);
    println!("label: {}", label);
    println!("transport: file-bus");
    println!("node_store: {}", node_store.root().display());
    println!(
        "capabilities: {:?}",
        registration
            .capabilities
            .iter()
            .map(|cap| cap.name.as_str())
            .collect::<Vec<_>>()
    );
    println!("press Ctrl-C to stop");
    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                node_store.disconnect_node(&id, "operator_shutdown")?;
                println!("headless node stopped");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                let _ = node_store.heartbeat(&id)?;
                for dispatch in node_store.pending_commands(&id)? {
                    execute_headless_node_dispatch(&node_store, &dispatch, &read_file_tool, &exec_tool).await?;
                }
            }
        }
    }
    Ok(())
}

fn build_headless_node_tools()
-> Result<(mosaic_tool_core::ReadFileTool, mosaic_tool_core::ExecTool)> {
    let roots = vec![std::env::current_dir()?];
    Ok((
        mosaic_tool_core::ReadFileTool::new_with_allowed_roots(vec![roots[0].clone()]),
        mosaic_tool_core::ExecTool::new(roots),
    ))
}

fn tool_node_capability(
    tool: &dyn mosaic_tool_core::Tool,
) -> mosaic_node_protocol::NodeCapabilityDeclaration {
    let metadata = tool.metadata();
    mosaic_node_protocol::NodeCapabilityDeclaration {
        name: metadata
            .capability
            .node
            .capability
            .clone()
            .unwrap_or_else(|| metadata.name.clone()),
        kind: metadata.capability.kind.clone(),
        permission_scopes: metadata.capability.permission_scopes.clone(),
        risk: metadata.capability.risk.clone(),
    }
}

async fn execute_headless_node_dispatch(
    node_store: &mosaic_node_protocol::FileNodeStore,
    dispatch: &mosaic_node_protocol::NodeCommandDispatch,
    read_file_tool: &mosaic_tool_core::ReadFileTool,
    exec_tool: &mosaic_tool_core::ExecTool,
) -> Result<()> {
    let result = match dispatch.capability.as_str() {
        "read_file" => {
            read_file_tool
                .call(
                    dispatch.input.clone(),
                    &mosaic_tool_core::ToolContext::default(),
                )
                .await
        }
        "exec_command" => {
            exec_tool
                .call(
                    dispatch.input.clone(),
                    &mosaic_tool_core::ToolContext::default(),
                )
                .await
        }
        capability => Err(anyhow!("unsupported node capability: {}", capability)),
    };

    let envelope = match result {
        Ok(result) => mosaic_node_protocol::NodeCommandResultEnvelope::success(dispatch, result),
        Err(err) => mosaic_node_protocol::NodeCommandResultEnvelope::failure(
            dispatch,
            "failed",
            err.to_string(),
            None,
        ),
    };
    node_store.complete_command(&envelope)
}

fn print_node_list(
    nodes: &[mosaic_node_protocol::NodeRegistration],
    affinities: &[mosaic_node_protocol::NodeAffinityRecord],
) -> Result<()> {
    if nodes.is_empty() {
        println!("no nodes found");
        return Ok(());
    }

    for node in nodes {
        let health = node.health(
            chrono::Utc::now(),
            mosaic_node_protocol::DEFAULT_STALE_AFTER_SECS,
        );
        println!(
            "{} | health={} | transport={} | platform={} | capabilities={} | last_heartbeat_at={}",
            node.node_id,
            health.label(),
            node.transport,
            node.platform,
            node.capabilities
                .iter()
                .map(|cap| cap.name.as_str())
                .collect::<Vec<_>>()
                .join(","),
            node.last_heartbeat_at,
        );
        let references = affinities
            .iter()
            .filter(|record| record.node_id == node.node_id)
            .map(|record| affinity_scope_label(&record.session_id))
            .collect::<Vec<_>>();
        if !references.is_empty() {
            println!("  affinities: {}", references.join(", "));
        }
        if let Some(reason) = node.last_disconnect_reason.as_deref() {
            println!("  disconnect_reason: {}", reason);
        }
    }
    Ok(())
}

fn affinity_scope_label(session_id: &str) -> String {
    if session_id == mosaic_node_protocol::DEFAULT_AFFINITY_KEY {
        "default".to_owned()
    } else {
        format!("session:{}", session_id)
    }
}

fn print_node_capabilities(
    node_id: &str,
    capabilities: &[mosaic_node_protocol::NodeCapabilityDeclaration],
) -> Result<()> {
    println!("node {}", node_id);
    if capabilities.is_empty() {
        println!("  capabilities: none");
        return Ok(());
    }
    for capability in capabilities {
        println!(
            "  - {} | kind={} | risk={} | scopes={:?}",
            capability.name,
            capability.kind.label(),
            capability.risk.label(),
            capability
                .permission_scopes
                .iter()
                .map(|scope| scope.label())
                .collect::<Vec<_>>(),
        );
    }
    Ok(())
}
