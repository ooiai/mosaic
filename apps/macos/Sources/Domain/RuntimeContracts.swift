import Foundation

public struct SetupSummary: Equatable, Sendable {
    public let profile: String
    public let mode: String
    public let configPath: String

    public init(profile: String, mode: String, configPath: String) {
        self.profile = profile
        self.mode = mode
        self.configPath = configPath
    }
}

public struct RuntimeStatusSummary: Equatable, Sendable {
    public struct ProviderSummary: Equatable, Sendable {
        public let apiKeyEnv: String?
        public let baseURL: String?
        public let kind: String?
        public let model: String?

        public init(apiKeyEnv: String?, baseURL: String?, kind: String?, model: String?) {
            self.apiKeyEnv = apiKeyEnv
            self.baseURL = baseURL
            self.kind = kind
            self.model = model
        }
    }

    public let configured: Bool
    public let configPath: String
    public let profile: String?
    public let latestSession: String?
    public let defaultAgentID: String?
    public let agentsCount: Int
    public let stateMode: String?
    public let provider: ProviderSummary?

    public init(
        configured: Bool,
        configPath: String,
        profile: String?,
        latestSession: String?,
        defaultAgentID: String?,
        agentsCount: Int,
        stateMode: String?,
        provider: ProviderSummary?
    ) {
        self.configured = configured
        self.configPath = configPath
        self.profile = profile
        self.latestSession = latestSession
        self.defaultAgentID = defaultAgentID
        self.agentsCount = agentsCount
        self.stateMode = stateMode
        self.provider = provider
    }
}

public struct HealthCheckSummary: Equatable, Sendable {
    public let name: String
    public let detail: String
    public let status: String

    public init(name: String, detail: String, status: String) {
        self.name = name
        self.detail = detail
        self.status = status
    }
}

public struct HealthSummary: Equatable, Sendable {
    public let type: String
    public let checks: [HealthCheckSummary]

    public init(type: String, checks: [HealthCheckSummary]) {
        self.type = type
        self.checks = checks
    }

    public var overallStatus: String {
        if checks.contains(where: { $0.status == "fail" }) {
            return "Degraded"
        }
        if checks.contains(where: { $0.status == "warn" }) {
            return "Needs attention"
        }
        return "Healthy"
    }
}

public struct ModelSummary: Identifiable, Equatable, Sendable {
    public let id: String
    public let ownedBy: String?

    public init(id: String, ownedBy: String?) {
        self.id = id
        self.ownedBy = ownedBy
    }
}

public struct ModelsStatusSummary: Equatable, Sendable {
    public let profile: String?
    public let currentModel: String
    public let effectiveModel: String
    public let baseURL: String
    public let apiKeyEnv: String
    public let aliases: [String: String]
    public let fallbacks: [String]

    public init(
        profile: String?,
        currentModel: String,
        effectiveModel: String,
        baseURL: String,
        apiKeyEnv: String,
        aliases: [String: String],
        fallbacks: [String]
    ) {
        self.profile = profile
        self.currentModel = currentModel
        self.effectiveModel = effectiveModel
        self.baseURL = baseURL
        self.apiKeyEnv = apiKeyEnv
        self.aliases = aliases
        self.fallbacks = fallbacks
    }
}

public struct ConfigurationSummary: Equatable, Sendable {
    public struct AgentSummary: Equatable, Sendable {
        public let maxTurns: Int
        public let temperature: Double

        public init(maxTurns: Int, temperature: Double) {
            self.maxTurns = maxTurns
            self.temperature = temperature
        }
    }

    public struct ProviderSummary: Equatable, Sendable {
        public let apiKeyEnv: String
        public let baseURL: String
        public let kind: String
        public let model: String

        public init(apiKeyEnv: String, baseURL: String, kind: String, model: String) {
            self.apiKeyEnv = apiKeyEnv
            self.baseURL = baseURL
            self.kind = kind
            self.model = model
        }
    }

    public let profileName: String
    public let agent: AgentSummary
    public let provider: ProviderSummary
    public let stateMode: String
    public let projectDir: String

    public init(
        profileName: String,
        agent: AgentSummary,
        provider: ProviderSummary,
        stateMode: String,
        projectDir: String
    ) {
        self.profileName = profileName
        self.agent = agent
        self.provider = provider
        self.stateMode = stateMode
        self.projectDir = projectDir
    }
}

public enum RuntimeConfigKey: String, Equatable, Sendable {
    case providerBaseURL = "provider.base_url"
    case providerAPIKeyEnv = "provider.api_key_env"
}

public struct ModelSelectionSummary: Equatable, Sendable {
    public let requestedModel: String
    public let effectiveModel: String
    public let previousModel: String?

    public init(requestedModel: String, effectiveModel: String, previousModel: String?) {
        self.requestedModel = requestedModel
        self.effectiveModel = effectiveModel
        self.previousModel = previousModel
    }
}

public struct PromptResponse: Equatable, Sendable {
    public let sessionID: String
    public let response: String
    public let profile: String
    public let agentID: String?
    public let turns: Int

    public init(sessionID: String, response: String, profile: String, agentID: String?, turns: Int) {
        self.sessionID = sessionID
        self.response = response
        self.profile = profile
        self.agentID = agentID
        self.turns = turns
    }
}

public struct SessionEvent: Identifiable, Equatable, Sendable {
    public enum Kind: String, Codable, Sendable {
        case user
        case assistant
        case system
        case toolCall = "tool_call"
        case toolResult = "tool_result"
        case error
    }

    public let id: String
    public let sessionID: String
    public let type: Kind
    public let timestamp: String
    public let text: String

    public init(id: String, sessionID: String, type: Kind, timestamp: String, text: String) {
        self.id = id
        self.sessionID = sessionID
        self.type = type
        self.timestamp = timestamp
        self.text = text
    }
}

public struct SessionSummaryData: Identifiable, Equatable, Sendable {
    public let id: String
    public let eventCount: Int
    public let lastUpdated: String

    public init(id: String, eventCount: Int, lastUpdated: String) {
        self.id = id
        self.eventCount = eventCount
        self.lastUpdated = lastUpdated
    }
}

public struct SessionTranscript: Equatable, Sendable {
    public let sessionID: String
    public let events: [SessionEvent]

    public init(sessionID: String, events: [SessionEvent]) {
        self.sessionID = sessionID
        self.events = events
    }
}

public enum MosaicRuntimeFailure: LocalizedError, Equatable, Sendable {
    case executableNotFound(String)
    case invalidJSON(String)
    case commandFailed(code: String, message: String, exitCode: Int)
    case transport(String)
    case timedOut

    public var errorDescription: String? {
        switch self {
        case let .executableNotFound(path):
            "Unable to find mosaic CLI at \(path)."
        case let .invalidJSON(description):
            "CLI returned invalid JSON: \(description)"
        case let .commandFailed(_, message, _):
            message
        case let .transport(message):
            message
        case .timedOut:
            "The CLI request timed out."
        }
    }
}

public protocol MosaicRuntimeClient: Sendable {
    func setup(
        workspace: WorkspaceReference,
        baseURL: String,
        model: String,
        apiKeyEnv: String
    ) async throws -> SetupSummary

    func status(workspace: WorkspaceReference) async throws -> RuntimeStatusSummary
    func health(workspace: WorkspaceReference) async throws -> HealthSummary
    func configureShow(workspace: WorkspaceReference) async throws -> ConfigurationSummary
    func configureSet(
        workspace: WorkspaceReference,
        key: RuntimeConfigKey,
        value: String
    ) async throws
    func modelsStatus(workspace: WorkspaceReference) async throws -> ModelsStatusSummary
    func modelsList(workspace: WorkspaceReference) async throws -> [ModelSummary]
    func setModel(
        workspace: WorkspaceReference,
        model: String
    ) async throws -> ModelSelectionSummary
    func ask(workspace: WorkspaceReference, prompt: String) async throws -> PromptResponse
    func chat(
        workspace: WorkspaceReference,
        prompt: String,
        sessionID: String?
    ) async throws -> PromptResponse
    func listSessions(workspace: WorkspaceReference) async throws -> [SessionSummaryData]
    func showSession(
        workspace: WorkspaceReference,
        sessionID: String
    ) async throws -> SessionTranscript
    func clearSession(
        workspace: WorkspaceReference,
        sessionID: String
    ) async throws -> String
}

public protocol WorkspaceStoring: Sendable {
    func recentWorkspaces() async -> [WorkspaceReference]
    func selectedWorkspace() async -> WorkspaceReference?
    func save(workspace: WorkspaceReference) async
    func select(workspaceID: UUID) async
}

public protocol CommandHistoryStoring: Sendable {
    func recentCommandActionIDs() async -> [String]
    func recordCommandActionID(_ actionID: String) async
    func clearRecentCommandActionIDs() async
}
