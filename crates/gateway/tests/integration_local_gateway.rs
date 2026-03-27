mod support;

use std::fs;

use mosaic_control_protocol::RunSubmission;
use mosaic_gateway::GatewayHandle;
use tokio::runtime::Handle;

#[tokio::test]
async fn local_gateway_submits_run_and_persists_session_state() {
    let root = support::temp_dir("local");
    fs::create_dir_all(&root).expect("temp root should exist");
    let gateway = GatewayHandle::new_local(Handle::current(), support::build_components(&root));

    let submitted = gateway
        .submit_run(RunSubmission {
            system: None,
            input: "What time is it right now?".to_owned(),
            skill: None,
            workflow: None,
            session_id: Some("gateway-demo".to_owned()),
            profile: Some("demo-provider".to_owned()),
            ingress: None,
        })
        .expect("run should submit");
    let result = submitted.wait().await.expect("run should finish");

    assert!(result.output.contains("current time"));
    assert_eq!(result.trace.session_id.as_deref(), Some("gateway-demo"));

    let session = gateway
        .load_session("gateway-demo")
        .expect("session load should succeed")
        .expect("session should exist");
    assert_eq!(session.id, "gateway-demo");
    assert_eq!(session.run.status.label(), "success");

    fs::remove_dir_all(root).ok();
}
