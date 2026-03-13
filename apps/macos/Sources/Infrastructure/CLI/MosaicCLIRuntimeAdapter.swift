import Domain
import Foundation

public actor MosaicCLIRuntimeAdapter: AgentWorkbenchRuntime {
    private let processService: CLIStreamingProcessService
    private let gitInspector: WorkspaceGitInspector
    private let cliPathProvider: @Sendable () -> String?
    private var activeCancels: [UUID: @Sendable () -> Void] = [:]

    public init(
        processService: CLIStreamingProcessService = CLIStreamingProcessService(),
        gitInspector: WorkspaceGitInspector = WorkspaceGitInspector(),
        cliPathProvider: @escaping @Sendable () -> String? = { nil }
    ) {
        self.processService = processService
        self.gitInspector = gitInspector
        self.cliPathProvider = cliPathProvider
    }

    public func loadSnapshot(project: Project, selectedSessionID: String?) async throws -> ProjectSnapshot {
        let client = makeClient(cliPathOverride: nil)
        let workspace = project.workspaceReference

        async let status = client.status(workspace: workspace)
        async let health = try? client.health(workspace: workspace)
        async let configuration = try? client.configureShow(workspace: workspace)
        async let modelsStatus = try? client.modelsStatus(workspace: workspace)
        async let availableModels = (try? client.modelsList(workspace: workspace)) ?? []
        async let sessions = client.listSessions(workspace: workspace)

        let resolvedSessions = try await sessions
        let transcript: SessionTranscript?
        if let selectedSessionID, !selectedSessionID.isEmpty {
            transcript = try? await client.showSession(workspace: workspace, sessionID: selectedSessionID)
        } else {
            transcript = nil
        }

        return try await ProjectSnapshot(
            status: status,
            health: health,
            configuration: configuration,
            modelsStatus: modelsStatus,
            availableModels: availableModels,
            sessions: resolvedSessions,
            transcript: transcript
        )
    }

    public func setModel(project: Project, model: String) async throws -> ModelSelectionSummary {
        let client = makeClient(cliPathOverride: nil)
        return try await client.setModel(workspace: project.workspaceReference, model: model)
    }

    public func startTask(_ request: AgentTaskRequest) async throws -> RuntimeExecution {
        let executableURL = makeExecutableURL(cliPathOverride: request.cliPathOverride)
        guard FileManager.default.fileExists(atPath: executableURL.path) else {
            throw MosaicRuntimeFailure.executableNotFound(executableURL.path)
        }

        let arguments = makeTaskArguments(for: request)
        let command = CommandInvocation(
            displayCommand: renderCommand(executableURL: executableURL, arguments: arguments),
            executablePath: executableURL.path,
            arguments: arguments,
            workingDirectory: request.project.workspacePath,
            status: .waiting
        )
        let beforeSnapshot = await gitInspector.capture(project: request.project)
        let execution = try await processService.execute(
            CLIProcessRequest(
                executableURL: executableURL,
                arguments: arguments,
                workingDirectory: URL(fileURLWithPath: request.project.workspacePath, isDirectory: true),
                timeout: request.timeout
            )
        )
        let cancellation = CancellationBox()
        activeCancels[execution.id] = {
            cancellation.markCancelled()
            execution.cancel()
        }

        let stream = AsyncThrowingStream<RuntimeEvent, Error> { continuation in
            continuation.yield(.command(command))

            Task {
                var stdoutBuffer = ""
                var sawCompletion = false
                var sawFailure = false

                do {
                    for try await chunk in execution.stream {
                        switch chunk {
                        case let .started(pid):
                            continuation.yield(
                                .timeline(
                                    TimelineEntry(
                                        title: "Process launched",
                                        detail: pid.map { "mosaic-cli pid \($0)" } ?? "mosaic-cli started",
                                        level: .info
                                    )
                                )
                            )
                            continuation.yield(
                                .cliEvent(
                                    CLIEvent(
                                        taskID: execution.id,
                                        stream: .command,
                                        text: command.displayCommand,
                                        isImportant: true
                                    )
                                )
                            )
                        case let .stdout(text):
                            stdoutBuffer.append(text)
                            for line in Self.consumeLines(from: &stdoutBuffer) {
                                if let event = Self.parseRuntimeLine(
                                    line,
                                    taskID: execution.id,
                                    defaultProfile: request.profile
                                ) {
                                    switch event {
                                    case let .completed(response, exitCode):
                                        sawCompletion = true
                                        continuation.yield(.completed(response, exitCode: exitCode))
                                    case let .failed(message, exitCode):
                                        sawFailure = true
                                        continuation.yield(.failed(message: message, exitCode: exitCode))
                                    default:
                                        continuation.yield(event)
                                    }
                                } else {
                                    continuation.yield(
                                        .cliEvent(
                                            CLIEvent(taskID: execution.id, stream: .stdout, text: line)
                                        )
                                    )
                                }
                            }
                        case let .stderr(text):
                            continuation.yield(
                                .cliEvent(
                                    CLIEvent(
                                        taskID: execution.id,
                                        stream: .stderr,
                                        text: text,
                                        isImportant: true
                                    )
                                )
                            )
                        case .timedOut:
                            sawFailure = true
                            continuation.yield(
                                .failed(
                                    message: MosaicRuntimeFailure.timedOut.localizedDescription,
                                    exitCode: 124
                                )
                            )
                        case let .exited(code):
                            if cancellation.isCancelled {
                                continuation.yield(.cancelled)
                            } else if code != 0 && !sawCompletion && !sawFailure {
                                continuation.yield(
                                    .failed(
                                        message: "mosaic-cli exited with code \(code).",
                                        exitCode: Int(code)
                                    )
                                )
                            }
                        }
                    }

                    let afterSnapshot = await self.gitInspector.capture(project: request.project)
                    let changes = await self.gitInspector.changes(from: beforeSnapshot, to: afterSnapshot)
                    if !changes.isEmpty {
                        continuation.yield(.fileChanges(changes))
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }

                self.finishExecution(id: execution.id)
            }

            continuation.onTermination = { _ in
                cancellation.markCancelled()
                execution.cancel()
            }
        }

        return RuntimeExecution(
            id: execution.id,
            events: stream,
            cancel: {
                cancellation.markCancelled()
                execution.cancel()
            }
        )
    }

    public func cancelTask(id: UUID) async {
        activeCancels[id]?()
        activeCancels[id] = nil
    }

    private func makeClient(cliPathOverride: String?) -> MosaicCLIClient {
        let executableURL = makeExecutableURL(cliPathOverride: cliPathOverride)
        return MosaicCLIClient(executableURL: executableURL)
    }

    private func makeExecutableURL(cliPathOverride: String?) -> URL {
        MosaicCLIClient.resolveExecutableURL(overridePath: cliPathOverride ?? cliPathProvider())
    }

    private func makeTaskArguments(for request: AgentTaskRequest) -> [String] {
        var arguments = [
            "--profile", request.profile,
            "--project-state",
            "chat",
            "--emit-events",
            "--prompt", request.prompt,
        ]
        if let sessionID = request.sessionID, !sessionID.isEmpty {
            arguments.append(contentsOf: ["--session", sessionID])
        }
        return arguments
    }

    private func renderCommand(executableURL: URL, arguments: [String]) -> String {
        ([executableURL.path] + arguments)
            .map(Self.shellQuote)
            .joined(separator: " ")
    }

    private func finishExecution(id: UUID) {
        activeCancels[id] = nil
    }

    private static func consumeLines(from buffer: inout String) -> [String] {
        let normalized = buffer.replacingOccurrences(of: "\r\n", with: "\n")
        guard normalized.contains("\n") else {
            buffer = normalized
            return []
        }

        let parts = normalized.components(separatedBy: "\n")
        buffer = parts.last ?? ""
        return parts.dropLast().filter { !$0.isEmpty }
    }

    private static func parseRuntimeLine(
        _ line: String,
        taskID: UUID,
        defaultProfile: String
    ) -> RuntimeEvent? {
        guard
            let data = line.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = object["type"] as? String
        else {
            return nil
        }

        switch type {
        case "run_started":
            let detail = [
                object["profile"] as? String,
                object["agent_id"] as? String,
                object["cwd"] as? String,
            ]
            .compactMap { $0 }
            .joined(separator: " · ")
            return .timeline(TimelineEntry(title: "Task started", detail: detail))
        case "user":
            guard let sessionID = object["session_id"] as? String else { return nil }
            return .sessionStarted(sessionID)
        case "assistant":
            guard let text = object["text"] as? String else { return nil }
            return .messageDelta(text)
        case "tool_call":
            let name = object["name"] as? String ?? "tool"
            let rendered = renderJSONFragment(object["args"])
            return .toolCall(name: name, detail: rendered)
        case "tool_result":
            let name = object["name"] as? String ?? "tool"
            let rendered = renderJSONFragment(object["result"])
            return .toolResult(name: name, detail: rendered)
        case "error":
            let message = object["message"] as? String ?? "Unknown runtime error"
            return .failed(message: message, exitCode: nil)
        case "run_finished":
            guard
                let sessionID = object["session_id"] as? String,
                let response = object["response"] as? String
            else {
                return nil
            }
            let turns = object["turns"] as? Int ?? 1
            let profile = object["profile"] as? String ?? defaultProfile
            let agentID = object["agent_id"] as? String
            return .completed(
                PromptResponse(
                    sessionID: sessionID,
                    response: response,
                    profile: profile,
                    agentID: agentID,
                    turns: turns
                ),
                exitCode: 0
            )
        case "run_failed":
            let message = object["message"] as? String ?? "mosaic-cli task failed"
            return .failed(message: message, exitCode: object["exit_code"] as? Int)
        default:
            return nil
        }
    }

    private static func renderJSONFragment(_ value: Any?) -> String {
        guard let value else { return "" }
        guard JSONSerialization.isValidJSONObject(value) else { return String(describing: value) }
        guard
            let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted]),
            let text = String(data: data, encoding: .utf8)
        else {
            return String(describing: value)
        }
        return text
    }

    private static func shellQuote(_ value: String) -> String {
        if value.range(of: #"[\s"'\\]"#, options: .regularExpression) == nil {
            return value
        }
        return "'\(value.replacingOccurrences(of: "'", with: "'\\''"))'"
    }
}

private final class CancellationBox: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false

    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func markCancelled() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }
}
