import Domain
import Foundation

public final class MockWorkbenchRuntime: AgentWorkbenchRuntime, @unchecked Sendable {
    public var snapshotHandler: ((Project, String?) async throws -> ProjectSnapshot)?
    public var startTaskHandler: ((AgentTaskRequest) async throws -> RuntimeExecution)?
    public var cancelTaskHandler: ((UUID) async -> Void)?

    public init() {}

    public func loadSnapshot(project: Project, selectedSessionID: String?) async throws -> ProjectSnapshot {
        if let snapshotHandler {
            return try await snapshotHandler(project, selectedSessionID)
        }
        return PreviewFixtures.projectSnapshot
    }

    public func startTask(_ request: AgentTaskRequest) async throws -> RuntimeExecution {
        if let startTaskHandler {
            return try await startTaskHandler(request)
        }
        return Self.immediateExecution()
    }

    public func cancelTask(id: UUID) async {
        await cancelTaskHandler?(id)
    }

    public static func immediateExecution(events: [RuntimeEvent] = [
        .timeline(TimelineEntry(title: "Started", detail: "Mock task started")),
        .sessionStarted(PreviewFixtures.session.id),
        .messageDelta("Mock response"),
        .completed(PreviewFixtures.promptResponse, exitCode: 0),
    ]) -> RuntimeExecution {
        RuntimeExecution(
            id: UUID(),
            events: AsyncThrowingStream { continuation in
                for event in events {
                    continuation.yield(event)
                }
                continuation.finish()
            },
            cancel: {}
        )
    }
}
