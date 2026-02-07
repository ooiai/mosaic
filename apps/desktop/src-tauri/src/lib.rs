use serde::Serialize;
use std::sync::OnceLock;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidebarThread {
    id: String,
    title: String,
    updated_at: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceGroup {
    id: String,
    name: String,
    threads: Vec<SidebarThread>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ChatSection {
    Heading { text: String },
    Paragraph { text: String },
    List { items: Vec<String> },
    Code { code: String },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatBlock {
    id: String,
    role: String,
    sections: Vec<ChatSection>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
enum StageTab {
    Unstaged,
    Staged,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellSnapshot {
    shell_title: String,
    active_thread_id: String,
    workspaces: Vec<WorkspaceGroup>,
    chat_blocks: Vec<ChatBlock>,
    branch: String,
    runtime_label: String,
    default_stage_tab: StageTab,
    unstaged_count: u8,
    staged_count: u8,
}

fn build_shell_snapshot() -> ShellSnapshot {
    let default_stage_tab = if std::env::var_os("MOSAIC_DEFAULT_STAGED").is_some() {
        StageTab::Staged
    } else {
        StageTab::Unstaged
    };

    ShellSnapshot {
        shell_title: "Fix SlideStrip duplicate action".to_string(),
        active_thread_id: "thread-topedu-1".to_string(),
        workspaces: vec![
            WorkspaceGroup {
                id: "ws-topedu".to_string(),
                name: "topedu".to_string(),
                threads: vec![
                    SidebarThread {
                        id: "thread-topedu-1".to_string(),
                        title: "Fix SlideStrip duplicate action".to_string(),
                        updated_at: "2h".to_string(),
                    },
                    SidebarThread {
                        id: "thread-topedu-2".to_string(),
                        title: "Install skill-installer".to_string(),
                        updated_at: "2h".to_string(),
                    },
                ],
            },
            WorkspaceGroup {
                id: "ws-mosaic".to_string(),
                name: "mosaic".to_string(),
                threads: vec![
                    SidebarThread {
                        id: "thread-mosaic-1".to_string(),
                        title: "Build Phase 1 UI framework".to_string(),
                        updated_at: "2h".to_string(),
                    },
                    SidebarThread {
                        id: "thread-mosaic-2".to_string(),
                        title: "Send friendly greeting reply".to_string(),
                        updated_at: "2h".to_string(),
                    },
                ],
            },
        ],
        chat_blocks: vec![
            ChatBlock {
                id: "assistant-report".to_string(),
                role: "assistant".to_string(),
                sections: vec![
                    ChatSection::Paragraph {
                        text: "The duplicate slide action is fixed and verified with local typecheck."
                            .to_string(),
                    },
                    ChatSection::Heading {
                        text: "What changed".to_string(),
                    },
                    ChatSection::List {
                        items: vec![
                            "Added a safe active-slide update path to avoid stale references during quick navigation.".to_string(),
                            "Replaced direct active slide writes with a synchronized helper in editor flow.".to_string(),
                            "Kept existing behavior unchanged for normal edit workflows.".to_string(),
                        ],
                    },
                    ChatSection::Heading {
                        text: "Validation steps".to_string(),
                    },
                    ChatSection::List {
                        items: vec![
                            "Create a new page and type text in it.".to_string(),
                            "Click next page and immediately go back to previous page.".to_string(),
                            "Confirm text remains stable and is not overwritten.".to_string(),
                        ],
                    },
                    ChatSection::Code {
                        code: "pnpm -C /Users/jerrychir/Desktop/dev/coding/topeduction/topedu/frontend --filter @edublocks/core typecheck".to_string(),
                    },
                ],
            },
            ChatBlock {
                id: "user-followup".to_string(),
                role: "user".to_string(),
                sections: vec![ChatSection::Paragraph {
                    text: "Create a new page, switch quickly, then return. The previous page content should never be replaced."
                        .to_string(),
                }],
            },
            ChatBlock {
                id: "assistant-next".to_string(),
                role: "assistant".to_string(),
                sections: vec![ChatSection::Paragraph {
                    text: "I can continue with the next UI phase and wire the component editing workflow.".to_string(),
                }],
            },
        ],
        branch: "main".to_string(),
        runtime_label: "Desktop runtime via Rust snapshot".to_string(),
        default_stage_tab,
        unstaged_count: 0,
        staged_count: 0,
    }
}

static SNAPSHOT_CACHE: OnceLock<ShellSnapshot> = OnceLock::new();

#[tauri::command]
fn load_shell_snapshot() -> ShellSnapshot {
    SNAPSHOT_CACHE.get_or_init(build_shell_snapshot).clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, load_shell_snapshot])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
