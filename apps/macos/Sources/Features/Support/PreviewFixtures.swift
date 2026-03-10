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
            apiKeyEnv: "OPENAI_API_KEY",
            baseURL: "https://api.openai.com",
            kind: "openai_compatible",
            model: "gpt-4o-mini"
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
            apiKeyEnv: "OPENAI_API_KEY",
            baseURL: "https://api.openai.com",
            kind: "openai_compatible",
            model: "gpt-4o-mini"
        ),
        stateMode: "project",
        projectDir: ".mosaic"
    )

    public static let modelsStatusSummary = ModelsStatusSummary(
        profile: "default",
        currentModel: "gpt-4o-mini",
        effectiveModel: "gpt-4o-mini",
        baseURL: "https://api.openai.com",
        apiKeyEnv: "OPENAI_API_KEY",
        aliases: [:],
        fallbacks: []
    )

    public static let modelList = [
        ModelSummary(id: "gpt-4o-mini", ownedBy: "openai"),
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
}
