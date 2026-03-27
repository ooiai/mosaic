use std::collections::BTreeMap;

use anyhow::{Result, bail};
use mosaic_control_protocol::{CronRegistrationRequest, ExecJobRequest, WebhookJobRequest};

use crate::{
    CapabilityCommand, CronCommand, ExecCapabilityCommand, WebhookCapabilityCommand,
    build_gateway_handle, ensure_loaded_config, gateway_client_from_loaded, print_capability_job,
    print_capability_jobs, print_cron_registrations,
};

pub(crate) async fn capability_cmd(
    attach: Option<String>,
    command: CapabilityCommand,
) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;

    match command {
        CapabilityCommand::Doctor => {
            if attach.is_some() {
                bail!("remote capability doctor is not supported");
            }
            let components = crate::bootstrap::build_gateway_components(&loaded.config, None)?;
            println!("capability doctor:");
            println!("workspace_root: {}", std::env::current_dir()?.display());
            for tool in components.tools.iter() {
                let meta = tool.metadata();
                println!(
                    "{} | kind={} | risk={} | scopes={:?} | timeout_ms={} | retry_limit={} | authorized={} | healthy={} | source={}",
                    meta.name,
                    meta.capability.kind.label(),
                    meta.capability.risk.label(),
                    meta.capability
                        .permission_scopes
                        .iter()
                        .map(|scope| scope.label())
                        .collect::<Vec<_>>(),
                    meta.capability.execution.timeout_ms,
                    meta.capability.execution.retry_limit,
                    meta.capability.authorized,
                    meta.capability.healthy,
                    meta.source.label(),
                );
            }
            Ok(())
        }
        CapabilityCommand::Jobs => {
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                return print_capability_jobs(&client.list_capability_jobs().await?);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            print_capability_jobs(&gateway.list_capability_jobs())
        }
        CapabilityCommand::Exec { command } => match command {
            ExecCapabilityCommand::Guardrails => {
                if attach.is_some() {
                    bail!("remote exec guardrails are not supported");
                }
                println!("exec guardrails:");
                println!("  allowed_root: {}", std::env::current_dir()?.display());
                println!("  permission_scope: local_exec");
                println!("  timeout_policy: tool metadata controlled");
                Ok(())
            }
            ExecCapabilityCommand::Run {
                command,
                args,
                cwd,
                session,
            } => {
                if let Some(url) = attach {
                    let client = gateway_client_from_loaded(&loaded, url);
                    let job = client
                        .run_exec_job(ExecJobRequest {
                            session_id: session,
                            command,
                            args,
                            cwd,
                        })
                        .await?;
                    return print_capability_job(&job);
                }

                let gateway = build_gateway_handle(&loaded, None)?;
                let job = gateway
                    .run_exec_job(ExecJobRequest {
                        session_id: session,
                        command,
                        args,
                        cwd,
                    })
                    .await?;
                print_capability_job(&job)
            }
        },
        CapabilityCommand::Webhook { command } => match command {
            WebhookCapabilityCommand::Test {
                url,
                method,
                body,
                headers,
                session,
            } => {
                let request = WebhookJobRequest {
                    session_id: session,
                    url,
                    method,
                    body,
                    headers: parse_header_args(&headers)?,
                };
                if let Some(url) = attach {
                    let client = gateway_client_from_loaded(&loaded, url);
                    let job = client.run_webhook_job(request).await?;
                    return print_capability_job(&job);
                }

                let gateway = build_gateway_handle(&loaded, None)?;
                let job = gateway.run_webhook_job(request).await?;
                print_capability_job(&job)
            }
        },
    }
}

pub(crate) async fn cron_cmd(attach: Option<String>, command: CronCommand) -> Result<()> {
    let loaded = ensure_loaded_config(None)?;

    match command {
        CronCommand::List => {
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                return print_cron_registrations(&client.list_cron_registrations().await?);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            let registrations = gateway
                .list_cron_registrations()?
                .iter()
                .map(mosaic_gateway::cron_registration_dto)
                .collect::<Vec<_>>();
            print_cron_registrations(&registrations)
        }
        CronCommand::Register {
            id,
            schedule,
            input,
            session,
            profile,
            skill,
            workflow,
        } => {
            let request = CronRegistrationRequest {
                id,
                schedule,
                input,
                session_id: session,
                profile,
                skill,
                workflow,
            };
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                let registration = client.register_cron(request).await?;
                return print_cron_registrations(&[registration]);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            let registration = gateway.register_cron(request)?;
            print_cron_registrations(&[mosaic_gateway::cron_registration_dto(&registration)])
        }
        CronCommand::Trigger { id } => {
            if let Some(url) = attach {
                let client = gateway_client_from_loaded(&loaded, url);
                let response = client.trigger_cron(&id).await?;
                return crate::finish_remote_gateway_run(&loaded, response);
            }
            let gateway = build_gateway_handle(&loaded, None)?;
            let result = gateway.trigger_cron(&id).await?;
            crate::run_cmd::finish_successful_gateway_run(result)
        }
    }
}

fn parse_header_args(headers: &[String]) -> Result<BTreeMap<String, String>> {
    let mut parsed = BTreeMap::new();
    for header in headers {
        let Some((name, value)) = header.split_once('=') else {
            bail!("invalid header '{}'; expected KEY=VALUE", header);
        };
        parsed.insert(name.trim().to_owned(), value.trim().to_owned());
    }
    Ok(parsed)
}
