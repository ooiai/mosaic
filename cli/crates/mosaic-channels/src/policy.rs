use std::time::Duration;

const DEFAULT_RETRY_BACKOFF_MS: [u64; 3] = [200, 500, 1000];
pub const DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub timeout: Duration,
    pub backoff_ms: Vec<u64>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS),
            backoff_ms: DEFAULT_RETRY_BACKOFF_MS.to_vec(),
        }
    }
}

impl RetryPolicy {
    pub fn from_env() -> Self {
        let timeout_ms = std::env::var("MOSAIC_CHANNELS_HTTP_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_HTTP_TIMEOUT_MS);

        Self {
            timeout: Duration::from_millis(timeout_ms),
            backoff_ms: DEFAULT_RETRY_BACKOFF_MS.to_vec(),
        }
    }

    pub fn max_attempts(&self) -> usize {
        self.backoff_ms.len() + 1
    }

    pub fn backoff_before_attempt(&self, attempt_index: usize) -> Option<Duration> {
        if attempt_index == 0 {
            return None;
        }
        self.backoff_ms
            .get(attempt_index - 1)
            .copied()
            .map(Duration::from_millis)
    }
}

pub(crate) fn should_retry_http_status(status: u16) -> bool {
    (500..600).contains(&status)
}
