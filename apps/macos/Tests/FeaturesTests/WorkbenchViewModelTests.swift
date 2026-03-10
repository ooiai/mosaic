import Domain
import Features
import XCTest

@MainActor
final class WorkbenchViewModelTests: XCTestCase {
    func testRefreshLoadsFirstThreadTranscript() async {
        let client = MockRuntimeClient()
        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            runtimeClient: client
        )

        await viewModel.refresh()

        XCTAssertEqual(viewModel.selectedThreadID, "thread-1")
        XCTAssertEqual(viewModel.state.conversation.threadTitle, "Can you audit this migration and highlight risks?")
        XCTAssertEqual(viewModel.state.conversation.messages.count, 4)
        XCTAssertNil(viewModel.state.conversation.inlineError)
    }

    func testSendCurrentPromptClearsComposerAndRefreshesTranscript() async {
        actor SessionStore {
            private(set) var sessionID = "thread-1"

            func activateNewSession() {
                sessionID = "thread-2"
            }

            func currentSessionID() -> String {
                sessionID
            }
        }

        let store = SessionStore()
        let client = MockRuntimeClient()
        client.chatHandler = { _, prompt, _ in
            XCTAssertEqual(prompt, "Open the new inspector design.")
            await store.activateNewSession()
            return PromptResponse(
                sessionID: "thread-2",
                response: "Updated",
                profile: "default",
                agentID: "writer",
                turns: 2
            )
        }
        client.sessionsHandler = { _ in
            let active = await store.currentSessionID()
            return [SessionSummaryData(id: active, eventCount: 2, lastUpdated: "2026-03-10T10:20:00Z")]
        }
        client.transcriptHandler = { _, sessionID in
            SessionTranscript(
                sessionID: sessionID,
                events: [
                    SessionEvent(
                        id: "user-1",
                        sessionID: sessionID,
                        type: .user,
                        timestamp: "2026-03-10T10:19:59Z",
                        text: "Open the new inspector design."
                    ),
                    SessionEvent(
                        id: "assistant-1",
                        sessionID: sessionID,
                        type: .assistant,
                        timestamp: "2026-03-10T10:20:00Z",
                        text: "Inspector is now visible."
                    ),
                ]
            )
        }

        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace],
            runtimeClient: client
        )
        viewModel.composerText = "Open the new inspector design."

        await viewModel.sendCurrentPrompt()

        XCTAssertEqual(viewModel.selectedThreadID, "thread-2")
        XCTAssertEqual(viewModel.state.conversation.sessionID, "thread-2")
        XCTAssertTrue(viewModel.state.conversation.messages.contains(where: { $0.body == "Inspector is now visible." }))
        XCTAssertEqual(viewModel.composerText, "")
        XCTAssertFalse(viewModel.state.conversation.isSending)
    }

    func testSendCurrentPromptSurfacesInlineErrors() async {
        let client = MockRuntimeClient()
        client.chatHandler = { _, _, _ in
            throw MosaicRuntimeFailure.transport("CLI unavailable")
        }

        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace],
            runtimeClient: client
        )
        viewModel.composerText = "retry this"

        await viewModel.sendCurrentPrompt()

        XCTAssertEqual(viewModel.state.conversation.inlineError, "CLI unavailable")
        XCTAssertEqual(viewModel.lastError, "CLI unavailable")
        XCTAssertEqual(viewModel.composerText, "retry this")
        XCTAssertFalse(viewModel.state.conversation.isSending)
    }
}
