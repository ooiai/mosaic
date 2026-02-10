use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditSummary {
    pub ok: bool,
    pub findings: usize,
}

impl Default for SecurityAuditSummary {
    fn default() -> Self {
        Self {
            ok: true,
            findings: 0,
        }
    }
}
