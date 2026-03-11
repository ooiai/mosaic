import Foundation

public struct AgentTaskRequest: Sendable {
    public var project: Project
    public var prompt: String
    public var sessionID: String?
    public var profile: String
    public var timeout: Duration
    public var cliPathOverride: String?

    public init(
        project: Project,
        prompt: String,
        sessionID: String?,
        profile: String,
        timeout: Duration = .seconds(120),
        cliPathOverride: String? = nil
    ) {
        self.project = project
        self.prompt = prompt
        self.sessionID = sessionID
        self.profile = profile
        self.timeout = timeout
        self.cliPathOverride = cliPathOverride
    }
}

public enum RuntimeEvent: Sendable {
    case command(CommandInvocation)
    case timeline(TimelineEntry)
    case cliEvent(CLIEvent)
    case sessionStarted(String)
    case messageDelta(String)
    case fileChanges([FileChange])
    case completed(PromptResponse, exitCode: Int)
    case failed(message: String, exitCode: Int?)
    case cancelled
}

public struct RuntimeExecution: Sendable {
    public let id: UUID
    public let events: AsyncThrowingStream<RuntimeEvent, Error>
    public let cancel: @Sendable () -> Void

    public init(
        id: UUID = UUID(),
        events: AsyncThrowingStream<RuntimeEvent, Error>,
        cancel: @escaping @Sendable () -> Void
    ) {
        self.id = id
        self.events = events
        self.cancel = cancel
    }
}

public protocol AgentWorkbenchRuntime: Sendable {
    func loadSnapshot(project: Project, selectedSessionID: String?) async throws -> ProjectSnapshot
    func startTask(_ request: AgentTaskRequest) async throws -> RuntimeExecution
    func cancelTask(id: UUID) async
}

public protocol DesktopPersistenceStoring: Sendable {
    func loadArchive() async -> DesktopArchive
    func saveArchive(_ archive: DesktopArchive) async throws
}
