mod approvals;
mod logs;
mod sandbox;
mod system;

pub use approvals::{
    ApprovalDecision, ApprovalMode, ApprovalPolicy, ApprovalStore, evaluate_approval,
};
pub use logs::{UnifiedLogEntry, collect_logs};
pub use sandbox::{
    SandboxPolicy, SandboxProfile, SandboxProfileInfo, SandboxStore, evaluate_sandbox,
    list_profiles, profile_info,
};
pub use system::{
    PresenceSnapshot, SystemEvent, SystemEventStore, snapshot_presence, system_events_path,
};

#[derive(Debug, Clone)]
pub struct RuntimePolicy {
    pub approval: ApprovalPolicy,
    pub sandbox: SandboxPolicy,
}
