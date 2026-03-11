import Domain
import Foundation

public enum PreviewFixtures {
    public static let workspace = WorkspaceReference(
        id: UUID(uuidString: "11111111-1111-1111-1111-111111111111")!,
        name: "mosaic",
        path: "/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic",
        lastOpenedAt: Date(timeIntervalSince1970: 1_741_592_800)
    )

    public static let secondaryWorkspace = WorkspaceReference(
        id: UUID(uuidString: "22222222-2222-2222-2222-222222222222")!,
        name: "playground",
        path: "/Users/jerrychir/Desktop/dev/coding/playground",
        lastOpenedAt: Date(timeIntervalSince1970: 1_741_582_800)
    )

    public static let statusSummary = RuntimeStatusSummary(
        configured: true,
        configPath: "\(workspace.path)/.mosaic/config.toml",
        profile: "default",
        latestSession: "thread-1",
        defaultAgentID: "writer",
        agentsCount: 1,
        stateMode: "project",
        provider: .init(
            apiKeyEnv: "AZURE_OPENAI_API_KEY",
            baseURL: "https://demo-resource.openai.azure.com/openai/v1",
            kind: "openai_compatible",
            model: "gpt-5.2"
        )
    )

    public static let healthSummary = HealthSummary(
        type: "health",
        checks: [
            .init(name: "state_dirs", detail: "state paths ready", status: "ok"),
            .init(name: "provider", detail: "provider reachable", status: "ok"),
        ]
    )

    public static let configurationSummary = ConfigurationSummary(
        profileName: "default",
        agent: .init(maxTurns: 8, temperature: 0.2),
        provider: .init(
            apiKeyEnv: "AZURE_OPENAI_API_KEY",
            baseURL: "https://demo-resource.openai.azure.com/openai/v1",
            kind: "openai_compatible",
            model: "gpt-5.2"
        ),
        stateMode: "project",
        projectDir: ".mosaic"
    )

    public static let modelsStatusSummary = ModelsStatusSummary(
        profile: "default",
        currentModel: "gpt-5.2",
        effectiveModel: "gpt-5.2",
        baseURL: "https://demo-resource.openai.azure.com/openai/v1",
        apiKeyEnv: "AZURE_OPENAI_API_KEY",
        aliases: [:],
        fallbacks: []
    )

    public static let modelList = [
        ModelSummary(id: "gpt-5.2", ownedBy: "azure"),
        ModelSummary(id: "gpt-4.1-mini", ownedBy: "openai"),
    ]

    public static let sessions = [
        SessionSummaryData(id: "thread-1", eventCount: 4, lastUpdated: "2026-03-10T09:20:00Z"),
        SessionSummaryData(id: "thread-2", eventCount: 2, lastUpdated: "2026-03-09T18:12:00Z"),
    ]

    public static let transcript = SessionTranscript(
        sessionID: "thread-1",
        events: [
            SessionEvent(
                id: "msg-1",
                sessionID: "thread-1",
                type: .user,
                timestamp: "2026-03-10T09:10:00Z",
                text: "Can you audit this migration and highlight risks?"
            ),
            SessionEvent(
                id: "msg-2",
                sessionID: "thread-1",
                type: .assistant,
                timestamp: "2026-03-10T09:10:01Z",
                text: "The migration is viable. The sharp edges are binary distribution, CLI bundle lookup, and session-state ownership."
            ),
            SessionEvent(
                id: "msg-3",
                sessionID: "thread-1",
                type: .user,
                timestamp: "2026-03-10T09:18:00Z",
                text: "Focus on the macOS app structure next."
            ),
            SessionEvent(
                id: "msg-4",
                sessionID: "thread-1",
                type: .assistant,
                timestamp: "2026-03-10T09:18:01Z",
                text: "Use a three-column workbench: workspace/thread list, conversation, and a runtime inspector."
            ),
        ]
    )

    public static let promptResponse = PromptResponse(
        sessionID: "thread-1",
        response: "Use a three-column workbench and keep the runtime summary in the inspector.",
        profile: "default",
        agentID: "writer",
        turns: 4
    )

    public static let project = Project(
        id: workspace.id,
        name: workspace.name,
        workspacePath: workspace.path,
        lastOpenedAt: workspace.lastOpenedAt ?? .now,
        preferredProfile: "default",
        recentProfiles: ["default", "review"]
    )

    public static let session = Session(
        id: "thread-1",
        projectID: project.id,
        title: "Audit the macOS app structure",
        summary: "Focus on the shell, streaming runtime, and inspector.",
        createdAt: Date(timeIntervalSince1970: 1_741_592_800),
        updatedAt: Date(timeIntervalSince1970: 1_741_593_200),
        messageCount: 4,
        taskCount: 1,
        state: .done,
        isPinned: true
    )

    public static let messageList: [Message] = [
        Message(
            sessionID: session.id,
            role: .user,
            body: "Can you audit this migration and highlight risks?",
            createdAt: Date(timeIntervalSince1970: 1_741_592_801)
        ),
        Message(
            sessionID: session.id,
            role: .assistant,
            body: "The migration is viable. The sharp edges are binary distribution, CLI bundle lookup, and session-state ownership.",
            createdAt: Date(timeIntervalSince1970: 1_741_592_802)
        ),
    ]

    public static let task = AgentTask(
        sessionID: session.id,
        title: "Execute · Audit the macOS app structure",
        prompt: "Audit the macOS app structure and highlight risks.",
        summary: "Three high-risk areas surfaced in the shell and runtime adapter.",
        status: .done,
        createdAt: Date(timeIntervalSince1970: 1_741_592_900),
        startedAt: Date(timeIntervalSince1970: 1_741_592_900),
        finishedAt: Date(timeIntervalSince1970: 1_741_592_940),
        responseText: "Three high-risk areas surfaced in the shell and runtime adapter.",
        commands: [
            CommandInvocation(
                displayCommand: "mosaic --profile default --project-state chat --emit-events --prompt 'Audit the app'",
                executablePath: "/usr/local/bin/mosaic",
                arguments: ["--profile", "default"],
                workingDirectory: project.workspacePath,
                startedAt: Date(timeIntervalSince1970: 1_741_592_900),
                finishedAt: Date(timeIntervalSince1970: 1_741_592_940),
                exitCode: 0,
                status: .done
            )
        ],
        timeline: [
            TimelineEntry(title: "Started", detail: "Mock task launched"),
            TimelineEntry(title: "Completed", detail: "Mock task completed", level: .success),
        ],
        cliEvents: [
            CLIEvent(taskID: UUID(uuidString: "33333333-3333-3333-3333-333333333333")!, stream: .stdout, text: "mock log"),
        ],
        fileChanges: [
            FileChange(path: "apps/macos/Sources/UI/App/WorkbenchView.swift", status: .modified, additions: 42, deletions: 12, diff: "@@ -1,1 +1,1 @@"),
        ],
        metadata: [
            MetadataItem(key: "workspace", value: project.workspacePath),
            MetadataItem(key: "profile", value: "default"),
        ],
        exitCode: 0
    )

    public static let projectArchive = ProjectArchive(
        project: project,
        sessions: [session],
        messages: messageList + [
            Message(
                sessionID: session.id,
                role: .system,
                kind: .task,
                body: task.title,
                createdAt: Date(timeIntervalSince1970: 1_741_592_905),
                relatedTaskID: task.id
            ),
        ],
        tasks: [task],
        selectedSessionID: session.id,
        composerDraft: "Continue the refactor."
    )

    public static let projectSnapshot = ProjectSnapshot(
        status: statusSummary,
        health: healthSummary,
        configuration: configurationSummary,
        modelsStatus: modelsStatusSummary,
        availableModels: modelList,
        sessions: sessions,
        transcript: transcript
    )
}
