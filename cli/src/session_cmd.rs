use anyhow::{Result, anyhow};
use mosaic_control_protocol::SessionDetailDto;

use crate::{
    SessionCommand, build_gateway_handle, ensure_loaded_config, gateway_client_from_loaded,
};

pub(crate) async fn session_cmd(attach: Option<String>, command: SessionCommand) -> Result<()> {
    if let Some(url) = attach {
        let loaded = ensure_loaded_config(None)?;
        let client = gateway_client_from_loaded(&loaded, url);

        return match command {
            SessionCommand::List => {
                crate::print_remote_session_list(&client.list_sessions().await?)
            }
            SessionCommand::Show { id } => {
                let session = client
                    .get_session(&id)
                    .await?
                    .ok_or_else(|| anyhow!("session not found: {}", id))?;
                print_remote_session_detail(&session)
            }
        };
    }

    let loaded = ensure_loaded_config(None)?;
    let gateway = build_gateway_handle(&loaded, None)?;

    match command {
        SessionCommand::List => crate::print_session_list(&gateway.list_sessions()?),
        SessionCommand::Show { id } => {
            let session = gateway
                .load_session(&id)?
                .ok_or_else(|| anyhow!("session not found: {}", id))?;

            println!("id: {}", session.id);
            println!("title: {}", session.title);
            println!("created_at: {}", session.created_at);
            println!("updated_at: {}", session.updated_at);
            println!("provider_profile: {}", session.provider_profile);
            println!("provider_type: {}", session.provider_type);
            println!("model: {}", session.model);
            println!("last_run_id: {:?}", session.last_run_id);
            println!("run_status: {}", session.run.status.label());
            println!("current_run_id: {:?}", session.run.current_run_id);
            println!(
                "current_gateway_run_id: {:?}",
                session.run.current_gateway_run_id
            );
            println!(
                "current_correlation_id: {:?}",
                session.run.current_correlation_id
            );
            println!("last_error: {:?}", session.run.last_error);
            println!("last_failure_kind: {:?}", session.run.last_failure_kind);
            println!("run_updated_at: {:?}", session.run.updated_at);
            println!("session_route: {}", session.gateway.route);
            println!("channel: {:?}", session.channel_context.channel);
            println!("actor_id: {:?}", session.channel_context.actor_id);
            println!("actor_name: {:?}", session.channel_context.actor_name);
            println!("thread_id: {:?}", session.channel_context.thread_id);
            println!("thread_title: {:?}", session.channel_context.thread_title);
            println!("reply_target: {:?}", session.channel_context.reply_target);
            println!(
                "last_gateway_run_id: {:?}",
                session.gateway.last_gateway_run_id
            );
            println!(
                "last_correlation_id: {:?}",
                session.gateway.last_correlation_id
            );
            println!("message_count: {}", session.transcript.len());
            println!("memory_summary: {:?}", session.memory.latest_summary);
            println!(
                "compressed_context: {:?}",
                session.memory.compressed_context
            );
            println!("memory_entry_count: {}", session.memory.memory_entry_count);
            println!("compression_count: {}", session.memory.compression_count);
            println!("reference_count: {}", session.references.len());

            if !session.references.is_empty() {
                println!("\nreferences:");
                for reference in &session.references {
                    println!(
                        "- {} | reason={} | created_at={}",
                        reference.session_id, reference.reason, reference.created_at
                    );
                }
            }

            if !session.transcript.is_empty() {
                println!("\ntranscript:");
                for (idx, message) in session.transcript.iter().enumerate() {
                    println!(
                        "[{}] {} {} {:?}",
                        idx + 1,
                        crate::transcript_role_label(&message.role),
                        message.created_at,
                        message.tool_call_id
                    );
                    println!("  {}", crate::truncate_for_cli(&message.content, 400));
                }
            }

            Ok(())
        }
    }
}

fn print_remote_session_detail(session: &SessionDetailDto) -> Result<()> {
    println!("id: {}", session.id);
    println!("title: {}", session.title);
    println!("created_at: {}", session.created_at);
    println!("updated_at: {}", session.updated_at);
    println!("provider_profile: {}", session.provider_profile);
    println!("provider_type: {}", session.provider_type);
    println!("model: {}", session.model);
    println!("last_run_id: {:?}", session.last_run_id);
    println!("run_status: {}", session.run.status.label());
    println!("current_run_id: {:?}", session.run.current_run_id);
    println!(
        "current_gateway_run_id: {:?}",
        session.run.current_gateway_run_id
    );
    println!(
        "current_correlation_id: {:?}",
        session.run.current_correlation_id
    );
    println!("last_error: {:?}", session.run.last_error);
    println!("last_failure_kind: {:?}", session.run.last_failure_kind);
    println!("run_updated_at: {:?}", session.run.updated_at);
    println!("session_route: {}", session.gateway.route);
    println!("channel: {:?}", session.channel_context.channel);
    println!("actor_id: {:?}", session.channel_context.actor_id);
    println!("actor_name: {:?}", session.channel_context.actor_name);
    println!("thread_id: {:?}", session.channel_context.thread_id);
    println!("thread_title: {:?}", session.channel_context.thread_title);
    println!("reply_target: {:?}", session.channel_context.reply_target);
    println!(
        "last_gateway_run_id: {:?}",
        session.gateway.last_gateway_run_id
    );
    println!(
        "last_correlation_id: {:?}",
        session.gateway.last_correlation_id
    );
    println!("message_count: {}", session.transcript.len());
    println!("memory_summary: {:?}", session.memory_summary);
    println!("compressed_context: {:?}", session.compressed_context);
    println!("reference_count: {}", session.references.len());

    if !session.references.is_empty() {
        println!("\nreferences:");
        for reference in &session.references {
            println!(
                "- {} | reason={} | created_at={}",
                reference.session_id, reference.reason, reference.created_at
            );
        }
    }

    if !session.transcript.is_empty() {
        println!("\ntranscript:");
        for (idx, message) in session.transcript.iter().enumerate() {
            println!(
                "[{}] {} {} {:?}",
                idx + 1,
                crate::remote_transcript_role_label(&message.role),
                message.created_at,
                message.tool_call_id
            );
            println!("  {}", crate::truncate_for_cli(&message.content, 400));
        }
    }

    Ok(())
}
