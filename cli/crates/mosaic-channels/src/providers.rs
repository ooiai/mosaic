use serde_json::{Value, json};

use mosaic_core::error::{MosaicError, Result};

use crate::policy::{RetryPolicy, should_retry_http_status};
use crate::schema::mask_endpoint;

#[derive(Debug, Clone)]
pub(crate) struct DeliveryAttemptResult {
    pub ok: bool,
    pub attempts: usize,
    pub http_status: Option<u16>,
    pub error: Option<String>,
    pub endpoint_masked: Option<String>,
}

pub(crate) fn local_delivery_success() -> DeliveryAttemptResult {
    DeliveryAttemptResult {
        ok: true,
        attempts: 1,
        http_status: Some(200),
        error: None,
        endpoint_masked: None,
    }
}

pub(crate) async fn send_slack_webhook(
    endpoint: &str,
    text: &str,
    bearer_token: Option<String>,
    policy: &RetryPolicy,
) -> Result<DeliveryAttemptResult> {
    send_with_retry(endpoint, json!({ "text": text }), bearer_token, policy).await
}

pub(crate) async fn send_webhook(
    endpoint: &str,
    payload: Value,
    bearer_token: Option<String>,
    policy: &RetryPolicy,
) -> Result<DeliveryAttemptResult> {
    send_with_retry(endpoint, payload, bearer_token, policy).await
}

async fn send_with_retry(
    endpoint: &str,
    payload: Value,
    bearer_token: Option<String>,
    policy: &RetryPolicy,
) -> Result<DeliveryAttemptResult> {
    if endpoint.starts_with("mock-http://") {
        return simulate_mock_http(endpoint, policy).await;
    }

    let client = reqwest::Client::builder()
        .timeout(policy.timeout)
        .build()
        .map_err(|err| MosaicError::Network(format!("failed to build HTTP client: {err}")))?;

    let mut attempts = 0usize;
    let mut last_error: Option<String> = None;
    let mut last_status: Option<u16> = None;

    for attempt_idx in 0..policy.max_attempts() {
        attempts += 1;
        if let Some(delay) = policy.backoff_before_attempt(attempt_idx) {
            tokio::time::sleep(delay).await;
        }

        let mut request = client.post(endpoint).json(&payload);
        if let Some(token) = &bearer_token {
            request = request.bearer_auth(token);
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                last_status = Some(status);
                if response.status().is_success() {
                    return Ok(DeliveryAttemptResult {
                        ok: true,
                        attempts,
                        http_status: Some(status),
                        error: None,
                        endpoint_masked: Some(mask_endpoint(endpoint)),
                    });
                }

                if response.status().is_client_error() {
                    return Ok(DeliveryAttemptResult {
                        ok: false,
                        attempts,
                        http_status: Some(status),
                        error: Some(format!("webhook returned client error status {status}")),
                        endpoint_masked: Some(mask_endpoint(endpoint)),
                    });
                }

                last_error = Some(format!("webhook returned server error status {status}"));
                if !should_retry_http_status(status) || attempt_idx + 1 >= policy.max_attempts() {
                    break;
                }
            }
            Err(err) => {
                last_error = Some(if err.is_timeout() {
                    "webhook request timed out".to_string()
                } else {
                    format!("webhook request failed: {err}")
                });
                if attempt_idx + 1 >= policy.max_attempts() {
                    break;
                }
            }
        }
    }

    Ok(DeliveryAttemptResult {
        ok: false,
        attempts,
        http_status: last_status,
        error: Some(
            last_error.unwrap_or_else(|| "webhook request failed after retries".to_string()),
        ),
        endpoint_masked: Some(mask_endpoint(endpoint)),
    })
}

async fn simulate_mock_http(endpoint: &str, policy: &RetryPolicy) -> Result<DeliveryAttemptResult> {
    let sequence = endpoint.trim_start_matches("mock-http://");
    if sequence.trim().is_empty() {
        return Ok(DeliveryAttemptResult {
            ok: true,
            attempts: 1,
            http_status: Some(200),
            error: None,
            endpoint_masked: Some(mask_endpoint(endpoint)),
        });
    }

    let steps = sequence
        .split(',')
        .map(|value| value.trim().to_lowercase())
        .collect::<Vec<_>>();

    let mut attempts = 0usize;
    let mut last_status: Option<u16> = None;
    let mut last_error: Option<String> = None;

    for (idx, step) in steps.iter().enumerate() {
        attempts += 1;
        if let Some(delay) = policy.backoff_before_attempt(idx) {
            tokio::time::sleep(delay).await;
        }

        if step == "timeout" {
            last_error = Some("webhook request timed out".to_string());
            continue;
        }

        let status = step.parse::<u16>().map_err(|_| {
            MosaicError::Validation(format!(
                "invalid mock-http response step '{}' in endpoint {}",
                step, endpoint
            ))
        })?;

        last_status = Some(status);
        if (200..300).contains(&status) {
            return Ok(DeliveryAttemptResult {
                ok: true,
                attempts,
                http_status: Some(status),
                error: None,
                endpoint_masked: Some(mask_endpoint(endpoint)),
            });
        }

        if (400..500).contains(&status) {
            return Ok(DeliveryAttemptResult {
                ok: false,
                attempts,
                http_status: Some(status),
                error: Some(format!("webhook returned client error status {status}")),
                endpoint_masked: Some(mask_endpoint(endpoint)),
            });
        }

        last_error = Some(format!("webhook returned server error status {status}"));
    }

    Ok(DeliveryAttemptResult {
        ok: false,
        attempts,
        http_status: last_status,
        error: Some(last_error.unwrap_or_else(|| "mock-http failed".to_string())),
        endpoint_masked: Some(mask_endpoint(endpoint)),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn mock_http_simulates_retry_and_success() {
        let policy = RetryPolicy {
            timeout: Duration::from_millis(10),
            backoff_ms: vec![1, 1, 1],
        };
        let result = send_webhook(
            "mock-http://500,500,200",
            json!({"text":"hello"}),
            None,
            &policy,
        )
        .await
        .expect("send");
        assert!(result.ok);
        assert_eq!(result.attempts, 3);
        assert_eq!(result.http_status, Some(200));
    }
}
