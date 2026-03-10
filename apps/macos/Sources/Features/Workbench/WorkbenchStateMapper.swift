import Domain
import Foundation

public enum WorkbenchStateMapper {
    public static func map(
        workspace: WorkspaceReference,
        recentWorkspaces: [WorkspaceReference],
        status: RuntimeStatusSummary,
        health: HealthSummary?,
        models: ModelsStatusSummary?,
        sessions: [SessionSummaryData],
        transcript: SessionTranscript?,
        composerText: String,
        isSending: Bool,
        inlineError: String?
    ) -> WorkbenchState {
        let selectedSessionID = transcript?.sessionID ?? sessions.first?.id
        let threads = sessions.map {
            ThreadSummary(
                id: $0.id,
                title: threadTitle(for: $0),
                subtitle: $0.id,
                updatedLabel: $0.lastUpdated,
                eventCount: $0.eventCount
            )
        }

        let conversationMessages = transcript?.events.map { event in
            ConversationMessage(
                id: event.id,
                role: role(for: event),
                body: event.text,
                timestampLabel: event.timestamp
            )
        } ?? []

        let runtimeDetail = [
            status.profile.map { "Profile \($0)" },
            models.map { "Model \($0.effectiveModel)" },
            health?.overallStatus,
        ]
        .compactMap { $0 }
        .joined(separator: " · ")

        return WorkbenchState(
            sidebar: WorkspaceSidebarState(
                currentWorkspace: workspace,
                recentWorkspaces: recentWorkspaces.filter { $0.id != workspace.id },
                threads: threads,
                quickActions: [
                    QuickAction(id: "new-thread", title: "New Thread", systemImage: "square.and.pencil"),
                    QuickAction(id: "refresh", title: "Refresh", systemImage: "arrow.clockwise"),
                    QuickAction(id: "switch-workspace", title: "Switch Workspace", systemImage: "folder"),
                    QuickAction(id: "reveal-workspace", title: "Reveal in Finder", systemImage: "folder.badge.gearshape"),
                    QuickAction(id: "settings", title: "Settings", systemImage: "gearshape"),
                ]
            ),
            conversation: ConversationState(
                threadTitle: transcript.map(threadTitle(for:)) ?? "New thread",
                sessionID: selectedSessionID,
                status: RuntimeStripState(
                    title: health?.overallStatus ?? "Connected",
                    detail: runtimeDetail.isEmpty ? "Ready" : runtimeDetail,
                    tone: tone(for: health)
                ),
                messages: conversationMessages,
                suggestedPrompts: suggestedPrompts(workspace: workspace, transcript: transcript, health: health),
                composerText: composerText,
                isSending: isSending,
                inlineError: inlineError
            ),
            inspector: InspectorState(
                sections: [
                    InspectorSectionState(
                        id: "context",
                        title: "Context",
                        rows: [
                            InspectorKeyValue(id: "workspace", label: "Workspace", value: workspace.name),
                            InspectorKeyValue(id: "path", label: "Path", value: workspace.displayPath),
                            InspectorKeyValue(id: "state", label: "State Mode", value: status.stateMode ?? "project"),
                        ]
                    ),
                    InspectorSectionState(
                        id: "session",
                        title: "Session",
                        rows: [
                            InspectorKeyValue(id: "session-id", label: "Session", value: selectedSessionID ?? "None"),
                            InspectorKeyValue(id: "turns", label: "Turns", value: "\(conversationMessages.count)"),
                            InspectorKeyValue(id: "latest", label: "Latest", value: sessions.first?.lastUpdated ?? "—"),
                        ]
                    ),
                    InspectorSectionState(
                        id: "runtime",
                        title: "Runtime",
                        rows: [
                            InspectorKeyValue(id: "profile", label: "Profile", value: status.profile ?? "default"),
                            InspectorKeyValue(id: "model", label: "Model", value: models?.effectiveModel ?? status.provider?.model ?? "Unknown"),
                            InspectorKeyValue(id: "health", label: "Health", value: health?.overallStatus ?? "Pending"),
                        ]
                    ),
                ]
            )
        )
    }

    private static func tone(for health: HealthSummary?) -> RuntimeStripState.Tone {
        guard let health else { return .quiet }
        if health.checks.contains(where: { $0.status == "fail" }) { return .failure }
        if health.checks.contains(where: { $0.status == "warn" }) { return .warning }
        return .success
    }

    private static func threadTitle(for session: SessionSummaryData) -> String {
        "Thread \(session.id.prefix(8))"
    }

    private static func threadTitle(for transcript: SessionTranscript) -> String {
        let candidate = transcript.events.first {
            $0.type == .user && !$0.text.isEmpty
        }?.text ?? transcript.events.first(where: { !$0.text.isEmpty })?.text

        guard let firstText = candidate else {
            return "Thread \(transcript.sessionID.prefix(8))"
        }
        return String(firstText.prefix(64))
    }

    private static func role(for event: SessionEvent) -> ConversationMessage.Role {
        switch event.type {
        case .assistant:
            .assistant
        case .user:
            .user
        case .system, .toolCall, .toolResult, .error:
            .system
        }
    }

    private static func suggestedPrompts(
        workspace: WorkspaceReference,
        transcript: SessionTranscript?,
        health: HealthSummary?
    ) -> [String] {
        if transcript == nil {
            return [
                "Summarize the \(workspace.name) workspace and explain where I should start.",
                "Check the runtime health and tell me what needs attention.",
                "Propose a concrete implementation plan for the next change in this project.",
            ]
        }

        return [
            "Continue this thread with the next concrete action.",
            "Turn the discussion so far into a short execution plan.",
            health?.overallStatus == "Healthy"
                ? "Given the current healthy runtime, what should I do next?"
                : "The runtime is not fully healthy. What should I fix first?",
        ]
    }
}
