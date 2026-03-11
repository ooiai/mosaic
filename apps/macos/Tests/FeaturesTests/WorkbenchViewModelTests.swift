import Domain
import Features
import Infrastructure
import XCTest

@MainActor
final class WorkbenchViewModelTests: XCTestCase {
    func testRefreshLoadsSnapshotIntoSessionsAndMessages() async {
        let runtime = MockWorkbenchRuntime()
        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: runtime,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.sessions.first?.id, PreviewFixtures.session.id)
        XCTAssertTrue(viewModel.selectedMessages.contains(where: { $0.role == .assistant }))
        XCTAssertEqual(viewModel.currentModelLabel, PreviewFixtures.modelsStatusSummary.effectiveModel)
    }

    func testSendCurrentPromptConsumesRuntimeExecution() async throws {
        let runtime = MockWorkbenchRuntime()
        var didSend = false
        let newFile = FileChange(
            path: "apps/macos/Sources/UI/App/WorkbenchView.swift",
            status: .modified,
            additions: 8,
            deletions: 2,
            diff: "@@ -1,1 +1,1 @@"
        )
        runtime.snapshotHandler = { _, _ in
            if didSend {
                return ProjectSnapshot(
                    status: PreviewFixtures.statusSummary,
                    health: PreviewFixtures.healthSummary,
                    configuration: PreviewFixtures.configurationSummary,
                    modelsStatus: PreviewFixtures.modelsStatusSummary,
                    availableModels: PreviewFixtures.modelList,
                    sessions: PreviewFixtures.sessions,
                    transcript: SessionTranscript(
                        sessionID: PreviewFixtures.session.id,
                        events: PreviewFixtures.transcript.events + [
                            SessionEvent(
                                id: "msg-5",
                                sessionID: PreviewFixtures.session.id,
                                type: .assistant,
                                timestamp: "2026-03-10T09:19:00Z",
                                text: "Agent completed the task."
                            ),
                        ]
                    )
                )
            }
            return PreviewFixtures.projectSnapshot
        }
        runtime.startTaskHandler = { request in
            didSend = true
            let command = CommandInvocation(
                displayCommand: "mosaic chat --emit-events",
                executablePath: "/usr/local/bin/mosaic",
                arguments: ["chat", "--emit-events"],
                workingDirectory: request.project.workspacePath,
                status: .running
            )
            return RuntimeExecution(
                id: UUID(),
                events: AsyncThrowingStream { continuation in
                    continuation.yield(.command(command))
                    continuation.yield(.timeline(TimelineEntry(title: "Started", detail: "Mock runtime started")))
                    continuation.yield(.sessionStarted(PreviewFixtures.session.id))
                    continuation.yield(.cliEvent(CLIEvent(taskID: UUID(), stream: .stdout, text: "stdout line")))
                    continuation.yield(.messageDelta("Agent completed the task."))
                    continuation.yield(.fileChanges([newFile]))
                    continuation.yield(.completed(
                        PromptResponse(
                            sessionID: PreviewFixtures.session.id,
                            response: "Agent completed the task.",
                            profile: "default",
                            agentID: "writer",
                            turns: 2
                        ),
                        exitCode: 0
                    ))
                    continuation.finish()
                },
                cancel: {}
            )
        }

        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: runtime,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        await viewModel.bootstrap()
        viewModel.composerText = "Finish the App Shell refactor."
        await viewModel.sendCurrentPrompt(settings: AppSettings())

        XCTAssertEqual(viewModel.selectedTask?.status, .done)
        XCTAssertEqual(viewModel.selectedTask?.fileChanges.first?.path, newFile.path)
        XCTAssertTrue(viewModel.selectedMessages.contains(where: { $0.role == .assistant && $0.body.contains("Agent completed") }))
    }

    func testTogglePinnedPersistsState() async {
        let pinnedStore = InMemoryPinnedSessionStore()
        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: MockWorkbenchRuntime(),
            pinnedSessionsStore: pinnedStore
        ) { _ in }

        await viewModel.togglePinned(sessionID: PreviewFixtures.session.id)

        XCTAssertTrue(viewModel.sessions.first(where: { $0.id == PreviewFixtures.session.id })?.isPinned == false)
        let stored = await pinnedStore.pinnedSessionIDs(for: PreviewFixtures.project.id)
        XCTAssertTrue(stored.isEmpty)
    }
}
