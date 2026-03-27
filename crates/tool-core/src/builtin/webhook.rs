use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use reqwest::{
    Client, Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};

use crate::{CapabilityAudit, CapabilityKind, CapabilityMetadata, Tool, ToolMetadata, ToolResult};

pub struct WebhookTool {
    meta: ToolMetadata,
    client: Client,
}

impl WebhookTool {
    pub fn new() -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "webhook_call",
                "Dispatch an outbound HTTP webhook request",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" },
                        "method": { "type": "string" },
                        "body": { "type": "string" },
                        "headers": {
                            "type": "object",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["url"]
                }),
            )
            .with_capability(CapabilityMetadata::webhook()),
            client: Client::new(),
        }
    }
}

impl Default for WebhookTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebhookTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let url = input
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: url"))?
            .to_owned();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("webhook url must start with http:// or https://");
        }

        let method = input
            .get("method")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("POST")
            .parse::<Method>()?;
        let mut headers = HeaderMap::new();
        if let Some(raw_headers) = input.get("headers").and_then(serde_json::Value::as_object) {
            for (name, value) in raw_headers {
                let value = value
                    .as_str()
                    .ok_or_else(|| anyhow!("webhook header values must be strings"))?;
                headers.insert(
                    HeaderName::from_bytes(name.as_bytes())?,
                    HeaderValue::from_str(value)?,
                );
            }
        }
        let body = input
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();

        let response = self
            .client
            .request(method.clone(), &url)
            .headers(headers)
            .body(body.clone())
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        Ok(ToolResult {
            content: text.clone(),
            structured: Some(serde_json::json!({
                "url": url,
                "method": method.as_str(),
                "status": status.as_u16(),
                "body": text,
                "request_body": body,
            })),
            is_error: !status.is_success(),
            audit: Some(CapabilityAudit {
                kind: CapabilityKind::Webhook,
                permission_scopes: self.meta.capability.permission_scopes.clone(),
                risk: self.meta.capability.risk.clone(),
                side_effect_summary: format!(
                    "webhook {} {} -> {}",
                    method.as_str(),
                    url,
                    status.as_u16()
                ),
                target: Some(url),
                exit_code: None,
                http_status: Some(status.as_u16()),
            }),
        })
    }
}
