use super::*;

#[derive(Debug, Clone)]
pub struct GatewaySandboxStatusView {
    pub base_dir: String,
    pub python_strategy: String,
    pub node_strategy: String,
    pub python_install_enabled: bool,
    pub node_install_enabled: bool,
    pub run_workdirs_after_hours: u64,
    pub attachments_after_hours: u64,
    pub runtime_statuses: Vec<mosaic_sandbox_core::SandboxRuntimeStatus>,
    pub env_count: usize,
}

impl GatewayHandle {
    pub fn sandbox_status(&self) -> Result<GatewaySandboxStatusView> {
        let components = self.snapshot_components();
        let env_count = components.sandbox.list_envs()?.len();
        Ok(GatewaySandboxStatusView {
            base_dir: components.sandbox.paths().root.display().to_string(),
            python_strategy: components
                .sandbox
                .settings()
                .python
                .strategy
                .label()
                .to_owned(),
            node_strategy: components
                .sandbox
                .settings()
                .node
                .strategy
                .label()
                .to_owned(),
            python_install_enabled: components.sandbox.settings().python.install.enabled,
            node_install_enabled: components.sandbox.settings().node.install.enabled,
            run_workdirs_after_hours: components
                .sandbox
                .settings()
                .cleanup
                .run_workdirs_after_hours,
            attachments_after_hours: components
                .sandbox
                .settings()
                .cleanup
                .attachments_after_hours,
            runtime_statuses: components.sandbox.runtime_statuses(),
            env_count,
        })
    }

    pub fn sandbox_list_envs(&self) -> Result<Vec<mosaic_sandbox_core::SandboxEnvRecord>> {
        self.snapshot_components().sandbox.list_envs()
    }

    pub fn sandbox_inspect_env(
        &self,
        env_id: &str,
    ) -> Result<mosaic_sandbox_core::SandboxEnvRecord> {
        self.snapshot_components().sandbox.inspect_env(env_id)
    }

    pub fn sandbox_rebuild_env(
        &self,
        env_id: &str,
    ) -> Result<mosaic_sandbox_core::SandboxEnvRecord> {
        self.snapshot_components().sandbox.rebuild_env(env_id)
    }

    pub fn sandbox_clean(&self) -> Result<mosaic_sandbox_core::SandboxCleanReport> {
        self.snapshot_components().sandbox.clean()
    }
}
