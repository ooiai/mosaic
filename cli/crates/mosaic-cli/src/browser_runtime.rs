use std::time::Duration;

use chrono::{DateTime, Utc};

use mosaic_core::error::MosaicError;

use super::{BrowserVisitRecord, Result, generate_browser_visit_id};
use crate::utils::{extract_html_title, preview_text};

pub(super) async fn browser_open_visit(url: &str, timeout_ms: u64) -> Result<BrowserVisitRecord> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|err| MosaicError::Validation(format!("invalid browser url '{}': {err}", url)))?;
    let visit_id = generate_browser_visit_id();
    let ts = Utc::now();
    match parsed.scheme() {
        "mock" => Ok(browser_open_mock_visit(visit_id, ts, url, &parsed)),
        "http" | "https" => {
            Ok(browser_open_http_visit(visit_id, ts, url, &parsed, timeout_ms).await)
        }
        scheme => Err(MosaicError::Validation(format!(
            "unsupported browser url scheme '{}', expected http/https/mock",
            scheme
        ))),
    }
}

fn browser_open_mock_visit(
    visit_id: String,
    ts: DateTime<Utc>,
    url: &str,
    parsed: &reqwest::Url,
) -> BrowserVisitRecord {
    let status = resolve_mock_browser_status(parsed);
    let title = parsed
        .query_pairs()
        .find(|(key, _)| key == "title")
        .map(|(_, value)| value.to_string())
        .or_else(|| Some("Mock Page".to_string()));
    let body = format!(
        "<html><head><title>{}</title></head><body>mock browser response status {status}</body></html>",
        title.clone().unwrap_or_else(|| "Mock Page".to_string())
    );
    let ok = (200..300).contains(&status);
    BrowserVisitRecord {
        id: visit_id,
        ts,
        url: url.to_string(),
        ok,
        http_status: Some(status),
        title,
        content_type: Some("text/html; charset=utf-8".to_string()),
        content_length: Some(body.len()),
        preview: preview_text(&body, 240),
        error: if ok {
            None
        } else {
            Some(format!("http status {status}"))
        },
    }
}

fn resolve_mock_browser_status(parsed: &reqwest::Url) -> u16 {
    if let Some(host) = parsed.host_str() {
        if host.eq_ignore_ascii_case("ok") {
            return 200;
        }
        if let Ok(code) = host.parse::<u16>() {
            return code;
        }
        if host.eq_ignore_ascii_case("status") {
            let path = parsed.path().trim_start_matches('/');
            if let Ok(code) = path.parse::<u16>() {
                return code;
            }
        }
    }
    200
}

async fn browser_open_http_visit(
    visit_id: String,
    ts: DateTime<Utc>,
    url: &str,
    parsed: &reqwest::Url,
    timeout_ms: u64,
) -> BrowserVisitRecord {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return BrowserVisitRecord {
                id: visit_id,
                ts,
                url: url.to_string(),
                ok: false,
                http_status: None,
                title: None,
                content_type: None,
                content_length: None,
                preview: None,
                error: Some(format!("failed to build http client: {err}")),
            };
        }
    };

    let response = match client.get(parsed.clone()).send().await {
        Ok(response) => response,
        Err(err) => {
            return BrowserVisitRecord {
                id: visit_id,
                ts,
                url: url.to_string(),
                ok: false,
                http_status: None,
                title: None,
                content_type: None,
                content_length: None,
                preview: None,
                error: Some(format!("request failed: {err}")),
            };
        }
    };

    let status = response.status().as_u16();
    let ok = response.status().is_success();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let body = match response.text().await {
        Ok(body) => body,
        Err(err) => {
            return BrowserVisitRecord {
                id: visit_id,
                ts,
                url: url.to_string(),
                ok: false,
                http_status: Some(status),
                title: None,
                content_type,
                content_length: None,
                preview: None,
                error: Some(format!("failed to read response body: {err}")),
            };
        }
    };
    let title = extract_html_title(&body);
    BrowserVisitRecord {
        id: visit_id,
        ts,
        url: url.to_string(),
        ok,
        http_status: Some(status),
        title,
        content_type,
        content_length: Some(body.len()),
        preview: preview_text(&body, 240),
        error: if ok {
            None
        } else {
            Some(format!("http status {status}"))
        },
    }
}
