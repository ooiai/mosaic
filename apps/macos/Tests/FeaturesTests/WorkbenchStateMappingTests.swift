import Domain
import Features
import XCTest

final class WorkbenchStateMappingTests: XCTestCase {
    func testMapsConfiguredWorkspaceIntoWorkbenchState() {
        let state = WorkbenchStateMapper.map(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            status: PreviewFixtures.statusSummary,
            health: PreviewFixtures.healthSummary,
            models: PreviewFixtures.modelsStatusSummary,
            sessions: PreviewFixtures.sessions,
            transcript: PreviewFixtures.transcript,
            composerText: "",
            isSending: false,
            inlineError: nil
        )

        XCTAssertEqual(state.sidebar.currentWorkspace.name, "mosaic")
        XCTAssertEqual(state.sidebar.recentWorkspaces.count, 1)
        XCTAssertEqual(state.sidebar.threads.count, 2)
        XCTAssertEqual(state.conversation.messages.count, 4)
        XCTAssertFalse(state.conversation.suggestedPrompts.isEmpty)
        XCTAssertEqual(state.conversation.status.title, "Healthy")
        XCTAssertEqual(state.inspector.sections.count, 3)
    }

    func testPreservesInlineErrorWhenMapped() {
        let state = WorkbenchStateMapper.map(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace],
            status: PreviewFixtures.statusSummary,
            health: nil,
            models: PreviewFixtures.modelsStatusSummary,
            sessions: [],
            transcript: nil,
            composerText: "hello",
            isSending: false,
            inlineError: "failed to send"
        )

        XCTAssertEqual(state.conversation.inlineError, "failed to send")
        XCTAssertEqual(state.conversation.composerText, "hello")
    }

    func testMapsSystemAndToolEventsToSystemMessages() {
        let transcript = SessionTranscript(
            sessionID: "thread-system",
            events: [
                SessionEvent(
                    id: "sys-1",
                    sessionID: "thread-system",
                    type: .system,
                    timestamp: "2026-03-10T09:00:00Z",
                    text: "Workspace ready."
                ),
                SessionEvent(
                    id: "tool-1",
                    sessionID: "thread-system",
                    type: .toolResult,
                    timestamp: "2026-03-10T09:00:01Z",
                    text: "cargo test passed"
                ),
            ]
        )

        let state = WorkbenchStateMapper.map(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace],
            status: PreviewFixtures.statusSummary,
            health: PreviewFixtures.healthSummary,
            models: PreviewFixtures.modelsStatusSummary,
            sessions: [SessionSummaryData(id: "thread-system", eventCount: 2, lastUpdated: "2026-03-10T09:00:01Z")],
            transcript: transcript,
            composerText: "",
            isSending: false,
            inlineError: nil
        )

        XCTAssertEqual(state.conversation.messages.map(\.role), [.system, .system])
        XCTAssertEqual(state.conversation.threadTitle, "Workspace ready.")
    }
}
