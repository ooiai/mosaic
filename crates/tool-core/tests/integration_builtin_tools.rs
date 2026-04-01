use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use mosaic_scheduler_core::{CronStore, FileCronStore};
use mosaic_tool_core::{CronRegisterTool, ExecTool, ReadFileTool, Tool, ToolContext, WebhookTool};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mosaic-tool-core-integration-{label}-{}-{nanos}-{count}",
        process::id()
    ))
}

fn spawn_http_server(body: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener addr should exist");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request should arrive");
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("response should write");
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn builtin_tools_execute_against_real_files_processes_and_http() {
    let dir = temp_dir("tools");
    fs::create_dir_all(&dir).expect("temp dir should exist");
    let file_path = dir.join("note.txt");
    fs::write(&file_path, "hello from tool-core").expect("file should write");

    let read_file = ReadFileTool::new_with_allowed_roots(vec![dir.clone()]);
    let read_result = read_file
        .call(
            serde_json::json!({ "path": file_path.display().to_string() }),
            &ToolContext::default(),
        )
        .await
        .expect("read_file should succeed");
    assert_eq!(read_result.content, "hello from tool-core");

    let exec = ExecTool::new(vec![dir.clone()]);
    let exec_result = exec
        .call(
            serde_json::json!({ "command": "printf", "args": ["tool-core-exec"] }),
            &ToolContext::default(),
        )
        .await
        .expect("exec should succeed");
    assert!(exec_result.content.contains("tool-core-exec"));

    let (url, server) = spawn_http_server("webhook-ok");
    let webhook = WebhookTool::new();
    let webhook_result = webhook
        .call(
            serde_json::json!({
                "url": url,
                "method": "POST",
                "body": "hello"
            }),
            &ToolContext::default(),
        )
        .await
        .expect("webhook should succeed");
    server
        .join()
        .expect("webhook server thread should complete");
    assert_eq!(webhook_result.content, "webhook-ok");

    let cron_store = std::sync::Arc::new(FileCronStore::new(dir.join("cron")));
    let cron = CronRegisterTool::new(cron_store.clone());
    cron.call(
        serde_json::json!({
            "id": "nightly",
            "schedule": "0 1 * * *",
            "input": "run report"
        }),
        &ToolContext::default(),
    )
    .await
    .expect("cron registration should succeed");
    assert!(
        cron_store
            .load("nightly")
            .expect("cron load should succeed")
            .is_some()
    );

    fs::remove_dir_all(dir).ok();
}
