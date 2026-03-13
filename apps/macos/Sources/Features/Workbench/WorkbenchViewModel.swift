import Domain
import Foundation
import Infrastructure
import Observation

public enum InspectorPanel: String, CaseIterable, Identifiable, Sendable {
    case overview
    case timeline
    case logs
    case commands
    case changes
    case metadata

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .overview: "Overview"
        case .timeline: "Timeline"
        case .logs: "CLI Logs"
        case .commands: "Commands"
        case .changes: "Files Changed"
        case .metadata: "Metadata"
        }
    }

    public var shortTitle: String {
        switch self {
        case .overview: "Overview"
        case .timeline: "Timeline"
        case .logs: "Logs"
        case .commands: "Commands"
        case .changes: "Changes"
        case .metadata: "Metadata"
        }
    }
}

public enum ComposerMode: String, CaseIterable, Identifiable, Sendable {
    case explore
    case execute
    case review

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .explore: "Explore"
        case .execute: "Execute"
        case .review: "Review"
        }
    }
}

@MainActor
@Observable
public final class WorkbenchViewModel {
    public private(set) var project: Project
    public private(set) var snapshot: ProjectSnapshot?
    public private(set) var sessions: [Session]
    public private(set) var messages: [Message]
    public private(set) var tasks: [AgentTask]
    public private(set) var selectedSessionID: String?
    public private(set) var selectedTaskID: UUID?
    public private(set) var selectedTimelineEntryID: UUID?
    public private(set) var selectedCLIEventID: UUID?
    public private(set) var selectedCommandID: UUID?
    public private(set) var activeExecutionID: UUID?
    public private(set) var selectedMessageID: UUID?
    public private(set) var highlightedMessageID: UUID?
    public private(set) var messageRevealToken = UUID()
    public var sidebarQuery = "" {
        didSet { persist() }
    }
    public var composerText = ""
    public var composerMode: ComposerMode = .execute
    public var selectedProfile: String
    public var isInspectorVisible = true {
        didSet { persist() }
    }
    public var inspectorPanel: InspectorPanel = .overview {
        didSet { persist() }
    }
    public var selectedFileChangeID: UUID? {
        didSet { persist() }
    }
    public private(set) var isLoadingSnapshot = false
    public private(set) var lastError: String?
    public private(set) var currentBranchLabel = "No git"

    private let runtime: AgentWorkbenchRuntime
    private let gitInspector: WorkspaceGitInspector
    private let pinnedSessionsStore: PinnedSessionsStoring
    private let onArchiveChange: @Sendable (ProjectArchive) async -> Void

    public init(
        project: Project,
        archive: ProjectArchive?,
        runtime: AgentWorkbenchRuntime,
        gitInspector: WorkspaceGitInspector = WorkspaceGitInspector(),
        pinnedSessionsStore: PinnedSessionsStoring,
        onArchiveChange: @escaping @Sendable (ProjectArchive) async -> Void
    ) {
        self.project = project
        self.runtime = runtime
        self.gitInspector = gitInspector
        self.pinnedSessionsStore = pinnedSessionsStore
        self.onArchiveChange = onArchiveChange
        self.sessions = archive?.sessions ?? []
        self.messages = archive?.messages ?? []
        self.tasks = archive?.tasks ?? []
        self.selectedSessionID = archive?.selectedSessionID
        self.sidebarQuery = archive?.uiState?.sidebarQuery ?? ""
        self.composerText = archive?.composerDraft ?? ""
        self.selectedProfile = archive?.project.preferredProfile ?? project.preferredProfile
        self.isInspectorVisible = archive?.uiState?.isInspectorVisible ?? true
        self.selectedFileChangeID = archive?.uiState?.selectedFileChangeID
        if let rawPanel = archive?.uiState?.inspectorPanel,
           let panel = InspectorPanel(rawValue: rawPanel) {
            self.inspectorPanel = panel
        }
    }

    public var filteredSessions: [Session] {
        let query = sidebarQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return sortedSessions }
        return sortedSessions.filter {
            $0.title.localizedCaseInsensitiveContains(query)
                || $0.summary.localizedCaseInsensitiveContains(query)
                || $0.id.localizedCaseInsensitiveContains(query)
        }
    }

    public var sortedSessions: [Session] {
        sessions.sorted { lhs, rhs in
            if lhs.isPinned != rhs.isPinned {
                return lhs.isPinned && !rhs.isPinned
            }
            return lhs.updatedAt > rhs.updatedAt
        }
    }

    public var selectedSession: Session? {
        guard let selectedSessionID else { return nil }
        return sessions.first(where: { $0.id == selectedSessionID })
    }

    public var selectedMessages: [Message] {
        guard let selectedSessionID else { return [] }
        return messages
            .filter { $0.sessionID == selectedSessionID }
            .sorted { $0.createdAt < $1.createdAt }
    }

    public var selectedTasks: [AgentTask] {
        guard let selectedSessionID else { return [] }
        return tasks
            .filter { $0.sessionID == selectedSessionID }
            .sorted { $0.createdAt > $1.createdAt }
    }

    public var recentTasks: [AgentTask] {
        tasks.sorted { $0.createdAt > $1.createdAt }
    }

    public var selectedTask: AgentTask? {
        if let selectedTaskID {
            return tasks.first(where: { $0.id == selectedTaskID })
        }
        return selectedTasks.first ?? recentTasks.first
    }

    public var selectedFileChange: FileChange? {
        guard
            let selectedFileChangeID,
            let selectedTask
        else {
            return selectedTask?.fileChanges.first
        }
        return selectedTask.fileChanges.first(where: { $0.id == selectedFileChangeID })
    }

    public var currentHealthLabel: String {
        snapshot?.health?.overallStatus ?? "Pending"
    }

    public var inspectorSelectionAnchorID: String? {
        switch inspectorPanel {
        case .timeline:
            return selectedTimelineEntryID.map { "timeline-\($0.uuidString)" }
        case .logs:
            return selectedCLIEventID.map { "log-\($0.uuidString)" }
        case .commands:
            return selectedCommandID.map { "command-\($0.uuidString)" }
        case .changes:
            return selectedFileChangeID.map { "change-\($0.uuidString)" }
        case .overview, .metadata:
            return nil
        }
    }

    public var currentModelLabel: String {
        snapshot?.modelsStatus?.effectiveModel ?? snapshot?.status.provider?.model ?? "Unknown"
    }

    public var availableModelChoices: [String] {
        let values = (snapshot?.availableModels ?? []).map(\.id)
        let combined = [currentModelLabel] + values
        return Self.uniqueProfiles(combined)
    }

    public var currentProviderLabel: String {
        let baseURL = snapshot?.configuration?.provider.baseURL
            ?? snapshot?.modelsStatus?.baseURL
            ?? snapshot?.status.provider?.baseURL
        if let baseURL, baseURL.contains("localhost") {
            return "Local"
        }
        if let baseURL, baseURL.contains("azure") {
            return "Azure OpenAI"
        }
        if baseURL != nil {
            return "OpenAI Compatible"
        }
        return "Unconfigured"
    }

    public var profileChoices: [String] {
        Self.uniqueProfiles(
            [selectedProfile, project.preferredProfile]
                + project.recentProfiles
                + [snapshot?.status.profile, snapshot?.modelsStatus?.profile]
                .compactMap { $0 }
        )
    }

    public var canSend: Bool {
        activeExecutionID == nil
            && !composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    public var canCancelTask: Bool { activeExecutionID != nil }

    public func bootstrap() async {
        let pinned = Set(await pinnedSessionsStore.pinnedSessionIDs(for: project.id))
        for index in sessions.indices {
            sessions[index].isPinned = pinned.contains(sessions[index].id)
        }
        await refresh()
    }

    public func refresh() async {
        isLoadingSnapshot = true
        defer { isLoadingSnapshot = false }
        do {
            let branch = await gitInspector.currentBranch(project: project)
            let snapshot = try await runtime.loadSnapshot(
                project: project,
                selectedSessionID: resolvedRuntimeSessionID
            )
            currentBranchLabel = branch ?? "No git"
            apply(snapshot: snapshot)
            lastError = nil
            persist()
        } catch {
            lastError = error.localizedDescription
        }
    }

    public func selectSession(_ sessionID: String) {
        selectedSessionID = sessionID
        selectedTaskID = selectedTasks.first?.id
        syncInspectorSelectionsForCurrentTask()
        selectedMessageID = nil
        inspectorPanel = .overview
        persist()
        Task { await refresh() }
    }

    public func selectTask(_ taskID: UUID) {
        selectedTaskID = taskID
        if let task = tasks.first(where: { $0.id == taskID }) {
            selectedSessionID = task.sessionID
            let fileChangeBelongsToTask = task.fileChanges.contains { $0.id == selectedFileChangeID }
            if !fileChangeBelongsToTask {
                selectedFileChangeID = task.fileChanges.first?.id
            }
        }
        syncInspectorSelectionsForCurrentTask()
        persist()
    }

    public func selectFileChange(_ fileChangeID: UUID) {
        selectedFileChangeID = fileChangeID
    }

    public func inspectFileChange(_ fileChangeID: UUID) {
        selectFileChange(fileChangeID)
        selectedTimelineEntryID = nil
        selectedCLIEventID = nil
        selectedCommandID = nil
        isInspectorVisible = true
        inspectorPanel = .changes
        guard
            let taskID = selectedTask?.id,
            let change = selectedFileChange
        else {
            return
        }
        revealMessage(matching: matchingActivityMessage(for: change, taskID: taskID))
    }

    public func inspectCommand(_ commandID: UUID) {
        guard
            let task = selectedTask,
            let command = task.commands.first(where: { $0.id == commandID })
        else {
            return
        }
        selectedCommandID = commandID
        selectedTimelineEntryID = nil
        selectedCLIEventID = nil
        isInspectorVisible = true
        inspectorPanel = .commands
        revealMessage(matching: matchingCommandMessage(for: command, taskID: task.id))
    }

    public func inspectTimelineEntry(_ entryID: UUID) {
        guard
            let task = selectedTask,
            let entry = task.timeline.first(where: { $0.id == entryID })
        else {
            return
        }
        selectedTimelineEntryID = entryID
        selectedCLIEventID = nil
        selectedCommandID = nil
        isInspectorVisible = true
        inspectorPanel = .timeline
        revealMessage(matching: matchingStatusMessage(for: task.id, entry: entry))
    }

    public func inspectCLIEvent(_ eventID: UUID) {
        guard
            let task = selectedTask,
            let event = task.cliEvents.first(where: { $0.id == eventID })
        else {
            return
        }
        selectedCLIEventID = eventID
        selectedTimelineEntryID = nil
        selectedCommandID = nil
        isInspectorVisible = true
        inspectorPanel = .logs
        revealMessage(matching: matchingLogMessage(for: task.id, event: event))
    }

    public func inspectActivity(_ payload: ActivityMessagePayload, taskID: UUID?) {
        guard let taskID else { return }
        selectTask(taskID)
        isInspectorVisible = true

        let panel = preferredInspectorPanel(for: payload, taskID: taskID)
        inspectorPanel = panel
        selectedTimelineEntryID = nil
        selectedCLIEventID = nil
        selectedCommandID = matchingCommand(for: payload, taskID: taskID)?.id

        if panel == .changes, let change = matchingFileChange(for: payload, taskID: taskID) {
            selectedFileChangeID = change.id
        } else {
            selectedFileChangeID = nil
            if panel == .timeline {
                selectedTimelineEntryID = matchingTimelineEntry(for: payload, taskID: taskID)?.id
            } else if panel == .logs {
                selectedCLIEventID = matchingCLIEvent(for: payload, taskID: taskID)?.id
            }
        }
    }

    public func inspectMessage(_ messageID: UUID) {
        guard let message = messages.first(where: { $0.id == messageID }) else { return }
        selectedMessageID = message.id
        selectedSessionID = message.sessionID

        guard let taskID = message.relatedTaskID else {
            persist()
            return
        }

        selectTask(taskID)
        isInspectorVisible = true

        switch message.kind {
        case .task:
            selectedTimelineEntryID = nil
            selectedCLIEventID = nil
            selectedCommandID = nil
            inspectorPanel = .overview
        case .activity:
            if let payload = ActivityMessagePayload.decode(from: message.body) {
                let panel = preferredInspectorPanel(for: payload, taskID: taskID)
                inspectorPanel = panel
                selectedCommandID = matchingCommand(for: payload, taskID: taskID)?.id
                selectedTimelineEntryID = panel == .timeline ? matchingTimelineEntry(for: payload, taskID: taskID)?.id : nil
                selectedCLIEventID = panel == .logs ? matchingCLIEvent(for: payload, taskID: taskID)?.id : nil
                if panel == .changes, let change = matchingFileChange(for: payload, taskID: taskID) {
                    selectedFileChangeID = change.id
                } else {
                    selectedFileChangeID = nil
                }
            } else {
                selectedTimelineEntryID = nil
                selectedCLIEventID = nil
                selectedCommandID = nil
                inspectorPanel = .timeline
            }
        case .status:
            selectedTimelineEntryID = matchingTimelineEntry(for: message, taskID: taskID)?.id
            selectedCLIEventID = nil
            selectedCommandID = nil
            selectedFileChangeID = nil
            inspectorPanel = .timeline
        case .log:
            if let command = matchingCommand(for: message, taskID: taskID) {
                selectedCommandID = command.id
                selectedCLIEventID = nil
                inspectorPanel = .commands
            } else {
                selectedCommandID = nil
                selectedCLIEventID = matchingCLIEvent(for: message, taskID: taskID)?.id
                inspectorPanel = .logs
            }
            selectedTimelineEntryID = nil
            selectedFileChangeID = nil
        case .error:
            selectedTimelineEntryID = nil
            selectedCommandID = nil
            selectedFileChangeID = nil
            selectedCLIEventID = matchingCLIEvent(for: message, taskID: taskID)?.id
            inspectorPanel = .logs
        case .markdown:
            selectedTimelineEntryID = nil
            selectedCLIEventID = nil
            selectedCommandID = nil
            selectedFileChangeID = nil
            inspectorPanel = .overview
        }

        persist()
    }

    private func revealMessage(matching message: Message?) {
        guard let message else { return }
        selectedMessageID = message.id
        highlightedMessageID = message.id
        messageRevealToken = UUID()
        Task {
            try? await Task.sleep(for: .seconds(2))
            if highlightedMessageID == message.id {
                highlightedMessageID = nil
            }
        }
    }

    public func newThread() {
        let pendingID = Self.pendingSessionID()
        let session = Session(
            id: pendingID,
            projectID: project.id,
            title: "New Task",
            summary: "Draft session",
            state: .idle
        )
        sessions.removeAll { $0.id == pendingID }
        sessions.insert(session, at: 0)
        selectedSessionID = pendingID
        selectedTaskID = nil
        selectedTimelineEntryID = nil
        selectedCLIEventID = nil
        selectedCommandID = nil
        selectedMessageID = nil
        composerText = ""
        persist()
    }

    public func togglePinned(sessionID: String) async {
        guard let index = sessions.firstIndex(where: { $0.id == sessionID }) else { return }
        sessions[index].isPinned.toggle()
        await pinnedSessionsStore.setPinnedSessionID(sessionID, pinned: sessions[index].isPinned, workspaceID: project.id)
        persist()
    }

    public func toggleInspector() {
        isInspectorVisible.toggle()
    }

    public func cancelActiveTask() async {
        guard let activeExecutionID else { return }
        await runtime.cancelTask(id: activeExecutionID)
    }

    public func selectModel(_ model: String) async {
        do {
            _ = try await runtime.setModel(project: project, model: model)
            await refresh()
        } catch {
            lastError = error.localizedDescription
        }
    }

    public func retrySelectedTask(settings: AppSettings) async {
        guard let selectedTask else { return }
        composerText = selectedTask.prompt
        await sendCurrentPrompt(settings: settings, retryCount: selectedTask.retryCount + 1)
    }

    public func sendCurrentPrompt(settings: AppSettings, retryCount: Int = 0) async {
        let prompt = composerText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty else { return }

        let pendingSessionID = selectedSessionID ?? Self.pendingSessionID()
        ensureSessionExists(id: pendingSessionID, title: "Running Task")

        let userMessage = Message(
            sessionID: pendingSessionID,
            role: .user,
            body: prompt,
            createdAt: .now
        )
        messages.append(userMessage)

        let task = AgentTask(
            sessionID: pendingSessionID,
            title: "\(composerMode.title) · \(taskTitle(for: prompt))",
            prompt: prompt,
            status: .running,
            startedAt: .now,
            retryCount: retryCount,
            timeline: [
                TimelineEntry(
                    title: "Queued",
                    detail: "Waiting for mosaic-cli to start.",
                    level: .info
                )
            ],
            metadata: [
                MetadataItem(key: "workspace", value: project.workspacePath),
                MetadataItem(key: "profile", value: selectedProfile),
                MetadataItem(key: "provider", value: currentProviderLabel),
            ]
        )
        tasks.append(task)
        messages.append(
            Message(
                sessionID: pendingSessionID,
                role: .system,
                kind: .task,
                body: task.title,
                relatedTaskID: task.id
            )
        )

        selectedSessionID = pendingSessionID
        selectedTaskID = task.id
        selectedFileChangeID = nil
        composerText = ""
        updateSession(pendingSessionID) {
            $0.updatedAt = .now
            $0.state = .running
            $0.taskCount += 1
            $0.messageCount += 1
            $0.latestTaskID = task.id
        }
        persist()

        do {
            let execution = try await runtime.startTask(
                AgentTaskRequest(
                    project: project,
                    prompt: prompt,
                    sessionID: pendingSessionID.hasPrefix("pending-") ? nil : pendingSessionID,
                    profile: selectedProfile,
                    cliPathOverride: settings.cliPath
                )
            )
            activeExecutionID = execution.id

            for try await event in execution.events {
                switch event {
                case let .command(command):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.commands = [command]
                        $0.timeline.append(
                            TimelineEntry(
                                title: "Command ready",
                                detail: command.displayCommand,
                                level: .info
                            )
                        )
                    }
                    _ = appendRuntimeLogEntry(
                        taskID: task.id,
                        sessionID: sessionID,
                        entry: "$ \(command.displayCommand)"
                    )
                case let .timeline(entry):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.timeline.append(entry)
                    }
                    _ = upsertRuntimeStatusMessage(
                        taskID: task.id,
                        sessionID: sessionID,
                        text: "\(entry.title)\n\(entry.detail)"
                    )
                case let .cliEvent(event):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.cliEvents.append(event)
                    }
                    if shouldSurface(event: event) {
                        _ = appendRuntimeLogEntry(
                            taskID: task.id,
                            sessionID: sessionID,
                            entry: Self.logLabel(for: event.stream) + event.text.trimmingCharacters(in: .whitespacesAndNewlines)
                        )
                    }
                case let .toolCall(name, detail):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.timeline.append(
                            TimelineEntry(
                                title: "Tool call",
                                detail: detail.isEmpty ? name : "\(name) · \(detail)",
                                level: .info
                            )
                        )
                    }
                    _ = appendActivityMessage(
                        taskID: task.id,
                        sessionID: sessionID,
                        title: "Tool call",
                        name: name,
                        detail: detail,
                        language: detail.isEmpty ? nil : "json"
                    )
                case let .toolResult(name, detail):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    let logText = detail.isEmpty ? "Tool result · \(name)" : "Tool result · \(name)\n\(detail)"
                    updateTask(task.id) {
                        $0.timeline.append(
                            TimelineEntry(
                                title: "Tool result",
                                detail: name,
                                level: .success
                            )
                        )
                        $0.cliEvents.append(
                            CLIEvent(
                                taskID: task.id,
                                stream: .status,
                                text: logText,
                                isImportant: true
                            )
                        )
                    }
                    _ = appendActivityMessage(
                        taskID: task.id,
                        sessionID: sessionID,
                        title: "Tool result",
                        name: name,
                        detail: detail,
                        language: detail.isEmpty ? nil : "json"
                    )
                case let .sessionStarted(sessionID):
                    reassignPendingSession(from: pendingSessionID, to: sessionID)
                case let .messageDelta(text):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    let assistantMessageUpdate = upsertAssistantMessage(
                        taskID: task.id,
                        sessionID: sessionID,
                        body: text
                    )
                    updateTask(task.id) {
                        $0.summary = Self.firstLine(of: assistantMessageUpdate.body)
                        $0.responseText = assistantMessageUpdate.body
                    }
                    updateSession(sessionID) {
                        $0.updatedAt = .now
                        $0.state = .running
                        $0.summary = Self.firstLine(of: assistantMessageUpdate.body, fallback: $0.summary)
                        if assistantMessageUpdate.inserted {
                            $0.messageCount += 1
                        }
                        $0.latestTaskID = task.id
                    }
                case let .fileChanges(changes):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.fileChanges = changes
                    }
                    _ = upsertRuntimeStatusMessage(
                        taskID: task.id,
                        sessionID: sessionID,
                        text: "Files changed\nDetected \(changes.count) file change(s)."
                    )
                    selectedFileChangeID = changes.first?.id
                case let .completed(response, exitCode):
                    updateTask(task.id) {
                        $0.status = .done
                        $0.finishedAt = .now
                        $0.exitCode = exitCode
                        $0.responseText = response.response
                        $0.summary = Self.firstLine(of: response.response)
                        $0.timeline.append(
                            TimelineEntry(
                                title: "Completed",
                                detail: "mosaic-cli returned \(response.turns) turn(s).",
                                level: .success
                            )
                        )
                    }
                    let resolvedSessionID = response.sessionID
                    reassignPendingSession(from: pendingSessionID, to: resolvedSessionID)
                    let assistantMessageUpdate = upsertAssistantMessage(
                        taskID: task.id,
                        sessionID: resolvedSessionID,
                        body: response.response
                    )
                    _ = upsertRuntimeStatusMessage(
                        taskID: task.id,
                        sessionID: resolvedSessionID,
                        text: "Completed\nmosaic-cli returned \(response.turns) turn(s)."
                    )
                    updateSession(resolvedSessionID) {
                        $0.updatedAt = .now
                        $0.state = .done
                        $0.title = Self.sessionTitle(for: response.response, fallback: $0.title)
                        $0.summary = Self.firstLine(of: response.response)
                        if assistantMessageUpdate.inserted {
                            $0.messageCount += 1
                        }
                        $0.latestTaskID = task.id
                    }
                case let .failed(message, exitCode):
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.status = .failed
                        $0.finishedAt = .now
                        $0.exitCode = exitCode
                        $0.errorText = message
                        $0.timeline.append(
                            TimelineEntry(
                                title: "Failed",
                                detail: message,
                                level: .failure
                            )
                        )
                    }
                    _ = appendRuntimeLogEntry(
                        taskID: task.id,
                        sessionID: sessionID,
                        entry: "[stderr] \(message)"
                    )
                    messages.append(
                        Message(
                            sessionID: sessionID,
                            role: .system,
                            kind: .error,
                            body: message,
                            createdAt: .now,
                            relatedTaskID: task.id
                        )
                    )
                    updateSession(sessionID) {
                        $0.updatedAt = .now
                        $0.state = .failed
                        $0.messageCount += 1
                        $0.latestTaskID = task.id
                    }
                    lastError = message
                case .cancelled:
                    let sessionID = currentSessionID(for: task.id, fallback: pendingSessionID)
                    updateTask(task.id) {
                        $0.status = .cancelled
                        $0.finishedAt = .now
                        $0.timeline.append(
                            TimelineEntry(
                                title: "Cancelled",
                                detail: "The running CLI task was stopped.",
                                level: .warning
                            )
                        )
                    }
                    _ = upsertRuntimeStatusMessage(
                        taskID: task.id,
                        sessionID: sessionID,
                        text: "Cancelled\nThe running CLI task was stopped."
                    )
                    updateSession(sessionID) {
                        $0.updatedAt = .now
                        $0.state = .cancelled
                        $0.latestTaskID = task.id
                    }
                }
                persist()
            }
        } catch {
            updateTask(task.id) {
                $0.status = .failed
                $0.finishedAt = .now
                $0.errorText = error.localizedDescription
            }
            lastError = error.localizedDescription
        }

        activeExecutionID = nil
        persist()
        await refresh()
    }

    public func archive() -> ProjectArchive {
        ProjectArchive(
            project: project,
            sessions: sessions,
            messages: messages,
            tasks: tasks,
            selectedSessionID: selectedSessionID,
            composerDraft: composerText,
            uiState: ProjectUIState(
                sidebarQuery: sidebarQuery,
                selectedFileChangeID: selectedFileChangeID,
                isInspectorVisible: isInspectorVisible,
                inspectorPanel: inspectorPanel.rawValue
            )
        )
    }

    private var resolvedRuntimeSessionID: String? {
        guard let selectedSessionID, !selectedSessionID.hasPrefix("pending-") else { return nil }
        return selectedSessionID
    }

    private func apply(snapshot: ProjectSnapshot) {
        self.snapshot = snapshot
        if let profile = snapshot.status.profile, !profile.isEmpty {
            selectedProfile = profile
            project.preferredProfile = profile
            project.recentProfiles = Self.uniqueProfiles([profile] + project.recentProfiles)
        }

        let pinnedIDs = Set(sessions.filter(\.isPinned).map(\.id))
        let mapped = snapshot.sessions.map { item in
            let existing = sessions.first(where: { $0.id == item.id })
            return Session(
                id: item.id,
                projectID: project.id,
                title: existing?.title ?? Self.sessionTitle(for: item.id, fallback: "Session \(item.id.prefix(8))"),
                summary: existing?.summary ?? item.id,
                createdAt: existing?.createdAt ?? Self.date(from: item.lastUpdated),
                updatedAt: Self.date(from: item.lastUpdated),
                messageCount: item.eventCount,
                taskCount: existing?.taskCount ?? tasks.filter { $0.sessionID == item.id }.count,
                state: existing?.state ?? .idle,
                isPinned: pinnedIDs.contains(item.id),
                latestTaskID: existing?.latestTaskID
            )
        }

        let pendingSessions = sessions.filter { $0.id.hasPrefix("pending-") }
        sessions = (mapped + pendingSessions)
            .reduce(into: [String: Session]()) { partial, session in
                partial[session.id] = session
            }
            .values
            .sorted { $0.updatedAt > $1.updatedAt }

        if let transcript = snapshot.transcript {
            replaceTranscriptMessages(sessionID: transcript.sessionID, events: transcript.events)
            if let title = transcript.events.first(where: { $0.type == .user })?.text {
                updateSession(transcript.sessionID) {
                    $0.title = Self.sessionTitle(for: title, fallback: $0.title)
                    $0.summary = Self.firstLine(of: transcript.events.last?.text ?? title)
                }
            }
        }

        if selectedSessionID == nil {
            selectedSessionID = sessions.first?.id
        } else if let selectedSessionID, !sessions.contains(where: { $0.id == selectedSessionID }) {
            self.selectedSessionID = sessions.first?.id
        }
        if let selectedTaskID, !selectedTasks.contains(where: { $0.id == selectedTaskID }) {
            self.selectedTaskID = nil
        }
        if selectedTaskID == nil {
            selectedTaskID = selectedTasks.first?.id
        }
        syncInspectorSelectionsForCurrentTask()
        if let selectedMessageID, !selectedMessages.contains(where: { $0.id == selectedMessageID }) {
            self.selectedMessageID = nil
        }
    }

    private func replaceTranscriptMessages(sessionID: String, events: [SessionEvent]) {
        messages.removeAll {
            $0.sessionID == sessionID
                && $0.kind != .task
                && !shouldPreserveRuntimeMessage($0)
        }
        let existingSignatures = Set(
            messages
                .filter { $0.sessionID == sessionID }
                .map(Self.messageSignature(_:))
        )
        let mappedMessages = events.map { mapTranscriptMessage(event: $0, sessionID: sessionID) }
        messages.append(contentsOf: mappedMessages.filter { !existingSignatures.contains(Self.messageSignature($0)) })
    }

    private func ensureSessionExists(id: String, title: String) {
        guard !sessions.contains(where: { $0.id == id }) else { return }
        sessions.insert(
            Session(
                id: id,
                projectID: project.id,
                title: title,
                summary: "New session",
                state: .waiting
            ),
            at: 0
        )
    }

    private func reassignPendingSession(from pendingID: String, to resolvedID: String) {
        guard pendingID != resolvedID else { return }
        messages.indices.forEach { index in
            if messages[index].sessionID == pendingID {
                messages[index].sessionID = resolvedID
            }
        }
        tasks.indices.forEach { index in
            if tasks[index].sessionID == pendingID {
                tasks[index].sessionID = resolvedID
            }
        }

        if let pendingIndex = sessions.firstIndex(where: { $0.id == pendingID }) {
            var pending = sessions.remove(at: pendingIndex)
            pending.projectID = project.id
            pending.title = pending.title == "Running Task" ? "Task Session" : pending.title
            if let existingIndex = sessions.firstIndex(where: { $0.id == resolvedID }) {
                sessions[existingIndex].updatedAt = .now
                sessions[existingIndex].latestTaskID = pending.latestTaskID
                sessions[existingIndex].taskCount = max(sessions[existingIndex].taskCount, pending.taskCount)
            } else {
                pending = Session(
                    id: resolvedID,
                    projectID: project.id,
                    title: pending.title,
                    summary: pending.summary,
                    createdAt: pending.createdAt,
                    updatedAt: .now,
                    messageCount: pending.messageCount,
                    taskCount: pending.taskCount,
                    state: pending.state,
                    isPinned: pending.isPinned,
                    latestTaskID: pending.latestTaskID
                )
                sessions.insert(pending, at: 0)
            }
        }

        selectedSessionID = resolvedID
    }

    private func updateTask(_ taskID: UUID, change: (inout AgentTask) -> Void) {
        guard let index = tasks.firstIndex(where: { $0.id == taskID }) else { return }
        change(&tasks[index])
    }

    private func updateSession(_ sessionID: String, change: (inout Session) -> Void) {
        guard let index = sessions.firstIndex(where: { $0.id == sessionID }) else { return }
        change(&sessions[index])
    }

    private func currentSessionID(for taskID: UUID, fallback: String) -> String {
        tasks.first(where: { $0.id == taskID })?.sessionID ?? fallback
    }

    @discardableResult
    private func upsertAssistantMessage(taskID: UUID, sessionID: String, body: String) -> (body: String, inserted: Bool) {
        guard !body.isEmpty else {
            let existingBody = messages.first(where: { $0.relatedTaskID == taskID && $0.role == .assistant })?.body ?? ""
            return (existingBody, false)
        }

        if let index = messages.firstIndex(where: { $0.relatedTaskID == taskID && $0.role == .assistant }) {
            let mergedBody = Self.mergeAssistantBody(existing: messages[index].body, incoming: body)
            messages[index].sessionID = sessionID
            messages[index].body = mergedBody
            return (mergedBody, false)
        }

        messages.append(
            Message(
                sessionID: sessionID,
                role: .assistant,
                body: body,
                createdAt: .now,
                relatedTaskID: taskID
            )
        )
        return (body, true)
    }

    @discardableResult
    private func upsertRuntimeStatusMessage(taskID: UUID, sessionID: String, text: String) -> Bool {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        let body = Self.statusBlockBody(normalized)
        if let index = messages.firstIndex(where: { $0.relatedTaskID == taskID && $0.kind == .status }) {
            messages[index].sessionID = sessionID
            messages[index].body = body
            return false
        }

        messages.append(
            Message(
                sessionID: sessionID,
                role: .system,
                kind: .status,
                body: body,
                createdAt: .now,
                relatedTaskID: taskID
            )
        )
        return true
    }

    @discardableResult
    private func appendRuntimeLogEntry(taskID: UUID, sessionID: String, entry: String) -> Bool {
        let normalized = entry.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        if let index = messages.firstIndex(where: { $0.relatedTaskID == taskID && $0.kind == .log }) {
            let existingLines = Self.blockLines(in: messages[index].body)
            let mergedLines = Self.mergedLogLines(existing: existingLines, incoming: normalized)
            messages[index].sessionID = sessionID
            messages[index].body = Self.logBlockBody(mergedLines.joined(separator: "\n"))
            return false
        }

        messages.append(
            Message(
                sessionID: sessionID,
                role: .system,
                kind: .log,
                body: Self.logBlockBody(normalized),
                createdAt: .now,
                relatedTaskID: taskID
            )
        )
        return true
    }

    @discardableResult
    private func appendActivityMessage(
        taskID: UUID,
        sessionID: String,
        title: String,
        name: String,
        detail: String,
        language: String?
    ) -> Bool {
        let payload = Self.makeActivityPayload(
            phase: title == "Tool result" ? .toolResult : .toolCall,
            name: name,
            detail: detail,
            language: language
        )
        messages.append(
            Message(
                sessionID: sessionID,
                role: .system,
                kind: .activity,
                body: payload.encodedString(),
                createdAt: .now,
                relatedTaskID: taskID
            )
        )
        return true
    }

    private func taskTitle(for prompt: String) -> String {
        Self.firstLine(of: prompt, fallback: "Agent Task")
    }

    private func persist() {
        project.lastOpenedAt = .now
        Task { await onArchiveChange(archive()) }
    }

    private static func pendingSessionID() -> String {
        "pending-\(UUID().uuidString)"
    }

    private func mapTranscriptMessage(event: SessionEvent, sessionID: String) -> Message {
        let role: MessageRole
        let kind: MessageKind
        let body: String
        let relatedTaskID = inferRelatedTaskID(for: event, sessionID: sessionID)

        switch event.type {
        case .user:
            role = .user
            kind = .markdown
            body = event.text
        case .assistant:
            role = .assistant
            kind = .markdown
            body = event.text
        case .toolCall:
            role = .system
            kind = .activity
            body = Self.activityPayload(for: .toolCall, rawText: event.text).encodedString()
        case .toolResult:
            role = .system
            kind = .activity
            body = Self.activityPayload(for: .toolResult, rawText: event.text).encodedString()
        case .system:
            role = .system
            kind = .status
            body = event.text
        case .error:
            role = .system
            kind = .error
            body = event.text
        }

        return Message(
            sessionID: event.sessionID,
            role: role,
            kind: kind,
            body: body,
            createdAt: Self.date(from: event.timestamp),
            relatedTaskID: relatedTaskID
        )
    }

    private func inferRelatedTaskID(for event: SessionEvent, sessionID: String) -> UUID? {
        let sessionTasks = tasks
            .filter { $0.sessionID == sessionID }
            .sorted { $0.createdAt < $1.createdAt }
        guard !sessionTasks.isEmpty else { return nil }
        if sessionTasks.count == 1 {
            return sessionTasks[0].id
        }

        let eventText = Self.normalizedText(event.text)
        let eventDate = Self.date(from: event.timestamp)

        func score(for task: AgentTask) -> Int {
            var score = 0
            let prompt = Self.normalizedText(task.prompt)
            let summary = Self.normalizedText(task.summary)
            let response = Self.normalizedText(task.responseText)

            switch event.type {
            case .user:
                if !eventText.isEmpty {
                    if prompt == eventText {
                        score += 1000
                    } else if prompt.contains(eventText) || eventText.contains(prompt) {
                        score += 700
                    }
                }
            case .assistant:
                if !eventText.isEmpty {
                    if response == eventText {
                        score += 1000
                    } else if response.contains(eventText) || eventText.contains(response) {
                        score += 700
                    } else if summary == eventText {
                        score += 500
                    } else if !summary.isEmpty, summary.contains(Self.firstLine(of: eventText, fallback: summary)) {
                        score += 300
                    }
                }
            case .toolCall, .toolResult, .system, .error:
                score += 200
            }

            let anchorDate: Date
            switch event.type {
            case .assistant:
                anchorDate = task.finishedAt ?? task.startedAt ?? task.createdAt
            default:
                anchorDate = task.startedAt ?? task.createdAt
            }
            let delta = abs(anchorDate.timeIntervalSince(eventDate))
            score += max(0, 240 - Int(delta))
            return score
        }

        return sessionTasks.max { lhs, rhs in
            let lhsScore = score(for: lhs)
            let rhsScore = score(for: rhs)
            if lhsScore == rhsScore {
                let lhsDelta = abs((lhs.finishedAt ?? lhs.startedAt ?? lhs.createdAt).timeIntervalSince(eventDate))
                let rhsDelta = abs((rhs.finishedAt ?? rhs.startedAt ?? rhs.createdAt).timeIntervalSince(eventDate))
                return lhsDelta > rhsDelta
            }
            return lhsScore < rhsScore
        }?.id
    }

    private static func date(from raw: String) -> Date {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: raw) {
            return date
        }
        let fallback = ISO8601DateFormatter()
        fallback.formatOptions = [.withInternetDateTime]
        if let date = fallback.date(from: raw) {
            return date
        }
        return .now
    }

    private static func firstLine(of text: String, fallback: String = "Untitled") -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return fallback }
        if let first = trimmed.split(separator: "\n").first {
            return String(first.prefix(96))
        }
        return String(trimmed.prefix(96))
    }

    private static func sessionTitle(for raw: String, fallback: String) -> String {
        let first = firstLine(of: raw, fallback: fallback)
        return first.isEmpty ? fallback : first
    }

    private static func mergeAssistantBody(existing: String, incoming: String) -> String {
        guard !incoming.isEmpty else { return existing }
        guard !existing.isEmpty else { return incoming }
        if incoming.hasPrefix(existing) {
            return incoming
        }
        if existing.hasPrefix(incoming) {
            return existing
        }
        return existing + incoming
    }

    private static func statusBlockBody(_ text: String) -> String {
        "```status\n\(text)\n```"
    }

    private static func logBlockBody(_ text: String) -> String {
        "```log\n\(text)\n```"
    }

    private static func blockLines(in body: String) -> [String] {
        body
            .replacingOccurrences(of: "```log\n", with: "")
            .replacingOccurrences(of: "\n```", with: "")
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
    }

    private static func mergedLogLines(existing: [String], incoming: String, limit: Int = 16) -> [String] {
        let candidate = incoming.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !candidate.isEmpty else { return existing }
        if existing.last == candidate {
            return existing
        }
        return Array((existing + [candidate]).suffix(limit))
    }

    private static func logLabel(for stream: CLIEventStream) -> String {
        switch stream {
        case .stderr:
            return "[stderr] "
        case .status:
            return "[status] "
        case .command:
            return "[command] "
        case .system:
            return "[system] "
        case .stdout:
            return "[stdout] "
        }
    }

    private func shouldSurface(event: CLIEvent) -> Bool {
        event.isImportant || event.stream == .stderr || event.stream == .status || event.stream == .command
    }

    private func shouldPreserveRuntimeMessage(_ message: Message) -> Bool {
        guard message.relatedTaskID != nil else { return false }
        switch message.kind {
        case .activity, .log, .error, .status:
            return true
        case .markdown:
            return message.role == .system
        case .task:
            return false
        }
    }

    private static func makeActivityPayload(
        phase: ActivityMessagePhase,
        name: String,
        detail: String,
        language _: String?
    ) -> ActivityMessagePayload {
        let normalizedDetail = detail.trimmingCharacters(in: .whitespacesAndNewlines)
        let object = jsonObject(from: normalizedDetail)
        let preview = activityPreview(phase: phase, name: name, object: object)
        return ActivityMessagePayload(
            phase: phase,
            name: name,
            summary: activitySummary(phase: phase, name: name, object: object, detail: normalizedDetail),
            fields: activityFields(phase: phase, name: name, object: object),
            previewTitle: preview.title,
            previewText: preview.text,
            previewKind: preview.kind,
            detail: normalizedDetail.isEmpty ? nil : normalizedDetail
        )
    }

    private static func activityPayload(for phase: ActivityMessagePhase, rawText: String) -> ActivityMessagePayload {
        let normalized = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard
            let data = normalized.data(using: .utf8),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return ActivityMessagePayload(
                phase: phase,
                name: phase == .toolCall ? "tool" : "result",
                summary: normalized.isEmpty ? phase.title : normalized,
                detail: nil
            )
        }

        let name = object["name"] as? String ?? (phase == .toolCall ? "tool" : "result")
        let fragment = phase == .toolCall ? object["args"] : object["result"]
        let detail = renderJSONFragment(fragment)
        return makeActivityPayload(phase: phase, name: name, detail: detail, language: "json")
    }

    private static func activitySummary(
        phase: ActivityMessagePhase,
        name: String,
        object: [String: Any]?,
        detail: String
    ) -> String {
        guard !detail.isEmpty, let object else {
            return phase == .toolCall ? "Preparing \(name)" : "Completed \(name)"
        }

        switch name {
        case "read_file":
            if let path = object["path"] as? String {
                return path
            }
        case "write_file", "edit_file":
            if let path = object["path"] as? String {
                return phase == .toolResult ? "Wrote \(path)" : path
            }
        case "search_text":
            if phase == .toolResult, let matches = object["matches"] as? [Any] {
                return searchSummary(matches: matches, truncated: object["truncated"] as? Bool ?? false)
            }
            if let query = object["query"] as? String {
                return query
            }
            if let pattern = object["pattern"] as? String {
                return pattern
            }
        case "run_cmd":
            if phase == .toolResult,
               let stderr = object["stderr"] as? String,
               !stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                return firstLine(of: stderr, fallback: "Command failed")
            }
            if let command = object["command"] as? String {
                return command
            }
            if let exitCode = object["exit_code"] as? NSNumber {
                return "exit \(exitCode.intValue)"
            }
        default:
            break
        }

        if let path = object["path"] as? String {
            return path
        }
        if let command = object["command"] as? String {
            return command
        }
        if let query = object["query"] as? String {
            return query
        }
        if let pattern = object["pattern"] as? String {
            return pattern
        }
        if let exitCode = object["exit_code"] as? NSNumber {
            return "exit \(exitCode.intValue)"
        }
        if let content = object["content"] as? String {
            return firstLine(of: content, fallback: phase.title)
        }

        return phase == .toolCall ? "Preparing \(name)" : "Completed \(name)"
    }

    private static func activityFields(
        phase: ActivityMessagePhase,
        name: String,
        object: [String: Any]?
    ) -> [ActivityMessageField] {
        guard let object else { return [] }

        var fields: [ActivityMessageField] = []

        func append(_ label: String, _ value: String?) {
            guard let value else { return }
            let normalized = value.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !normalized.isEmpty else { return }
            fields.append(ActivityMessageField(label: label, value: normalized))
        }

        switch name {
        case "run_cmd":
            append("Command", object["command"] as? String)
            append("Directory", object["cwd"] as? String)
            append("Approval", object["approved_by"] as? String)
            if phase == .toolResult {
                append("Exit", numberString(object["exit_code"]))
                append("Duration", formattedDuration(object["duration_ms"]))
            }
        case "read_file":
            append("Path", object["path"] as? String)
            if phase == .toolResult {
                append("Lines", lineCountString(object["content"] as? String))
            }
        case "write_file", "edit_file":
            append("Path", object["path"] as? String)
            if phase == .toolCall {
                append("Lines", lineCountString(object["content"] as? String))
            } else if (object["written"] as? Bool) == true {
                append("Status", "written")
            }
        case "search_text":
            append("Pattern", (object["query"] as? String) ?? (object["pattern"] as? String))
            append("Path", object["path"] as? String)
            if phase == .toolResult, let matches = object["matches"] as? [Any] {
                append("Matches", "\(matches.count)")
                append("Truncated", boolString(object["truncated"] as? Bool))
            }
        default:
            append("Path", object["path"] as? String)
            append("Command", object["command"] as? String)
            append("Query", object["query"] as? String)
        }

        if fields.isEmpty {
            append("Path", object["path"] as? String)
            append("Command", object["command"] as? String)
            append("Exit", numberString(object["exit_code"]))
        }

        return fields
    }

    private static func activityPreview(
        phase: ActivityMessagePhase,
        name: String,
        object: [String: Any]?
    ) -> (title: String?, text: String?, kind: ActivityPreviewKind?) {
        guard let object else {
            return (nil, nil, nil)
        }

        switch name {
        case "run_cmd":
            guard phase == .toolResult else { return (nil, nil, nil) }
            let stderr = previewText(from: object["stderr"] as? String)
            let stdout = previewText(from: object["stdout"] as? String)
            let exitCode = (object["exit_code"] as? NSNumber)?.intValue

            if let stderr, !stderr.isEmpty {
                let kind: ActivityPreviewKind = (exitCode ?? 0) == 0 ? .warning : .failure
                return ("stderr", stderr, kind)
            }
            if let stdout, !stdout.isEmpty {
                return ("stdout", stdout, .success)
            }
            if let exitCode, exitCode != 0 {
                return ("status", "Command exited with code \(exitCode).", .failure)
            }
        case "read_file":
            guard phase == .toolResult else { return (nil, nil, nil) }
            if let content = previewText(from: object["content"] as? String, lineLimit: 6, characterLimit: 420) {
                return ("content", content, .neutral)
            }
        case "write_file", "edit_file":
            if phase == .toolCall,
               let content = previewText(from: object["content"] as? String, lineLimit: 5, characterLimit: 360) {
                return ("content", content, .neutral)
            }
            if phase == .toolResult, (object["written"] as? Bool) == true {
                return ("status", "File write completed.", .success)
            }
        case "search_text":
            guard phase == .toolResult else { return (nil, nil, nil) }
            if let preview = searchMatchPreview(from: object["matches"] as? [Any]) {
                return ("matches", preview, .neutral)
            }
        default:
            break
        }
        return (nil, nil, nil)
    }

    private static func jsonObject(from text: String) -> [String: Any]? {
        guard !text.isEmpty, let data = text.data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    private static func numberString(_ value: Any?) -> String? {
        if let number = value as? NSNumber {
            return number.stringValue
        }
        if let string = value as? String {
            return string
        }
        return nil
    }

    private static func formattedDuration(_ value: Any?) -> String? {
        guard let raw = value as? NSNumber else { return nil }
        let milliseconds = raw.doubleValue
        if milliseconds >= 1000 {
            return String(format: "%.2fs", milliseconds / 1000)
        }
        return "\(raw.intValue) ms"
    }

    private static func lineCountString(_ value: String?) -> String? {
        guard let value else { return nil }
        let normalized = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return nil }
        return "\(normalized.split(separator: "\n", omittingEmptySubsequences: false).count)"
    }

    private static func boolString(_ value: Bool?) -> String? {
        guard let value else { return nil }
        return value ? "Yes" : "No"
    }

    private static func searchSummary(matches: [Any], truncated: Bool) -> String {
        let count = matches.count
        let base = count == 1 ? "1 match" : "\(count) matches"
        return truncated ? "\(base) (truncated)" : base
    }

    private static func searchMatchPreview(from matches: [Any]?, limit: Int = 4) -> String? {
        guard let matches, !matches.isEmpty else { return nil }
        let lines = matches.prefix(limit).compactMap { raw -> String? in
            guard let object = raw as? [String: Any] else { return nil }
            let path = object["path"] as? String ?? "file"
            let line = numberString(object["line_number"]) ?? "-"
            let text = (object["line"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            return "\(path):\(line) \(text)"
        }
        let preview = lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        return preview.isEmpty ? nil : preview
    }

    private func preferredInspectorPanel(for payload: ActivityMessagePayload, taskID: UUID) -> InspectorPanel {
        switch payload.name {
        case "run_cmd":
            return payload.phase == .toolCall ? .commands : .logs
        case "write_file", "edit_file":
            return matchingFileChange(for: payload, taskID: taskID) != nil ? .changes : .timeline
        case "read_file", "search_text":
            return .timeline
        default:
            return .overview
        }
    }

    private func matchingFileChange(for payload: ActivityMessagePayload, taskID: UUID) -> FileChange? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        guard let payloadPath = payload.fields.first(where: { $0.label == "Path" })?.value else { return nil }
        return task.fileChanges.first { change in
            change.path == payloadPath
                || payloadPath.hasSuffix(change.path)
                || change.path.hasSuffix(payloadPath)
                || change.previousPath == payloadPath
        }
    }

    private func matchingActivityMessage(for change: FileChange, taskID: UUID) -> Message? {
        messages
            .filter { $0.relatedTaskID == taskID && $0.kind == .activity }
            .first { message in
                guard let payload = ActivityMessagePayload.decode(from: message.body) else { return false }
                guard let payloadPath = payload.fields.first(where: { $0.label == "Path" })?.value else { return false }
                return payloadPath == change.path
                    || payloadPath.hasSuffix(change.path)
                    || change.path.hasSuffix(payloadPath)
                    || change.previousPath == payloadPath
            }
    }

    private func matchingCommandMessage(for command: CommandInvocation, taskID: UUID) -> Message? {
        messages
            .filter { $0.relatedTaskID == taskID }
            .first { message in
                (message.kind == .log && message.body.contains(command.displayCommand))
                    || (message.kind == .activity && message.body.contains(command.displayCommand))
            }
    }

    private func matchingCommand(for message: Message, taskID: UUID) -> CommandInvocation? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        return task.commands.first { command in
            message.body.contains(command.displayCommand)
        }
    }

    private func matchingCommand(for payload: ActivityMessagePayload, taskID: UUID) -> CommandInvocation? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        if let displayCommand = payload.fields.first(where: { $0.label == "Command" })?.value {
            return task.commands.first { command in
                command.displayCommand == displayCommand || displayCommand.contains(command.displayCommand)
            }
        }
        return nil
    }

    private func matchingStatusMessage(for taskID: UUID, entry: TimelineEntry) -> Message? {
        let taskMessages = messages.filter { $0.relatedTaskID == taskID }
        if let exact = taskMessages.first(where: {
            $0.kind == .status && ($0.body.contains(entry.title) || $0.body.contains(entry.detail))
        }) {
            return exact
        }
        return taskMessages.last(where: { $0.kind == .status || $0.kind == .task })
    }

    private func matchingLogMessage(for taskID: UUID, event: CLIEvent) -> Message? {
        let taskMessages = messages.filter { $0.relatedTaskID == taskID }
        if let exact = taskMessages.first(where: {
            $0.kind == .log && ($0.body.contains(event.text) || $0.body.contains(event.stream.rawValue))
        }) {
            return exact
        }
        return taskMessages.last(where: { $0.kind == .log })
    }

    private func matchingTimelineEntry(for message: Message, taskID: UUID) -> TimelineEntry? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        return task.timeline.first { entry in
            message.body.contains(entry.title) || message.body.contains(entry.detail)
        } ?? task.timeline.last
    }

    private func matchingTimelineEntry(for payload: ActivityMessagePayload, taskID: UUID) -> TimelineEntry? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        let candidates = [payload.name, payload.summary, payload.detail]
        return task.timeline.last { entry in
            candidates.compactMap { $0 }.contains { candidate in
                entry.detail.contains(candidate) || entry.title.contains(candidate)
            }
        } ?? task.timeline.last
    }

    private func matchingCLIEvent(for message: Message, taskID: UUID) -> CLIEvent? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        return task.cliEvents.first { event in
            message.body.contains(event.text) || message.body.contains(event.stream.rawValue)
        } ?? task.cliEvents.last
    }

    private func matchingCLIEvent(for payload: ActivityMessagePayload, taskID: UUID) -> CLIEvent? {
        guard let task = tasks.first(where: { $0.id == taskID }) else { return nil }
        let candidates = [payload.previewText, payload.summary, payload.detail]
        return task.cliEvents.last { event in
            candidates.compactMap { $0 }.contains { candidate in
                candidate.contains(event.text) || event.text.contains(candidate)
            }
        } ?? task.cliEvents.last
    }

    private func syncInspectorSelectionsForCurrentTask() {
        guard let task = selectedTask else {
            selectedTimelineEntryID = nil
            selectedCLIEventID = nil
            selectedCommandID = nil
            selectedFileChangeID = nil
            return
        }

        if !task.timeline.contains(where: { $0.id == selectedTimelineEntryID }) {
            selectedTimelineEntryID = nil
        }
        if !task.cliEvents.contains(where: { $0.id == selectedCLIEventID }) {
            selectedCLIEventID = nil
        }
        if !task.commands.contains(where: { $0.id == selectedCommandID }) {
            selectedCommandID = nil
        }
        if !task.fileChanges.contains(where: { $0.id == selectedFileChangeID }) {
            selectedFileChangeID = task.fileChanges.first?.id
        }
    }

    private static func previewText(from raw: String?, lineLimit: Int = 4, characterLimit: Int = 320) -> String? {
        guard let raw else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let lines = trimmed
            .split(separator: "\n", omittingEmptySubsequences: false)
            .prefix(lineLimit)
            .map(String.init)
        let joined = lines.joined(separator: "\n")
        return String(joined.prefix(characterLimit))
    }

    private static func renderJSONFragment(_ value: Any?) -> String {
        guard let value else { return "" }
        guard JSONSerialization.isValidJSONObject(value) else { return String(describing: value) }
        guard
            let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]),
            let text = String(data: data, encoding: .utf8)
        else {
            return String(describing: value)
        }
        return text
    }

    private static func uniqueProfiles(_ values: [String]) -> [String] {
        var seen = Set<String>()
        return values.filter {
            let normalized = $0.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !normalized.isEmpty, !seen.contains(normalized) else { return false }
            seen.insert(normalized)
            return true
        }
    }

    private static func normalizedText(_ value: String) -> String {
        value
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
    }

    private static func messageSignature(_ message: Message) -> String {
        if message.kind == .activity,
           let payload = ActivityMessagePayload.decode(from: message.body) {
            let fields = payload.fields
                .map { "\($0.label)=\($0.value)" }
                .joined(separator: "&")
            return [
                message.sessionID,
                message.role.rawValue,
                message.kind.rawValue,
                message.relatedTaskID?.uuidString ?? "none",
                payload.phase.rawValue,
                payload.name,
                payload.summary,
                fields,
            ].joined(separator: "|")
        }

        return [
            message.sessionID,
            message.role.rawValue,
            message.kind.rawValue,
            message.relatedTaskID?.uuidString ?? "none",
            message.body.trimmingCharacters(in: .whitespacesAndNewlines),
        ].joined(separator: "|")
    }
}
