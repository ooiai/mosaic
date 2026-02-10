use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use mosaic_core::error::{MosaicError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl GatewayRequest {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            method: method.into(),
            params: params.unwrap_or(Value::Null),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<GatewayErrorPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayProbe {
    pub ok: bool,
    pub endpoint: String,
    pub latency_ms: u128,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayDiscovery {
    pub ok: bool,
    pub endpoint: String,
    pub methods: Vec<String>,
}

#[async_trait]
pub trait GatewayClient: Send + Sync {
    async fn probe(&self) -> Result<GatewayProbe>;
    async fn discover(&self) -> Result<GatewayDiscovery>;
    async fn call(&self, request: GatewayRequest) -> Result<GatewayResponse>;
}

#[derive(Debug, Clone)]
pub struct HttpGatewayClient {
    base_url: String,
    client: reqwest::Client,
}

impl HttpGatewayClient {
    pub fn new(host: &str, port: u16) -> Result<Self> {
        let base_url = format!("http://{host}:{port}");
        Self::with_base_url(base_url)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .build()
            .map_err(|err| {
                MosaicError::GatewayUnavailable(format!("failed to build gateway client: {err}"))
            })?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn get_json(&self, path: &str) -> Result<(u16, Value)> {
        let url = self.endpoint(path);
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(map_network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|err| {
            MosaicError::GatewayProtocol(format!("failed to read gateway response body: {err}"))
        })?;
        let parsed = if body.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str::<Value>(&body).map_err(|err| {
                MosaicError::GatewayProtocol(format!("gateway returned invalid JSON: {err}"))
            })?
        };
        Ok((status, parsed))
    }

    async fn post_json(&self, path: &str, payload: &impl Serialize) -> Result<(u16, Value)> {
        let url = self.endpoint(path);
        let response = self
            .client
            .post(url)
            .json(payload)
            .send()
            .await
            .map_err(map_network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|err| {
            MosaicError::GatewayProtocol(format!("failed to read gateway response body: {err}"))
        })?;
        let parsed = if body.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str::<Value>(&body).map_err(|err| {
                MosaicError::GatewayProtocol(format!("gateway returned invalid JSON: {err}"))
            })?
        };
        Ok((status, parsed))
    }
}

#[async_trait]
impl GatewayClient for HttpGatewayClient {
    async fn probe(&self) -> Result<GatewayProbe> {
        let started = Instant::now();
        let endpoint = self.endpoint("/health");
        let (status, body) = self.get_json("/health").await?;
        if !(200..300).contains(&status) {
            return Err(MosaicError::GatewayUnavailable(format!(
                "gateway probe failed with HTTP {status}"
            )));
        }
        Ok(GatewayProbe {
            ok: true,
            endpoint,
            latency_ms: started.elapsed().as_millis(),
            detail: body
                .get("service")
                .and_then(Value::as_str)
                .map(|service| format!("service={service}"))
                .unwrap_or_else(|| "gateway health endpoint reachable".to_string()),
        })
    }

    async fn discover(&self) -> Result<GatewayDiscovery> {
        let endpoint = self.endpoint("/discover");
        let (status, body) = self.get_json("/discover").await?;

        if status == 404 {
            return Ok(GatewayDiscovery {
                ok: true,
                endpoint,
                methods: vec!["health".to_string(), "status".to_string()],
            });
        }
        if !(200..300).contains(&status) {
            return Err(MosaicError::GatewayUnavailable(format!(
                "gateway discover failed with HTTP {status}"
            )));
        }

        let methods = body
            .get("methods")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                MosaicError::GatewayProtocol(
                    "gateway discover response missing methods array".to_string(),
                )
            })?
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        Ok(GatewayDiscovery {
            ok: true,
            endpoint,
            methods,
        })
    }

    async fn call(&self, request: GatewayRequest) -> Result<GatewayResponse> {
        let (status, body) = self.post_json("/call", &request).await?;
        if status == 404 {
            return call_legacy_method(self, &request).await;
        }
        if !(200..300).contains(&status) {
            return Err(MosaicError::GatewayUnavailable(format!(
                "gateway call failed with HTTP {status}"
            )));
        }

        let response: GatewayResponse = serde_json::from_value(body).map_err(|err| {
            MosaicError::GatewayProtocol(format!("invalid gateway call response: {err}"))
        })?;
        if !response.ok {
            if let Some(err) = &response.error {
                return Err(MosaicError::GatewayProtocol(format!(
                    "gateway method '{}' failed: {} ({})",
                    request.method, err.message, err.code
                )));
            }
            return Err(MosaicError::GatewayProtocol(format!(
                "gateway method '{}' failed without error payload",
                request.method
            )));
        }
        Ok(response)
    }
}

async fn call_legacy_method(
    client: &HttpGatewayClient,
    request: &GatewayRequest,
) -> Result<GatewayResponse> {
    let Some(path) = legacy_method_path(&request.method) else {
        return Err(MosaicError::GatewayProtocol(format!(
            "gateway /call endpoint unavailable and method '{}' has no legacy mapping",
            request.method
        )));
    };

    let (status, body) = client.get_json(path).await?;
    if !(200..300).contains(&status) {
        return Err(MosaicError::GatewayUnavailable(format!(
            "legacy gateway method '{}' failed with HTTP {status}",
            request.method
        )));
    }

    Ok(GatewayResponse {
        ok: true,
        result: Some(body),
        error: None,
    })
}

fn legacy_method_path(method: &str) -> Option<&'static str> {
    match method.trim().to_lowercase().as_str() {
        "health" => Some("/health"),
        "status" => Some("/status"),
        _ => None,
    }
}

fn map_network_error(err: reqwest::Error) -> MosaicError {
    if err.is_timeout() {
        return MosaicError::GatewayUnavailable("gateway request timed out".to_string());
    }
    MosaicError::GatewayUnavailable(format!("gateway request failed: {err}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn gateway_request_serializes() {
        let request = GatewayRequest::new("status", Some(json!({"verbose": true})));
        let encoded = serde_json::to_value(&request).expect("serialize request");
        assert_eq!(encoded["method"], "status");
        assert_eq!(encoded["params"]["verbose"], true);
        assert!(encoded["id"].is_string());
    }

    #[test]
    fn gateway_response_round_trip() {
        let value = json!({
            "ok": true,
            "result": { "service": "mosaic-gateway" }
        });
        let parsed: GatewayResponse = serde_json::from_value(value.clone()).expect("parse");
        assert!(parsed.ok);
        let encoded = serde_json::to_value(parsed).expect("serialize");
        assert_eq!(encoded, value);
    }
}
