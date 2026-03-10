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
    public var composerText: String {
        get { state.conversation.composerText }
        set { state.conversation.composerText = newValue }
    }

    private let workspace: WorkspaceReference
    private let recentWorkspaces: [WorkspaceReference]
    private let runtimeClient: MosaicRuntimeClient

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
        state.conversation.inlineError = nil
    }

    public func toggleInspector() {
        isInspectorVisible.toggle()
    }

    public func replaceStateForPreview(_ state: WorkbenchState) {
        self.state = state
        self.selectedThreadID = state.conversation.sessionID
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
