import Domain
import Foundation

public struct CLIProcessOutput: Sendable {
    public let stdout: Data
    public let stderr: Data
    public let exitCode: Int32
}

public actor CLIProcessRunner {
    public init() {}

    public func run(
        executableURL: URL,
        arguments: [String],
        workingDirectory: URL,
        environment: [String: String] = [:],
        timeout: Duration = .seconds(15)
    ) async throws -> CLIProcessOutput {
        let process = Process()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.executableURL = executableURL
        process.arguments = arguments
        process.currentDirectoryURL = workingDirectory
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.environment = ProcessInfo.processInfo.environment.merging(environment) { _, new in new }

        return try await withThrowingTaskGroup(of: CLIProcessOutput.self) { group in
            group.addTask {
                try process.run()
                process.waitUntilExit()
                return CLIProcessOutput(
                    stdout: stdoutPipe.fileHandleForReading.readDataToEndOfFile(),
                    stderr: stderrPipe.fileHandleForReading.readDataToEndOfFile(),
                    exitCode: process.terminationStatus
                )
            }

            group.addTask {
                try await Task.sleep(for: timeout)
                if process.isRunning {
                    process.terminate()
                }
                throw MosaicRuntimeFailure.timedOut
            }

            let output = try await group.next() ?? CLIProcessOutput(stdout: Data(), stderr: Data(), exitCode: 1)
            group.cancelAll()
            return output
        }
    }
}
