import Domain
import Foundation

public struct CLIProcessRequest: Sendable {
    public var executableURL: URL
    public var arguments: [String]
    public var workingDirectory: URL
    public var environment: [String: String]
    public var timeout: Duration

    public init(
        executableURL: URL,
        arguments: [String],
        workingDirectory: URL,
        environment: [String: String] = [:],
        timeout: Duration = .seconds(120)
    ) {
        self.executableURL = executableURL
        self.arguments = arguments
        self.workingDirectory = workingDirectory
        self.environment = environment
        self.timeout = timeout
    }
}

public enum CLIProcessChunk: Sendable {
    case started(pid: Int32?)
    case stdout(String)
    case stderr(String)
    case exited(Int32)
    case timedOut
}

public struct CLIProcessExecution: Sendable {
    public let id: UUID
    public let stream: AsyncThrowingStream<CLIProcessChunk, Error>
    public let cancel: @Sendable () -> Void
}

public actor CLIStreamingProcessService {
    private var controllers: [UUID: CLIProcessController] = [:]

    public init() {}

    public func execute(_ request: CLIProcessRequest) throws -> CLIProcessExecution {
        let id = UUID()
        let controller = CLIProcessController(id: id, request: request) { [weak self] executionID in
            Task { await self?.removeController(for: executionID) }
        }
        controllers[id] = controller
        let stream = try controller.start()
        return CLIProcessExecution(id: id, stream: stream, cancel: { controller.cancel() })
    }

    public func cancel(id: UUID) {
        controllers[id]?.cancel()
        controllers[id] = nil
    }

    private func removeController(for id: UUID) {
        controllers[id] = nil
    }
}

private final class CLIProcessController: @unchecked Sendable {
    private let id: UUID
    private let request: CLIProcessRequest
    private let completion: @Sendable (UUID) -> Void
    private let lock = NSLock()
    private var process: Process?
    private var timeoutTask: Task<Void, Never>?
    private var didFinish = false

    init(id: UUID, request: CLIProcessRequest, completion: @escaping @Sendable (UUID) -> Void) {
        self.id = id
        self.request = request
        self.completion = completion
    }

    func start() throws -> AsyncThrowingStream<CLIProcessChunk, Error> {
        let process = Process()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()

        process.executableURL = request.executableURL
        process.arguments = request.arguments
        process.currentDirectoryURL = request.workingDirectory
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.environment = ProcessInfo.processInfo.environment.merging(request.environment) { _, new in new }

        self.process = process

        return AsyncThrowingStream { continuation in
            self.installReadHandler(on: stdoutPipe.fileHandleForReading) { text in
                continuation.yield(.stdout(text))
            }
            self.installReadHandler(on: stderrPipe.fileHandleForReading) { text in
                continuation.yield(.stderr(text))
            }

            process.terminationHandler = { [weak self] process in
                guard let self else { return }
                self.finish(continuation: continuation, event: .exited(process.terminationStatus))
            }

            continuation.onTermination = { [weak self] _ in
                self?.cancel()
            }

            do {
                try process.run()
                continuation.yield(.started(pid: process.processIdentifier))
                self.timeoutTask = Task { [weak self] in
                    guard let self else { return }
                    try? await Task.sleep(for: self.request.timeout)
                    self.handleTimeout(continuation: continuation)
                }
            } catch {
                continuation.finish(throwing: error)
                self.completion(self.id)
            }
        }
    }

    func cancel() {
        lock.lock()
        defer { lock.unlock() }
        guard let process, process.isRunning else { return }
        process.terminate()
    }

    private func handleTimeout(continuation: AsyncThrowingStream<CLIProcessChunk, Error>.Continuation) {
        lock.lock()
        let alreadyFinished = didFinish
        let process = self.process
        lock.unlock()

        guard !alreadyFinished else { return }
        continuation.yield(.timedOut)
        process?.terminate()
        finish(continuation: continuation, event: .exited(process?.terminationStatus ?? 124))
    }

    private func finish(
        continuation: AsyncThrowingStream<CLIProcessChunk, Error>.Continuation,
        event: CLIProcessChunk
    ) {
        lock.lock()
        guard !didFinish else {
            lock.unlock()
            return
        }
        didFinish = true
        let timeoutTask = self.timeoutTask
        let stdout = (process?.standardOutput as? Pipe)?.fileHandleForReading
        let stderr = (process?.standardError as? Pipe)?.fileHandleForReading
        stdout?.readabilityHandler = nil
        stderr?.readabilityHandler = nil
        lock.unlock()

        timeoutTask?.cancel()
        continuation.yield(event)
        continuation.finish()
        completion(id)
    }

    private func installReadHandler(on handle: FileHandle, emit: @escaping @Sendable (String) -> Void) {
        handle.readabilityHandler = { fileHandle in
            let data = fileHandle.availableData
            guard !data.isEmpty else { return }
            let text = String(decoding: data, as: UTF8.self)
            guard !text.isEmpty else { return }
            emit(text)
        }
    }
}
