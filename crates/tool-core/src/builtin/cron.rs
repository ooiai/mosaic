use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use mosaic_scheduler_core::{CronRegistration, CronStore};

use crate::{CapabilityAudit, CapabilityKind, CapabilityMetadata, Tool, ToolMetadata, ToolResult};

pub struct CronRegisterTool {
    meta: ToolMetadata,
    store: Arc<dyn CronStore>,
}

impl CronRegisterTool {
    pub fn new(store: Arc<dyn CronStore>) -> Self {
        Self {
            meta: ToolMetadata::builtin(
                "cron_register",
                "Register a cron job in the local gateway scheduler",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "schedule": { "type": "string" },
                        "input": { "type": "string" },
                        "session_id": { "type": "string" },
                        "profile": { "type": "string" },
                        "skill": { "type": "string" },
                        "workflow": { "type": "string" }
                    },
                    "required": ["id", "schedule", "input"]
                }),
            )
            .with_capability(CapabilityMetadata::cron()),
            store,
        }
    }
}

#[async_trait]
impl Tool for CronRegisterTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.meta
    }

    async fn call(&self, input: serde_json::Value) -> Result<ToolResult> {
        let id = input
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: id"))?
            .to_owned();
        let schedule = input
            .get("schedule")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: schedule"))?
            .to_owned();
        let message = input
            .get("input")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("missing required field: input"))?
            .to_owned();

        let mut registration = CronRegistration::new(id.clone(), schedule.clone(), message.clone());
        registration.session_id = input
            .get("session_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        registration.profile = input
            .get("profile")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        registration.skill = input
            .get("skill")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        registration.workflow = input
            .get("workflow")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        self.store.save(&registration)?;

        Ok(ToolResult {
            content: format!("registered cron {}", id),
            structured: Some(serde_json::to_value(&registration)?),
            is_error: false,
            audit: Some(CapabilityAudit {
                kind: CapabilityKind::Cron,
                permission_scopes: self.meta.capability.permission_scopes.clone(),
                risk: self.meta.capability.risk.clone(),
                side_effect_summary: format!("registered cron {} on {}", id, schedule),
                target: Some(id),
                exit_code: None,
                http_status: None,
            }),
        })
    }
}
