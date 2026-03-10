import Domain
import Features
import Infrastructure
import XCTest

@MainActor
final class WorkbenchViewModelTests: XCTestCase {
    func testRefreshLoadsFirstThreadTranscript() async {
        let client = MockRuntimeClient()
        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
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
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
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
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )
        viewModel.composerText = "retry this"

        await viewModel.sendCurrentPrompt()

        XCTAssertEqual(viewModel.state.conversation.inlineError, "CLI unavailable")
        XCTAssertEqual(viewModel.lastError, "CLI unavailable")
        XCTAssertEqual(viewModel.composerText, "retry this")
        XCTAssertFalse(viewModel.state.conversation.isSending)
    }

    func testThreadFilteringAndSuggestedPromptEditing() async {
        let client = MockRuntimeClient()
        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )

        await viewModel.refresh()
        viewModel.threadFilter = "thread-2"

        XCTAssertEqual(viewModel.filteredThreads.count, 1)
        XCTAssertEqual(viewModel.filteredThreads.first?.id, "thread-2")

        viewModel.applySuggestedPrompt("Inspect the current workspace.")
        XCTAssertEqual(viewModel.composerText, "Inspect the current workspace.")
    }

    func testClearSelectedThreadRemovesCurrentSession() async {
        actor SessionStore {
            private(set) var sessions: [SessionSummaryData] = [
                SessionSummaryData(id: "thread-1", eventCount: 4, lastUpdated: "2026-03-10T09:20:00Z"),
                SessionSummaryData(id: "thread-2", eventCount: 2, lastUpdated: "2026-03-09T18:12:00Z"),
            ]

            func remove(_ id: String) {
                sessions.removeAll { $0.id == id }
            }

            func list() -> [SessionSummaryData] {
                sessions
            }
        }

        let store = SessionStore()
        let client = MockRuntimeClient()
        client.sessionsHandler = { _ in
            await store.list()
        }
        client.clearSessionHandler = { _, sessionID in
            await store.remove(sessionID)
            return sessionID
        }
        client.transcriptHandler = { _, sessionID in
            if sessionID == "thread-2" {
                return SessionTranscript(
                    sessionID: sessionID,
                    events: [
                        SessionEvent(
                            id: "assistant-2",
                            sessionID: sessionID,
                            type: .assistant,
                            timestamp: "2026-03-10T09:30:00Z",
                            text: "Second thread"
                        ),
                    ]
                )
            }
            return PreviewFixtures.transcript
        }

        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace],
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )

        await viewModel.refresh()
        XCTAssertEqual(viewModel.selectedThreadID, "thread-1")

        await viewModel.clearSelectedThread()

        XCTAssertEqual(viewModel.selectedThreadID, "thread-2")
        XCTAssertEqual(viewModel.state.conversation.sessionID, "thread-2")
        XCTAssertEqual(viewModel.state.sidebar.threads.count, 1)
        XCTAssertNil(viewModel.state.conversation.inlineError)
    }

    func testPinnedThreadsPersistIntoSections() async {
        let pinnedStore = InMemoryPinnedSessionStore(state: [
            PreviewFixtures.workspace.id: ["thread-2"]
        ])
        let client = MockRuntimeClient()
        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace],
            runtimeClient: client,
            pinnedSessionsStore: pinnedStore
        )

        await viewModel.refresh()

        XCTAssertEqual(viewModel.pinnedThreads.map(\.id), ["thread-2"])
        XCTAssertEqual(viewModel.recentThreads.map(\.id), ["thread-1"])
        XCTAssertEqual(viewModel.threadSections.map(\.id), ["pinned", "recent"])

        await viewModel.togglePinnedThread("thread-1")

        XCTAssertTrue(viewModel.isPinnedThread("thread-1"))
        XCTAssertEqual(Set(viewModel.pinnedThreads.map(\.id)), Set(["thread-1", "thread-2"]))
    }
}
