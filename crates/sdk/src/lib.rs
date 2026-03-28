use std::env;

use anyhow::Result;
use futures::{StreamExt, stream::BoxStream};
use mosaic_channel_telegram::TelegramUpdate;
use mosaic_control_protocol::{
    AdapterStatusDto, CapabilityJobDto, CronRegistrationDto, CronRegistrationRequest,
    ErrorResponse, EventStreamEnvelope, ExecJobRequest, GatewayAuditEventDto, HealthResponse,
    InboundMessage, IncidentBundleDto, MetricsResponse, ReadinessResponse, ReplayWindowResponse,
    RunDetailDto, RunResponse, RunSubmission, RunSummaryDto, SessionDetailDto, SessionSummaryDto,
    WebhookJobRequest,
};

#[derive(Clone)]
pub struct GatewayClient {
    base_url: String,
    http: reqwest::Client,
    bearer_token: Option<String>,
}

impl GatewayClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
            bearer_token: env::var("MOSAIC_OPERATOR_TOKEN")
                .ok()
                .or_else(|| env::var("MOSAIC_GATEWAY_TOKEN").ok()),
        }
    }

    pub fn with_bearer_token(mut self, token: Option<String>) -> Self {
        if token.is_some() {
            self.bearer_token = token;
        }
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        self.get_json("/health").await
    }

    pub async fn readiness(&self) -> Result<ReadinessResponse> {
        self.get_json("/ready").await
    }

    pub async fn metrics(&self) -> Result<MetricsResponse> {
        self.get_json("/metrics").await
    }

    pub async fn audit_events(&self, limit: usize) -> Result<Vec<GatewayAuditEventDto>> {
        self.get_json(&format!("/audit/events?limit={limit}")).await
    }

    pub async fn replay_window(&self, limit: usize) -> Result<ReplayWindowResponse> {
        self.get_json(&format!("/events/recent?limit={limit}"))
            .await
    }

    pub async fn incident_bundle(&self, id: &str) -> Result<IncidentBundleDto> {
        self.get_json(&format!("/incidents/{id}")).await
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionSummaryDto>> {
        self.get_json("/sessions").await
    }

    pub async fn list_adapters(&self) -> Result<Vec<AdapterStatusDto>> {
        self.get_json("/adapters").await
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<SessionDetailDto>> {
        let response = self
            .request(reqwest::Method::GET, &format!("/sessions/{id}"))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        Ok(Some(decode_response(response).await?))
    }

    pub async fn list_runs(&self) -> Result<Vec<RunSummaryDto>> {
        self.get_json("/runs").await
    }

    pub async fn get_run(&self, id: &str) -> Result<Option<RunDetailDto>> {
        let response = self
            .request(reqwest::Method::GET, &format!("/runs/{id}"))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        Ok(Some(decode_response(response).await?))
    }

    pub async fn cancel_run(&self, id: &str) -> Result<RunDetailDto> {
        self.post_json::<RunDetailDto, serde_json::Value>(
            &format!("/runs/{id}/cancel"),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn retry_run(&self, id: &str) -> Result<RunResponse> {
        self.post_json::<RunResponse, serde_json::Value>(
            &format!("/runs/{id}/retry"),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn submit_run(&self, submission: RunSubmission) -> Result<RunResponse> {
        self.post_json("/runs", &submission).await
    }

    pub async fn list_capability_jobs(&self) -> Result<Vec<CapabilityJobDto>> {
        self.get_json("/capabilities/jobs").await
    }

    pub async fn run_exec_job(&self, request: ExecJobRequest) -> Result<CapabilityJobDto> {
        self.post_json("/capabilities/exec", &request).await
    }

    pub async fn run_webhook_job(&self, request: WebhookJobRequest) -> Result<CapabilityJobDto> {
        self.post_json("/capabilities/webhook", &request).await
    }

    pub async fn list_cron_registrations(&self) -> Result<Vec<CronRegistrationDto>> {
        self.get_json("/cron").await
    }

    pub async fn register_cron(
        &self,
        request: CronRegistrationRequest,
    ) -> Result<CronRegistrationDto> {
        self.post_json("/cron", &request).await
    }

    pub async fn trigger_cron(&self, id: &str) -> Result<RunResponse> {
        self.post_json::<RunResponse, serde_json::Value>(
            &format!("/cron/{id}/trigger"),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn submit_webchat(&self, message: InboundMessage) -> Result<RunResponse> {
        self.post_json("/ingress/webchat", &message).await
    }

    pub async fn submit_telegram(&self, update: TelegramUpdate) -> Result<RunResponse> {
        self.post_json("/ingress/telegram", &update).await
    }

    pub async fn subscribe_events(&self) -> Result<GatewayEventStream> {
        let response = self.request(reqwest::Method::GET, "/events").send().await?;
        let status = response.status();
        if !status.is_success() {
            let bytes = response.bytes().await?;
            return Err(decode_error(status, &bytes));
        }

        Ok(GatewayEventStream {
            stream: response.bytes_stream().boxed(),
            buffer: String::new(),
        })
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.request(reqwest::Method::GET, path).send().await?;
        decode_response(response).await
    }

    async fn post_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let response = self
            .request(reqwest::Method::POST, path)
            .json(body)
            .send()
            .await?;
        decode_response(response).await
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut builder = self.http.request(method, self.url(path));
        if let Some(token) = self.bearer_token.as_deref() {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

pub struct GatewayEventStream {
    stream: BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    buffer: String,
}

impl GatewayEventStream {
    pub async fn next_event(&mut self) -> Result<Option<EventStreamEnvelope>> {
        loop {
            if let Some(frame) = next_sse_frame(&mut self.buffer) {
                if let Some(data) = sse_data_payload(&frame) {
                    return Ok(Some(serde_json::from_str(&data)?));
                }
            }

            match self.stream.next().await {
                Some(Ok(chunk)) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                }
                Some(Err(err)) => return Err(err.into()),
                None => {
                    if let Some(frame) = flush_sse_frame(&mut self.buffer) {
                        if let Some(data) = sse_data_payload(&frame) {
                            return Ok(Some(serde_json::from_str(&data)?));
                        }
                    }

                    return Ok(None);
                }
            }
        }
    }
}

async fn decode_response<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let status = response.status();
    let bytes = response.bytes().await?;
    if !status.is_success() {
        return Err(decode_error(status, &bytes));
    }

    Ok(serde_json::from_slice(&bytes)?)
}

fn decode_error(status: reqwest::StatusCode, bytes: &[u8]) -> anyhow::Error {
    if let Ok(error) = serde_json::from_slice::<ErrorResponse>(&bytes) {
        return anyhow::anyhow!(error.error);
    }

    let body = String::from_utf8_lossy(&bytes);
    anyhow::anyhow!("gateway request failed with status {}: {}", status, body)
}

fn next_sse_frame(buffer: &mut String) -> Option<String> {
    let delimiter = buffer.find("\n\n")?;
    let frame = buffer[..delimiter].to_owned();
    buffer.drain(..delimiter + 2);
    Some(frame)
}

fn flush_sse_frame(buffer: &mut String) -> Option<String> {
    let frame = buffer.trim();
    if frame.is_empty() {
        buffer.clear();
        return None;
    }

    let frame = frame.to_owned();
    buffer.clear();
    Some(frame)
}

fn sse_data_payload(frame: &str) -> Option<String> {
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|line| line.trim_start())
        .collect::<Vec<_>>();

    if data.is_empty() {
        None
    } else {
        Some(data.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, sync::Arc};

    use anyhow::Result;
    use mosaic_config::{MosaicConfig, ProviderProfileConfig};
    use mosaic_control_protocol::{GatewayEvent, InboundMessage, IngressTrace, RunSubmission};
    use mosaic_gateway::{GatewayHandle, GatewayRuntimeComponents, http_router};
    use mosaic_memory::{FileMemoryStore, MemoryPolicy};
    use mosaic_node_protocol::FileNodeStore;
    use mosaic_provider::{MockProvider, ProviderProfileRegistry};
    use mosaic_scheduler_core::FileCronStore;
    use mosaic_session_core::FileSessionStore;
    use mosaic_skill_core::SkillRegistry;
    use mosaic_tool_core::{TimeNowTool, ToolRegistry};
    use mosaic_workflow::WorkflowRegistry;
    use tokio::{net::TcpListener, sync::oneshot};

    use super::GatewayClient;

    async fn spawn_gateway() -> Result<(String, oneshot::Sender<()>)> {
        let mut config = MosaicConfig::default();
        config.active_profile = "demo-provider".to_owned();
        config.profiles.insert(
            "demo-provider".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
                transport: Default::default(),
                vendor: Default::default(),
            },
        );

        let profiles = ProviderProfileRegistry::from_config(&config)?;
        let mut tools = ToolRegistry::new();
        tools.register(Arc::new(TimeNowTool::new()));
        let session_root =
            std::env::temp_dir().join(format!("mosaic-sdk-tests-{}", uuid::Uuid::new_v4()));
        let gateway = GatewayHandle::new_local(
            tokio::runtime::Handle::current(),
            GatewayRuntimeComponents {
                profiles: Arc::new(profiles),
                provider_override: Some(Arc::new(MockProvider)),
                session_store: Arc::new(FileSessionStore::new(&session_root)),
                memory_store: Arc::new(FileMemoryStore::new(session_root.join("memory"))),
                memory_policy: MemoryPolicy::default(),
                runtime_policy: config.runtime.clone(),
                tools: Arc::new(tools),
                skills: Arc::new(SkillRegistry::new()),
                workflows: Arc::new(WorkflowRegistry::new()),
                node_store: Arc::new(FileNodeStore::new(session_root.join("nodes"))),
                mcp_manager: None,
                cron_store: Arc::new(FileCronStore::new(session_root.join("cron"))),
                workspace_root: session_root.clone(),
                runs_dir: std::env::temp_dir(),
                audit_root: session_root.join("audit"),
                extensions: Vec::new(),
                policies: mosaic_config::PolicyConfig::default(),
                deployment: config.deployment.clone(),
                auth: config.auth.clone(),
                audit: config.audit.clone(),
                observability: config.observability.clone(),
            },
        );

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr: SocketAddr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let router = http_router(gateway);
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        Ok((format!("http://{addr}"), shutdown_tx))
    }

    #[tokio::test]
    async fn client_can_submit_query_and_subscribe() {
        let (base_url, shutdown) = spawn_gateway().await.expect("gateway should start");
        let client = GatewayClient::new(base_url.clone());

        let health = client.health().await.expect("health should succeed");
        assert_eq!(health.status, "ok");
        assert_eq!(health.transport, "http+sse");

        let mut events = client
            .subscribe_events()
            .await
            .expect("event subscription should succeed");

        let response = client
            .submit_run(RunSubmission {
                system: Some("You are helpful.".to_owned()),
                input: "hello remote".to_owned(),
                skill: None,
                workflow: None,
                session_id: Some("remote-demo".to_owned()),
                profile: None,
                ingress: Some(IngressTrace {
                    kind: "remote_operator".to_owned(),
                    channel: Some("cli".to_owned()),
                    adapter: Some("cli_remote".to_owned()),
                    source: Some("mosaic-cli".to_owned()),
                    remote_addr: None,
                    display_name: None,
                    actor_id: None,
                    conversation_id: None,
                    thread_id: None,
                    thread_title: None,
                    reply_target: None,
                    message_id: None,
                    received_at: None,
                    raw_event_id: None,
                    session_hint: Some("remote-demo".to_owned()),
                    profile_hint: None,
                    gateway_url: Some(base_url.clone()),
                }),
            })
            .await
            .expect("remote run should succeed");

        assert_eq!(response.session_route, "gateway.channel/cli/remote-demo");
        assert_eq!(
            response
                .trace
                .ingress
                .as_ref()
                .map(|trace| trace.kind.as_str()),
            Some("remote_operator")
        );

        let sessions = client
            .list_sessions()
            .await
            .expect("session listing should succeed");
        assert_eq!(sessions.len(), 1);
        let detail = client
            .get_session("remote-demo")
            .await
            .expect("session detail should succeed")
            .expect("session should exist");
        assert_eq!(detail.gateway.route, "gateway.channel/cli/remote-demo");
        assert_eq!(detail.channel_context.channel.as_deref(), Some("cli"));
        assert_eq!(detail.transcript.len(), 3);

        let mut saw_completion = false;
        for _ in 0..12 {
            if let Some(envelope) = events
                .next_event()
                .await
                .expect("event stream should continue")
            {
                match envelope.event {
                    GatewayEvent::RunCompleted { .. } => {
                        saw_completion = true;
                        break;
                    }
                    GatewayEvent::RunUpdated { run } if run.status.is_terminal() => {
                        saw_completion = true;
                        break;
                    }
                    _ => {}
                }
            }
        }

        let _ = shutdown.send(());
        assert!(saw_completion);
    }

    #[tokio::test]
    async fn client_can_submit_webchat_ingress() {
        let (base_url, shutdown) = spawn_gateway().await.expect("gateway should start");
        let client = GatewayClient::new(base_url);

        let response = client
            .submit_webchat(InboundMessage {
                session_id: Some("webchat-demo".to_owned()),
                input: "hello from webchat".to_owned(),
                profile: None,
                display_name: Some("guest".to_owned()),
                actor_id: Some("guest-1".to_owned()),
                conversation_id: None,
                thread_id: Some("room-7".to_owned()),
                thread_title: Some("Launch Room".to_owned()),
                reply_target: Some("webchat:guest-1".to_owned()),
                message_id: None,
                received_at: None,
                raw_event_id: None,
                ingress: None,
            })
            .await
            .expect("webchat ingress should succeed");

        assert_eq!(response.trace.session_id.as_deref(), Some("webchat-demo"));
        assert_eq!(
            response
                .trace
                .ingress
                .as_ref()
                .map(|trace| trace.kind.as_str()),
            Some("webchat_http")
        );

        let _ = shutdown.send(());
    }
}
