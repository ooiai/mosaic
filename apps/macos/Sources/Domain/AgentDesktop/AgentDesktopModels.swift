import Foundation

public struct Project: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var name: String
    public var workspacePath: String
    public var lastOpenedAt: Date
    public var preferredProfile: String
    public var recentProfiles: [String]

    public init(
        id: UUID = UUID(),
        name: String,
        workspacePath: String,
        lastOpenedAt: Date = .now,
        preferredProfile: String = "default",
        recentProfiles: [String] = []
    ) {
        self.id = id
        self.name = name
        self.workspacePath = workspacePath
        self.lastOpenedAt = lastOpenedAt
        self.preferredProfile = preferredProfile
        self.recentProfiles = recentProfiles
    }

    public var displayPath: String { workspacePath }

    public var workspaceReference: WorkspaceReference {
        WorkspaceReference(
            id: id,
            name: name,
            path: workspacePath,
            lastOpenedAt: lastOpenedAt
        )
    }
}

public enum SessionState: String, Codable, Sendable {
    case idle
    case waiting
    case running
    case failed
    case cancelled
    case done
}

public struct Session: Identifiable, Codable, Hashable, Sendable {
    public let id: String
    public var projectID: UUID
    public var title: String
    public var summary: String
    public var createdAt: Date
    public var updatedAt: Date
    public var messageCount: Int
    public var taskCount: Int
    public var state: SessionState
    public var isPinned: Bool
    public var latestTaskID: UUID?

    public init(
        id: String,
        projectID: UUID,
        title: String,
        summary: String = "",
        createdAt: Date = .now,
        updatedAt: Date = .now,
        messageCount: Int = 0,
        taskCount: Int = 0,
        state: SessionState = .idle,
        isPinned: Bool = false,
        latestTaskID: UUID? = nil
    ) {
        self.id = id
        self.projectID = projectID
        self.title = title
        self.summary = summary
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.messageCount = messageCount
        self.taskCount = taskCount
        self.state = state
        self.isPinned = isPinned
        self.latestTaskID = latestTaskID
    }
}

public enum MessageRole: String, Codable, Sendable {
    case user
    case assistant
    case system

    public var title: String {
        switch self {
        case .user: "You"
        case .assistant: "Agent"
        case .system: "System"
        }
    }
}

public enum MessageKind: String, Codable, Sendable {
    case markdown
    case task
    case log
    case status
    case error
}

public struct Message: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var sessionID: String
    public var role: MessageRole
    public var kind: MessageKind
    public var body: String
    public var createdAt: Date
    public var relatedTaskID: UUID?

    public init(
        id: UUID = UUID(),
        sessionID: String,
        role: MessageRole,
        kind: MessageKind = .markdown,
        body: String,
        createdAt: Date = .now,
        relatedTaskID: UUID? = nil
    ) {
        self.id = id
        self.sessionID = sessionID
        self.role = role
        self.kind = kind
        self.body = body
        self.createdAt = createdAt
        self.relatedTaskID = relatedTaskID
    }
}

public enum TimelineLevel: String, Codable, Sendable {
    case info
    case success
    case warning
    case failure
}

public struct TimelineEntry: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var title: String
    public var detail: String
    public var level: TimelineLevel
    public var timestamp: Date

    public init(
        id: UUID = UUID(),
        title: String,
        detail: String,
        level: TimelineLevel = .info,
        timestamp: Date = .now
    ) {
        self.id = id
        self.title = title
        self.detail = detail
        self.level = level
        self.timestamp = timestamp
    }
}

public enum CLIEventStream: String, Codable, Sendable {
    case command
    case stdout
    case stderr
    case status
    case system
}

public struct CLIEvent: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var taskID: UUID
    public var stream: CLIEventStream
    public var text: String
    public var timestamp: Date
    public var isImportant: Bool

    public init(
        id: UUID = UUID(),
        taskID: UUID,
        stream: CLIEventStream,
        text: String,
        timestamp: Date = .now,
        isImportant: Bool = false
    ) {
        self.id = id
        self.taskID = taskID
        self.stream = stream
        self.text = text
        self.timestamp = timestamp
        self.isImportant = isImportant
    }
}

public enum FileChangeStatus: String, Codable, Sendable {
    case added
    case modified
    case deleted
    case renamed
    case untracked
    case unknown
}

public struct FileChange: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var path: String
    public var previousPath: String?
    public var status: FileChangeStatus
    public var additions: Int
    public var deletions: Int
    public var diff: String
    public var isBinary: Bool

    public init(
        id: UUID = UUID(),
        path: String,
        previousPath: String? = nil,
        status: FileChangeStatus = .unknown,
        additions: Int = 0,
        deletions: Int = 0,
        diff: String = "",
        isBinary: Bool = false
    ) {
        self.id = id
        self.path = path
        self.previousPath = previousPath
        self.status = status
        self.additions = additions
        self.deletions = deletions
        self.diff = diff
        self.isBinary = isBinary
    }
}

public struct CommandInvocation: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var displayCommand: String
    public var executablePath: String
    public var arguments: [String]
    public var workingDirectory: String
    public var startedAt: Date
    public var finishedAt: Date?
    public var exitCode: Int?
    public var status: SessionState

    public init(
        id: UUID = UUID(),
        displayCommand: String,
        executablePath: String,
        arguments: [String],
        workingDirectory: String,
        startedAt: Date = .now,
        finishedAt: Date? = nil,
        exitCode: Int? = nil,
        status: SessionState = .waiting
    ) {
        self.id = id
        self.displayCommand = displayCommand
        self.executablePath = executablePath
        self.arguments = arguments
        self.workingDirectory = workingDirectory
        self.startedAt = startedAt
        self.finishedAt = finishedAt
        self.exitCode = exitCode
        self.status = status
    }
}

public struct MetadataItem: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var key: String
    public var value: String

    public init(id: UUID = UUID(), key: String, value: String) {
        self.id = id
        self.key = key
        self.value = value
    }
}

public struct AgentTask: Identifiable, Codable, Hashable, Sendable {
    public let id: UUID
    public var sessionID: String
    public var title: String
    public var prompt: String
    public var summary: String
    public var status: SessionState
    public var createdAt: Date
    public var startedAt: Date?
    public var finishedAt: Date?
    public var responseText: String
    public var errorText: String?
    public var retryCount: Int
    public var commands: [CommandInvocation]
    public var timeline: [TimelineEntry]
    public var cliEvents: [CLIEvent]
    public var fileChanges: [FileChange]
    public var metadata: [MetadataItem]
    public var exitCode: Int?

    public init(
        id: UUID = UUID(),
        sessionID: String,
        title: String,
        prompt: String,
        summary: String = "",
        status: SessionState = .waiting,
        createdAt: Date = .now,
        startedAt: Date? = nil,
        finishedAt: Date? = nil,
        responseText: String = "",
        errorText: String? = nil,
        retryCount: Int = 0,
        commands: [CommandInvocation] = [],
        timeline: [TimelineEntry] = [],
        cliEvents: [CLIEvent] = [],
        fileChanges: [FileChange] = [],
        metadata: [MetadataItem] = [],
        exitCode: Int? = nil
    ) {
        self.id = id
        self.sessionID = sessionID
        self.title = title
        self.prompt = prompt
        self.summary = summary
        self.status = status
        self.createdAt = createdAt
        self.startedAt = startedAt
        self.finishedAt = finishedAt
        self.responseText = responseText
        self.errorText = errorText
        self.retryCount = retryCount
        self.commands = commands
        self.timeline = timeline
        self.cliEvents = cliEvents
        self.fileChanges = fileChanges
        self.metadata = metadata
        self.exitCode = exitCode
    }

    public var hasLogs: Bool { !cliEvents.isEmpty }
    public var isActive: Bool { status == .waiting || status == .running }
}

public enum ThemeMode: String, Codable, CaseIterable, Sendable {
    case system
    case light
    case dark
}

public struct MarkdownPreferences: Codable, Hashable, Sendable {
    public var collapseLongContent: Bool
    public var showLineNumbers: Bool
    public var wrapCode: Bool
    public var renderImages: Bool
    public var highlightCode: Bool

    public init(
        collapseLongContent: Bool = true,
        showLineNumbers: Bool = true,
        wrapCode: Bool = true,
        renderImages: Bool = true,
        highlightCode: Bool = true
    ) {
        self.collapseLongContent = collapseLongContent
        self.showLineNumbers = showLineNumbers
        self.wrapCode = wrapCode
        self.renderImages = renderImages
        self.highlightCode = highlightCode
    }
}

public struct DebugOptions: Codable, Hashable, Sendable {
    public var showRawCLIEvents: Bool
    public var persistCommandLogs: Bool
    public var echoStdErrInChat: Bool

    public init(
        showRawCLIEvents: Bool = false,
        persistCommandLogs: Bool = true,
        echoStdErrInChat: Bool = false
    ) {
        self.showRawCLIEvents = showRawCLIEvents
        self.persistCommandLogs = persistCommandLogs
        self.echoStdErrInChat = echoStdErrInChat
    }
}

public struct AppSettings: Codable, Hashable, Sendable {
    public var cliPath: String?
    public var defaultWorkspacePath: String?
    public var defaultProfile: String
    public var themeMode: ThemeMode
    public var interfaceFontSize: Double
    public var markdown: MarkdownPreferences
    public var debug: DebugOptions

    public init(
        cliPath: String? = nil,
        defaultWorkspacePath: String? = nil,
        defaultProfile: String = "default",
        themeMode: ThemeMode = .system,
        interfaceFontSize: Double = 13,
        markdown: MarkdownPreferences = .init(),
        debug: DebugOptions = .init()
    ) {
        self.cliPath = cliPath
        self.defaultWorkspacePath = defaultWorkspacePath
        self.defaultProfile = defaultProfile
        self.themeMode = themeMode
        self.interfaceFontSize = interfaceFontSize
        self.markdown = markdown
        self.debug = debug
    }
}

public struct ProjectSnapshot: Sendable {
    public var status: RuntimeStatusSummary
    public var health: HealthSummary?
    public var configuration: ConfigurationSummary?
    public var modelsStatus: ModelsStatusSummary?
    public var availableModels: [ModelSummary]
    public var sessions: [SessionSummaryData]
    public var transcript: SessionTranscript?

    public init(
        status: RuntimeStatusSummary,
        health: HealthSummary?,
        configuration: ConfigurationSummary?,
        modelsStatus: ModelsStatusSummary?,
        availableModels: [ModelSummary],
        sessions: [SessionSummaryData],
        transcript: SessionTranscript?
    ) {
        self.status = status
        self.health = health
        self.configuration = configuration
        self.modelsStatus = modelsStatus
        self.availableModels = availableModels
        self.sessions = sessions
        self.transcript = transcript
    }
}

public struct ProjectArchive: Codable, Hashable, Sendable {
    public var project: Project
    public var sessions: [Session]
    public var messages: [Message]
    public var tasks: [AgentTask]
    public var selectedSessionID: String?
    public var composerDraft: String

    public init(
        project: Project,
        sessions: [Session] = [],
        messages: [Message] = [],
        tasks: [AgentTask] = [],
        selectedSessionID: String? = nil,
        composerDraft: String = ""
    ) {
        self.project = project
        self.sessions = sessions
        self.messages = messages
        self.tasks = tasks
        self.selectedSessionID = selectedSessionID
        self.composerDraft = composerDraft
    }
}

public struct DesktopArchive: Codable, Hashable, Sendable {
    public var projects: [ProjectArchive]
    public var selectedProjectID: UUID?
    public var settings: AppSettings

    public init(
        projects: [ProjectArchive] = [],
        selectedProjectID: UUID? = nil,
        settings: AppSettings = .init()
    ) {
        self.projects = projects
        self.selectedProjectID = selectedProjectID
        self.settings = settings
    }
}

public extension Project {
    init(workspace: WorkspaceReference, preferredProfile: String = "default") {
        self.init(
            id: workspace.id,
            name: workspace.name,
            workspacePath: workspace.path,
            lastOpenedAt: workspace.lastOpenedAt ?? .now,
            preferredProfile: preferredProfile,
            recentProfiles: [preferredProfile]
        )
    }
}
