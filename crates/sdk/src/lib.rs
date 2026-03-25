use anyhow::Result;
use futures::{StreamExt, stream::BoxStream};
use mosaic_control_protocol::{
    ErrorResponse, EventStreamEnvelope, HealthResponse, InboundMessage, RunResponse, RunSubmission,
    SessionDetailDto, SessionSummaryDto,
};

#[derive(Clone)]
pub struct GatewayClient {
    base_url: String,
    http: reqwest::Client,
}

impl GatewayClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        self.get_json("/health").await
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionSummaryDto>> {
        self.get_json("/sessions").await
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<SessionDetailDto>> {
        let response = self
            .http
            .get(self.url(&format!("/sessions/{id}")))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        Ok(Some(decode_response(response).await?))
    }

    pub async fn submit_run(&self, submission: RunSubmission) -> Result<RunResponse> {
        self.post_json("/runs", &submission).await
    }

    pub async fn submit_webchat(&self, message: InboundMessage) -> Result<RunResponse> {
        self.post_json("/ingress/webchat", &message).await
    }

    pub async fn subscribe_events(&self) -> Result<GatewayEventStream> {
        let response = self.http.get(self.url("/events")).send().await?;
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
        let response = self.http.get(self.url(path)).send().await?;
        decode_response(response).await
    }

    async fn post_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let response = self.http.post(self.url(path)).json(body).send().await?;
        decode_response(response).await
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
    use mosaic_provider::{MockProvider, ProviderProfileRegistry};
    use mosaic_session_core::FileSessionStore;
    use mosaic_skill_core::SkillRegistry;
    use mosaic_tool_core::{TimeNowTool, ToolRegistry};
    use mosaic_workflow::WorkflowRegistry;
    use tokio::{net::TcpListener, sync::oneshot};

    use super::GatewayClient;

    async fn spawn_gateway() -> Result<(String, oneshot::Sender<()>)> {
        let mut config = MosaicConfig::default();
        config.active_profile = "mock".to_owned();
        config.profiles.insert(
            "mock".to_owned(),
            ProviderProfileConfig {
                provider_type: "mock".to_owned(),
                model: "mock".to_owned(),
                base_url: None,
                api_key_env: None,
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
                tools: Arc::new(tools),
                skills: Arc::new(SkillRegistry::new()),
                workflows: Arc::new(WorkflowRegistry::new()),
                mcp_manager: None,
                runs_dir: std::env::temp_dir(),
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
                    source: Some("mosaic-cli".to_owned()),
                    remote_addr: None,
                    display_name: None,
                    gateway_url: Some(base_url.clone()),
                }),
            })
            .await
            .expect("remote run should succeed");

        assert_eq!(response.session_route, "gateway.local/remote-demo");
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
        assert_eq!(detail.transcript.len(), 3);

        let mut saw_completion = false;
        for _ in 0..8 {
            if let Some(envelope) = events
                .next_event()
                .await
                .expect("event stream should continue")
            {
                if matches!(envelope.event, GatewayEvent::RunCompleted { .. }) {
                    saw_completion = true;
                    break;
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
            Some("webchat")
        );

        let _ = shutdown.send(());
    }
}
