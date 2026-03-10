import Foundation

public struct WorkspaceReference: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var name: String
    public var path: String
    public var lastOpenedAt: Date?

    public init(
        id: UUID = UUID(),
        name: String,
        path: String,
        lastOpenedAt: Date? = nil
    ) {
        self.id = id
        self.name = name
        self.path = path
        self.lastOpenedAt = lastOpenedAt
    }

    public var displayPath: String { path }
}

public struct QuickAction: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let systemImage: String

    public init(id: String, title: String, systemImage: String) {
        self.id = id
        self.title = title
        self.systemImage = systemImage
    }
}

public struct ThreadSummary: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let subtitle: String
    public let updatedLabel: String
    public let eventCount: Int

    public init(
        id: String,
        title: String,
        subtitle: String,
        updatedLabel: String,
        eventCount: Int
    ) {
        self.id = id
        self.title = title
        self.subtitle = subtitle
        self.updatedLabel = updatedLabel
        self.eventCount = eventCount
    }
}

public struct RuntimeStripState: Equatable, Sendable {
    public enum Tone: String, Sendable {
        case quiet
        case success
        case warning
        case failure
    }

    public var title: String
    public var detail: String
    public var tone: Tone

    public init(title: String, detail: String, tone: Tone) {
        self.title = title
        self.detail = detail
        self.tone = tone
    }
}

public struct ConversationMessage: Identifiable, Equatable, Sendable {
    public enum Role: String, Codable, Sendable {
        case assistant
        case user
        case system

        public var title: String {
            switch self {
            case .assistant:
                "Assistant"
            case .user:
                "You"
            case .system:
                "System"
            }
        }
    }

    public let id: String
    public let role: Role
    public let body: String
    public let timestampLabel: String

    public init(id: String, role: Role, body: String, timestampLabel: String) {
        self.id = id
        self.role = role
        self.body = body
        self.timestampLabel = timestampLabel
    }
}

public struct WorkspaceSidebarState: Equatable, Sendable {
    public var currentWorkspace: WorkspaceReference
    public var recentWorkspaces: [WorkspaceReference]
    public var threads: [ThreadSummary]
    public var quickActions: [QuickAction]

    public init(
        currentWorkspace: WorkspaceReference,
        recentWorkspaces: [WorkspaceReference],
        threads: [ThreadSummary],
        quickActions: [QuickAction]
    ) {
        self.currentWorkspace = currentWorkspace
        self.recentWorkspaces = recentWorkspaces
        self.threads = threads
        self.quickActions = quickActions
    }
}

public struct ConversationState: Equatable, Sendable {
    public var threadTitle: String
    public var sessionID: String?
    public var status: RuntimeStripState
    public var messages: [ConversationMessage]
    public var suggestedPrompts: [String]
    public var composerText: String
    public var isSending: Bool
    public var inlineError: String?

    public init(
        threadTitle: String,
        sessionID: String?,
        status: RuntimeStripState,
        messages: [ConversationMessage],
        suggestedPrompts: [String] = [],
        composerText: String = "",
        isSending: Bool = false,
        inlineError: String? = nil
    ) {
        self.threadTitle = threadTitle
        self.sessionID = sessionID
        self.status = status
        self.messages = messages
        self.suggestedPrompts = suggestedPrompts
        self.composerText = composerText
        self.isSending = isSending
        self.inlineError = inlineError
    }
}

public struct InspectorKeyValue: Identifiable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let value: String

    public init(id: String, label: String, value: String) {
        self.id = id
        self.label = label
        self.value = value
    }
}

public struct InspectorSectionState: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let rows: [InspectorKeyValue]

    public init(id: String, title: String, rows: [InspectorKeyValue]) {
        self.id = id
        self.title = title
        self.rows = rows
    }
}

public struct InspectorState: Equatable, Sendable {
    public var sections: [InspectorSectionState]

    public init(sections: [InspectorSectionState]) {
        self.sections = sections
    }
}

public struct WorkbenchState: Equatable, Sendable {
    public var sidebar: WorkspaceSidebarState
    public var conversation: ConversationState
    public var inspector: InspectorState

    public init(
        sidebar: WorkspaceSidebarState,
        conversation: ConversationState,
        inspector: InspectorState
    ) {
        self.sidebar = sidebar
        self.conversation = conversation
        self.inspector = inspector
    }

    public static func empty(
        workspace: WorkspaceReference,
        recentWorkspaces: [WorkspaceReference]
    ) -> Self {
        Self(
            sidebar: WorkspaceSidebarState(
                currentWorkspace: workspace,
                recentWorkspaces: recentWorkspaces,
                threads: [],
                quickActions: [
                    QuickAction(id: "new-thread", title: "New Thread", systemImage: "square.and.pencil"),
                    QuickAction(id: "refresh", title: "Refresh", systemImage: "arrow.clockwise"),
                    QuickAction(id: "switch-workspace", title: "Switch Workspace", systemImage: "folder"),
                    QuickAction(id: "reveal-workspace", title: "Reveal in Finder", systemImage: "folder.badge.gearshape"),
                    QuickAction(id: "settings", title: "Settings", systemImage: "gearshape"),
                ]
            ),
            conversation: ConversationState(
                threadTitle: "New thread",
                sessionID: nil,
                status: RuntimeStripState(
                    title: "Ready",
                    detail: "Select a thread or start a new conversation.",
                    tone: .quiet
                ),
                messages: [],
                suggestedPrompts: [
                    "Summarize this workspace and identify the important moving parts.",
                    "Review the current runtime health for this project.",
                    "Propose the next three concrete tasks for this workspace.",
                ]
            ),
            inspector: InspectorState(
                sections: [
                    InspectorSectionState(
                        id: "context",
                        title: "Context",
                        rows: [
                            InspectorKeyValue(id: "workspace", label: "Workspace", value: workspace.name),
                            InspectorKeyValue(id: "path", label: "Path", value: workspace.path),
                        ]
                    ),
                    InspectorSectionState(
                        id: "session",
                        title: "Session",
                        rows: [
                            InspectorKeyValue(id: "session-id", label: "Session", value: "Not selected"),
                            InspectorKeyValue(id: "turns", label: "Turns", value: "0"),
                        ]
                    ),
                    InspectorSectionState(
                        id: "runtime",
                        title: "Runtime",
                        rows: [
                            InspectorKeyValue(id: "profile", label: "Profile", value: "Unknown"),
                            InspectorKeyValue(id: "model", label: "Model", value: "Unknown"),
                            InspectorKeyValue(id: "health", label: "Health", value: "Pending"),
                        ]
                    ),
                ]
            )
        )
    }
}
