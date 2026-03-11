import Domain
import Foundation

public final class MosaicCLIClient: MosaicRuntimeClient, @unchecked Sendable {
    public let executableURL: URL
    private let runner: CLIProcessRunner
    private let decoder: JSONDecoder

    public init(
        executableURL: URL? = nil,
        runner: CLIProcessRunner = CLIProcessRunner(),
        decoder: JSONDecoder = JSONDecoder()
    ) {
        self.executableURL = executableURL ?? MosaicCLIClient.resolveDefaultExecutableURL()
        self.runner = runner
        self.decoder = decoder
    }

    public func setup(
        workspace: WorkspaceReference,
        baseURL: String,
        model: String,
        apiKeyEnv: String
    ) async throws -> SetupSummary {
        try await execute(
            .setup(baseURL: baseURL, model: model, apiKeyEnv: apiKeyEnv),
            in: workspace,
            decode: CLISetupPayload.self
        ).toDomain()
    }

    public func status(workspace: WorkspaceReference) async throws -> RuntimeStatusSummary {
        try await execute(.status, in: workspace, decode: CLIStatusPayload.self).toDomain()
    }

    public func health(workspace: WorkspaceReference) async throws -> HealthSummary {
        try await execute(.health, in: workspace, decode: CLIHealthPayload.self).toDomain()
    }

    public func configureShow(workspace: WorkspaceReference) async throws -> ConfigurationSummary {
        try await execute(.configureShow, in: workspace, decode: CLIConfigurePayload.self).toDomain()
    }

    public func configureSet(
        workspace: WorkspaceReference,
        key: RuntimeConfigKey,
        value: String
    ) async throws {
        _ = try await execute(
            .configureSet(key: key, value: value),
            in: workspace,
            decode: CLIConfigureSetPayload.self
        )
    }

    public func modelsStatus(workspace: WorkspaceReference) async throws -> ModelsStatusSummary {
        try await execute(.modelsStatus, in: workspace, decode: CLIModelsStatusPayload.self).toDomain()
    }

    public func modelsList(workspace: WorkspaceReference) async throws -> [ModelSummary] {
        try await execute(.modelsList, in: workspace, decode: CLIModelsListPayload.self).toDomain()
    }

    public func setModel(
        workspace: WorkspaceReference,
        model: String
    ) async throws -> ModelSelectionSummary {
        try await execute(.modelsSet(model: model), in: workspace, decode: CLIModelsSetPayload.self).toDomain()
    }

    public func ask(workspace: WorkspaceReference, prompt: String) async throws -> PromptResponse {
        try await execute(.ask(prompt: prompt), in: workspace, decode: CLIPromptPayload.self).toDomain()
    }

    public func chat(
        workspace: WorkspaceReference,
        prompt: String,
        sessionID: String?
    ) async throws -> PromptResponse {
        try await execute(.chat(prompt: prompt, sessionID: sessionID), in: workspace, decode: CLIPromptPayload.self).toDomain()
    }

    public func listSessions(workspace: WorkspaceReference) async throws -> [SessionSummaryData] {
        try await execute(.sessionList, in: workspace, decode: CLISessionsPayload.self).toDomain()
    }

    public func showSession(
        workspace: WorkspaceReference,
        sessionID: String
    ) async throws -> SessionTranscript {
        try await execute(.sessionShow(id: sessionID), in: workspace, decode: CLISessionTranscriptPayload.self).toDomain()
    }

    public func clearSession(
        workspace: WorkspaceReference,
        sessionID: String
    ) async throws -> String {
        try await execute(.sessionClear(id: sessionID), in: workspace, decode: CLISessionClearPayload.self).toDomain()
    }

    private func execute<Payload: Decodable>(
        _ command: MosaicCommand,
        in workspace: WorkspaceReference,
        decode type: Payload.Type
    ) async throws -> Payload {
        let workspaceURL = URL(fileURLWithPath: workspace.path, isDirectory: true)
        guard FileManager.default.fileExists(atPath: executableURL.path) else {
            throw MosaicRuntimeFailure.executableNotFound(executableURL.path)
        }

        let output = try await runner.run(
            executableURL: executableURL,
            arguments: command.arguments,
            workingDirectory: workspaceURL
        )

        if let errorEnvelope = try? decoder.decode(CLIErrorEnvelope.self, from: output.stdout) {
            throw MosaicRuntimeFailure.commandFailed(
                code: errorEnvelope.error.code,
                message: errorEnvelope.error.message,
                exitCode: errorEnvelope.error.exitCode
            )
        }

        do {
            let envelope = try decoder.decode(CLIEnvelope<Payload>.self, from: output.stdout)
            return envelope.payload
        } catch {
            let stderr = String(data: output.stderr, encoding: .utf8) ?? ""
            let stdout = String(data: output.stdout, encoding: .utf8) ?? ""
            let diagnostic = [stderr, stdout, error.localizedDescription]
                .filter { !$0.isEmpty }
                .joined(separator: " | ")
            throw MosaicRuntimeFailure.invalidJSON(diagnostic)
        }
    }

    public static func resolveExecutableURL(overridePath: String? = nil) -> URL {
        if let overridePath, !overridePath.isEmpty {
            return URL(fileURLWithPath: overridePath)
        }
        return resolveDefaultExecutableURL()
    }

    private static func resolveDefaultExecutableURL() -> URL {
        if let override = ProcessInfo.processInfo.environment["MOSAIC_CLI_PATH"], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }

        let bundleURL = Bundle.main.bundleURL
            .appendingPathComponent("Contents")
            .appendingPathComponent("Resources")
            .appendingPathComponent("bin")
            .appendingPathComponent("mosaic")
        if FileManager.default.fileExists(atPath: bundleURL.path) {
            return bundleURL
        }

        let sidecarURL = URL(fileURLWithPath: CommandLine.arguments[0])
            .standardizedFileURL
            .deletingLastPathComponent()
            .appendingPathComponent("bin")
            .appendingPathComponent("mosaic")
        if FileManager.default.fileExists(atPath: sidecarURL.path) {
            return sidecarURL
        }

        let repoLocal = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../cli/target/debug/mosaic")
            .standardizedFileURL
        if FileManager.default.fileExists(atPath: repoLocal.path) {
            return repoLocal
        }

        let repoRelease = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../cli/target/release/mosaic")
            .standardizedFileURL
        if FileManager.default.fileExists(atPath: repoRelease.path) {
            return repoRelease
        }

        return URL(fileURLWithPath: "/usr/local/bin/mosaic")
    }
}
