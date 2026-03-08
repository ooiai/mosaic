use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
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
            let detail = gateway_error_detail(&body);
            return Err(MosaicError::GatewayUnavailable(format!(
                "gateway probe failed with HTTP {status}{}",
                format_gateway_detail_suffix(detail.as_deref()),
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
            let detail = gateway_error_detail(&body);
            return Err(MosaicError::GatewayUnavailable(format!(
                "gateway discover failed with HTTP {status}{}",
                format_gateway_detail_suffix(detail.as_deref()),
            )));
        }

        let methods = parse_discovery_methods(&body)?;

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
            let detail = gateway_error_detail(&body);
            return Err(MosaicError::GatewayUnavailable(format!(
                "gateway call failed with HTTP {status}{}",
                format_gateway_detail_suffix(detail.as_deref()),
            )));
        }

        let response = parse_gateway_call_response(body, &request.method)?;
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

fn parse_discovery_methods(body: &Value) -> Result<Vec<String>> {
    let methods = body
        .get("methods")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            MosaicError::GatewayProtocol(
                "gateway discover response missing methods array".to_string(),
            )
        })?;

    let mut method_names = Vec::with_capacity(methods.len());
    let mut seen = BTreeSet::new();
    for (index, entry) in methods.iter().enumerate() {
        let method = entry.as_str().ok_or_else(|| {
            MosaicError::GatewayProtocol(format!(
                "gateway discover methods[{index}] must be a string"
            ))
        })?;
        let normalized = method.trim();
        if normalized.is_empty() {
            return Err(MosaicError::GatewayProtocol(format!(
                "gateway discover methods[{index}] must not be empty"
            )));
        }
        if seen.insert(normalized.to_string()) {
            method_names.push(normalized.to_string());
        }
    }

    Ok(method_names)
}

fn parse_gateway_call_response(body: Value, method: &str) -> Result<GatewayResponse> {
    let mut response: GatewayResponse = serde_json::from_value(body).map_err(|err| {
        MosaicError::GatewayProtocol(format!("invalid gateway call response: {err}"))
    })?;
    if response.ok {
        if response.error.is_some() {
            return Err(MosaicError::GatewayProtocol(format!(
                "gateway method '{method}' returned ok=true with error payload"
            )));
        }
        if response.result.is_none() {
            response.result = Some(Value::Null);
        }
        return Ok(response);
    }

    let Some(err) = response.error.as_ref() else {
        return Err(MosaicError::GatewayProtocol(format!(
            "gateway method '{method}' failed without error payload"
        )));
    };
    if err.code.trim().is_empty() || err.message.trim().is_empty() {
        return Err(MosaicError::GatewayProtocol(format!(
            "gateway method '{method}' returned malformed error payload"
        )));
    }
    Ok(response)
}

fn gateway_error_detail(body: &Value) -> Option<String> {
    let object = body.as_object()?;
    if let Some(error) = object.get("error") {
        if let Some(message) = error.as_str() {
            let trimmed = message.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Some(error_object) = error.as_object() {
            let code = error_object
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("");
            let message = error_object
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("");
            let combined = if !code.trim().is_empty() && !message.trim().is_empty() {
                format!("{} ({})", message.trim(), code.trim())
            } else if !message.trim().is_empty() {
                message.trim().to_string()
            } else if !code.trim().is_empty() {
                code.trim().to_string()
            } else {
                String::new()
            };
            if !combined.is_empty() {
                return Some(combined);
            }
        }
    }
    object
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn format_gateway_detail_suffix(detail: Option<&str>) -> String {
    detail.map(|value| format!(": {value}")).unwrap_or_default()
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

    #[test]
    fn parse_discovery_methods_rejects_invalid_entries() {
        let err = parse_discovery_methods(&json!({
            "methods": ["status", 1]
        }))
        .expect_err("should fail");
        assert!(err.to_string().contains("methods[1] must be a string"));
    }

    #[test]
    fn parse_gateway_call_response_validates_error_payload() {
        let err = parse_gateway_call_response(
            json!({
                "ok": false,
                "error": {"code": "", "message": "broken"}
            }),
            "status",
        )
        .expect_err("should fail");
        assert!(err.to_string().contains("returned malformed error payload"));
    }
}
