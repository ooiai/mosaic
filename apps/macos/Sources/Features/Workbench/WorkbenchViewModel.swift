import Domain
import Foundation
import Observation

@MainActor
@Observable
public final class WorkbenchViewModel {
    public private(set) var state: WorkbenchState
    public private(set) var selectedThreadID: String?
    public private(set) var isLoading = false
    public var isInspectorVisible = true
    public var lastError: String?
    public var threadFilter = ""
    public var composerText: String {
        get { state.conversation.composerText }
        set { state.conversation.composerText = newValue }
    }

    public var filteredThreads: [ThreadSummary] {
        let query = threadFilter.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return state.sidebar.threads }
        return state.sidebar.threads.filter {
            $0.title.localizedCaseInsensitiveContains(query)
                || $0.subtitle.localizedCaseInsensitiveContains(query)
        }
    }

    public var suggestedPrompts: [String] {
        [
            "Summarize the current workspace and tell me the next three concrete tasks.",
            "Review the latest session and surface the highest-risk follow-up.",
            "Inspect this project and propose a realistic implementation plan."
        ]
    }

    public var selectedThreadSummary: ThreadSummary? {
        guard let selectedThreadID else { return nil }
        return state.sidebar.threads.first(where: { $0.id == selectedThreadID })
    }

    public var threadCount: Int {
        state.sidebar.threads.count
    }

    public var messageCount: Int {
        state.conversation.messages.count
    }

    private let workspace: WorkspaceReference
    private let recentWorkspaces: [WorkspaceReference]
    private let runtimeClient: MosaicRuntimeClient

    public var workspaceReference: WorkspaceReference {
        workspace
    }

    public init(
        workspace: WorkspaceReference,
        recentWorkspaces: [WorkspaceReference],
        runtimeClient: MosaicRuntimeClient
    ) {
        self.workspace = workspace
        self.recentWorkspaces = recentWorkspaces
        self.runtimeClient = runtimeClient
        self.state = WorkbenchState.empty(workspace: workspace, recentWorkspaces: recentWorkspaces)
    }

    public func refresh() async {
        isLoading = true
        defer { isLoading = false }
        await loadState(preservingComposer: state.conversation.composerText)
    }

    public func selectThread(_ id: String) async {
        selectedThreadID = id
        await loadState(preservingComposer: state.conversation.composerText)
    }

    public func newThread() {
        selectedThreadID = nil
        state.conversation.sessionID = nil
        state.conversation.threadTitle = "New thread"
        state.conversation.messages = []
        state.conversation.composerText = ""
        state.conversation.status = RuntimeStripState(
            title: "Ready",
            detail: "Start with a concrete task, bug, or change request.",
            tone: .quiet
        )
        state.conversation.inlineError = nil
    }

    public func toggleInspector() {
        isInspectorVisible.toggle()
    }

    public func replaceStateForPreview(_ state: WorkbenchState) {
        self.state = state
        self.selectedThreadID = state.conversation.sessionID
    }

    public func applySuggestedPrompt(_ prompt: String) {
        composerText = prompt
    }

    public var canClearSelectedThread: Bool {
        selectedThreadID != nil && !isLoading && !state.conversation.isSending
    }

    public func sendCurrentPrompt() async {
        let prompt = state.conversation.composerText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty else { return }
        state.conversation.isSending = true
        state.conversation.inlineError = nil
        defer { state.conversation.isSending = false }

        do {
            let response = try await runtimeClient.chat(
                workspace: workspace,
                prompt: prompt,
                sessionID: selectedThreadID
            )
            selectedThreadID = response.sessionID
            state.conversation.composerText = ""
            await loadState(preservingComposer: "")
        } catch {
            lastError = error.localizedDescription
            state.conversation.inlineError = error.localizedDescription
        }
    }

    public func clearSelectedThread() async {
        guard let sessionID = selectedThreadID else { return }
        await clearThread(sessionID)
    }

    public func clearThread(_ sessionID: String) async {
        state.conversation.inlineError = nil
        isLoading = true
        defer { isLoading = false }

        do {
            _ = try await runtimeClient.clearSession(
                workspace: workspace,
                sessionID: sessionID
            )
            if selectedThreadID == sessionID {
                selectedThreadID = nil
            }
            await loadState(preservingComposer: state.conversation.composerText)
        } catch {
            lastError = error.localizedDescription
            state.conversation.inlineError = error.localizedDescription
        }
    }

    private func loadState(preservingComposer composerText: String) async {
        do {
            async let status = runtimeClient.status(workspace: workspace)
            async let models = runtimeClient.modelsStatus(workspace: workspace)
            async let sessions = runtimeClient.listSessions(workspace: workspace)

            let loadedStatus = try await status
            let loadedModels = try? await models
            let loadedHealth = try? await runtimeClient.health(workspace: workspace)
            let loadedSessions = try await sessions

            if selectedThreadID == nil {
                selectedThreadID = loadedSessions.first?.id
            }

            let transcript: SessionTranscript?
            if let selectedThreadID {
                transcript = try await runtimeClient.showSession(
                    workspace: workspace,
                    sessionID: selectedThreadID
                )
            } else {
                transcript = nil
            }

            state = WorkbenchStateMapper.map(
                workspace: workspace,
                recentWorkspaces: recentWorkspaces,
                status: loadedStatus,
                health: loadedHealth,
                models: loadedModels,
                sessions: loadedSessions,
                transcript: transcript,
                composerText: composerText,
                isSending: false,
                inlineError: nil
            )
        } catch {
            lastError = error.localizedDescription
            state.conversation.inlineError = error.localizedDescription
        }
    }
}
