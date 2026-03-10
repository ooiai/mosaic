import Domain
import Foundation

public final class MockRuntimeClient: MosaicRuntimeClient, @unchecked Sendable {
    public var setupHandler: ((WorkspaceReference, String, String, String) async throws -> SetupSummary)?
    public var statusHandler: ((WorkspaceReference) async throws -> RuntimeStatusSummary)?
    public var healthHandler: ((WorkspaceReference) async throws -> HealthSummary)?
    public var configureShowHandler: ((WorkspaceReference) async throws -> ConfigurationSummary)?
    public var configureSetHandler: ((WorkspaceReference, RuntimeConfigKey, String) async throws -> Void)?
    public var modelsStatusHandler: ((WorkspaceReference) async throws -> ModelsStatusSummary)?
    public var modelsListHandler: ((WorkspaceReference) async throws -> [ModelSummary])?
    public var setModelHandler: ((WorkspaceReference, String) async throws -> ModelSelectionSummary)?
    public var askHandler: ((WorkspaceReference, String) async throws -> PromptResponse)?
    public var chatHandler: ((WorkspaceReference, String, String?) async throws -> PromptResponse)?
    public var sessionsHandler: ((WorkspaceReference) async throws -> [SessionSummaryData])?
    public var transcriptHandler: ((WorkspaceReference, String) async throws -> SessionTranscript)?
    public var clearSessionHandler: ((WorkspaceReference, String) async throws -> String)?

    public init() {}

    public func setup(
        workspace: WorkspaceReference,
        baseURL: String,
        model: String,
        apiKeyEnv: String
    ) async throws -> SetupSummary {
        if let setupHandler {
            return try await setupHandler(workspace, baseURL, model, apiKeyEnv)
        }
        return SetupSummary(profile: "default", mode: "project", configPath: "\(workspace.path)/.mosaic/config.toml")
    }

    public func status(workspace: WorkspaceReference) async throws -> RuntimeStatusSummary {
        if let statusHandler {
            return try await statusHandler(workspace)
        }
        return PreviewFixtures.statusSummary
    }

    public func health(workspace: WorkspaceReference) async throws -> HealthSummary {
        if let healthHandler {
            return try await healthHandler(workspace)
        }
        return PreviewFixtures.healthSummary
    }

    public func configureShow(workspace: WorkspaceReference) async throws -> ConfigurationSummary {
        if let configureShowHandler {
            return try await configureShowHandler(workspace)
        }
        return PreviewFixtures.configurationSummary
    }

    public func configureSet(
        workspace: WorkspaceReference,
        key: RuntimeConfigKey,
        value: String
    ) async throws {
        if let configureSetHandler {
            try await configureSetHandler(workspace, key, value)
        }
    }

    public func modelsStatus(workspace: WorkspaceReference) async throws -> ModelsStatusSummary {
        if let modelsStatusHandler {
            return try await modelsStatusHandler(workspace)
        }
        return PreviewFixtures.modelsStatusSummary
    }

    public func modelsList(workspace: WorkspaceReference) async throws -> [ModelSummary] {
        if let modelsListHandler {
            return try await modelsListHandler(workspace)
        }
        return PreviewFixtures.modelList
    }

    public func setModel(
        workspace: WorkspaceReference,
        model: String
    ) async throws -> ModelSelectionSummary {
        if let setModelHandler {
            return try await setModelHandler(workspace, model)
        }
        return ModelSelectionSummary(
            requestedModel: model,
            effectiveModel: model,
            previousModel: PreviewFixtures.modelsStatusSummary.currentModel
        )
    }

    public func ask(workspace: WorkspaceReference, prompt: String) async throws -> PromptResponse {
        if let askHandler {
            return try await askHandler(workspace, prompt)
        }
        return PreviewFixtures.promptResponse
    }

    public func chat(
        workspace: WorkspaceReference,
        prompt: String,
        sessionID: String?
    ) async throws -> PromptResponse {
        if let chatHandler {
            return try await chatHandler(workspace, prompt, sessionID)
        }
        return PreviewFixtures.promptResponse
    }

    public func listSessions(workspace: WorkspaceReference) async throws -> [SessionSummaryData] {
        if let sessionsHandler {
            return try await sessionsHandler(workspace)
        }
        return PreviewFixtures.sessions
    }

    public func showSession(
        workspace: WorkspaceReference,
        sessionID: String
    ) async throws -> SessionTranscript {
        if let transcriptHandler {
            return try await transcriptHandler(workspace, sessionID)
        }
        return PreviewFixtures.transcript
    }

    public func clearSession(
        workspace: WorkspaceReference,
        sessionID: String
    ) async throws -> String {
        if let clearSessionHandler {
            return try await clearSessionHandler(workspace, sessionID)
        }
        return sessionID
    }
}
