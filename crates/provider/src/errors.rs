use serde::{Deserialize, Serialize};

use crate::types::ProviderAttempt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Auth,
    Timeout,
    RateLimited,
    Unavailable,
    InvalidRequest,
    Transport,
    Response,
    Unsupported,
    Unknown,
}

impl ProviderErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Timeout => "timeout",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::InvalidRequest => "invalid_request",
            Self::Transport => "transport",
            Self::Response => "response",
            Self::Unsupported => "unsupported",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub provider_type: String,
    pub profile_name: String,
    pub model: String,
    pub message: String,
    pub public_message: String,
    pub status_code: Option<u16>,
    pub retryable: bool,
    #[serde(default)]
    pub attempts: Vec<ProviderAttempt>,
}

impl ProviderError {
    pub(crate) fn new(
        kind: ProviderErrorKind,
        provider_type: impl Into<String>,
        profile_name: impl Into<String>,
        model: impl Into<String>,
        message: impl Into<String>,
        status_code: Option<u16>,
        retryable: bool,
    ) -> Self {
        let provider_type = provider_type.into();
        let profile_name = profile_name.into();
        let model = model.into();
        let message = redact_provider_message(&message.into());
        let mut public_message = match kind {
            ProviderErrorKind::Auth => format!(
                "{} provider authentication failed for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Timeout => format!(
                "{} provider request timed out for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::RateLimited => format!(
                "{} provider rate limit reached for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Unavailable => format!(
                "{} provider is temporarily unavailable for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::InvalidRequest => format!(
                "{} provider rejected the request for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Transport => format!(
                "{} provider transport failed for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Response => format!(
                "{} provider returned an invalid response for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Unsupported => format!(
                "{} provider is not supported for profile '{}'",
                provider_type, profile_name
            ),
            ProviderErrorKind::Unknown => format!(
                "{} provider request failed for profile '{}'",
                provider_type, profile_name
            ),
        };
        if let Some(status_code) = status_code {
            public_message.push_str(&format!(" (status {status_code})"));
        }

        Self {
            kind,
            provider_type,
            profile_name,
            model,
            message,
            public_message,
            status_code,
            retryable,
            attempts: Vec::new(),
        }
    }

    pub fn public_message(&self) -> &str {
        &self.public_message
    }

    pub fn kind_label(&self) -> &'static str {
        self.kind.as_str()
    }

    pub(crate) fn with_attempts(mut self, attempts: Vec<ProviderAttempt>) -> Self {
        self.attempts = attempts;
        self
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.public_message)
    }
}

impl std::error::Error for ProviderError {}

pub fn public_error_message(error: &anyhow::Error) -> String {
    if let Some(provider_error) = error.downcast_ref::<ProviderError>() {
        return provider_error.public_message().to_owned();
    }

    redact_provider_message(&error.to_string())
}

pub(crate) fn redact_provider_message(message: &str) -> String {
    let mut redacted = message.to_owned();
    for prefix in ["Bearer ", "api-key: ", "x-api-key: "] {
        redacted = redact_after_prefix(&redacted, prefix, &[' ', '\n', '\r', '\t', ',', ';']);
    }
    for prefix in [
        "\"api-key\":\"",
        "\"x-api-key\":\"",
        "\"authorization\":\"Bearer ",
    ] {
        redacted = redact_after_prefix(&redacted, prefix, &['\"']);
    }
    redacted
}

fn redact_after_prefix(value: &str, prefix: &str, terminators: &[char]) -> String {
    let remaining = value;
    let mut output = String::new();
    loop {
        let Some(index) = remaining.find(prefix) else {
            output.push_str(remaining);
            break;
        };
        let start = index + prefix.len();
        output.push_str(&remaining[..start]);
        let tail = &remaining[start..];
        let end = tail
            .find(|ch| terminators.contains(&ch))
            .unwrap_or(tail.len());
        output.push_str("<redacted>");
        output.push_str(&tail[end..]);
        break;
    }
    output
}
