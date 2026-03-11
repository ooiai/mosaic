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
    public private(set) var activeExecutionID: UUID?
    public var sidebarQuery = ""
    public var composerText = ""
    public var composerMode: ComposerMode = .execute
    public var selectedProfile: String
    public var isInspectorVisible = true
    public var inspectorPanel: InspectorPanel = .overview
    public var selectedFileChangeID: UUID?
    public private(set) var isLoadingSnapshot = false
    public private(set) var lastError: String?

    private let runtime: AgentWorkbenchRuntime
    private let pinnedSessionsStore: PinnedSessionsStoring
    private let onArchiveChange: @Sendable (ProjectArchive) async -> Void

    public init(
        project: Project,
        archive: ProjectArchive?,
        runtime: AgentWorkbenchRuntime,
        pinnedSessionsStore: PinnedSessionsStoring,
        onArchiveChange: @escaping @Sendable (ProjectArchive) async -> Void
    ) {
        self.project = project
        self.runtime = runtime
        self.pinnedSessionsStore = pinnedSessionsStore
        self.onArchiveChange = onArchiveChange
        self.sessions = archive?.sessions ?? []
        self.messages = archive?.messages ?? []
        self.tasks = archive?.tasks ?? []
        self.selectedSessionID = archive?.selectedSessionID
        self.composerText = archive?.composerDraft ?? ""
        self.selectedProfile = archive?.project.preferredProfile ?? project.preferredProfile
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

    public var currentModelLabel: String {
        snapshot?.modelsStatus?.effectiveModel ?? snapshot?.status.provider?.model ?? "Unknown"
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
            let snapshot = try await runtime.loadSnapshot(
                project: project,
                selectedSessionID: resolvedRuntimeSessionID
            )
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
        inspectorPanel = .overview
        persist()
        Task { await refresh() }
    }

    public func selectTask(_ taskID: UUID) {
        selectedTaskID = taskID
        if let task = tasks.first(where: { $0.id == taskID }) {
            selectedSessionID = task.sessionID
        }
        if selectedFileChangeID == nil {
            selectedFileChangeID = selectedTask?.fileChanges.first?.id
        }
        persist()
    }

    public func selectFileChange(_ fileChangeID: UUID) {
        selectedFileChangeID = fileChangeID
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

        var task = AgentTask(
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
                case let .timeline(entry):
                    updateTask(task.id) {
                        $0.timeline.append(entry)
                    }
                case let .cliEvent(event):
                    updateTask(task.id) {
                        $0.cliEvents.append(event)
                    }
                case let .sessionStarted(sessionID):
                    reassignPendingSession(from: pendingSessionID, to: sessionID)
                case let .messageDelta(text):
                    updateTask(task.id) {
                        $0.summary = Self.firstLine(of: text)
                        $0.responseText = text
                    }
                case let .fileChanges(changes):
                    updateTask(task.id) {
                        $0.fileChanges = changes
                    }
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
                    messages.append(
                        Message(
                            sessionID: resolvedSessionID,
                            role: .assistant,
                            body: response.response,
                            createdAt: .now,
                            relatedTaskID: task.id
                        )
                    )
                    updateSession(resolvedSessionID) {
                        $0.updatedAt = .now
                        $0.state = .done
                        $0.title = Self.sessionTitle(for: response.response, fallback: $0.title)
                        $0.summary = Self.firstLine(of: response.response)
                        $0.messageCount += 1
                        $0.latestTaskID = task.id
                    }
                case let .failed(message, exitCode):
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
                    messages.append(
                        Message(
                            sessionID: selectedSessionID ?? pendingSessionID,
                            role: .system,
                            kind: .error,
                            body: message,
                            createdAt: .now,
                            relatedTaskID: task.id
                        )
                    )
                    updateSession(selectedSessionID ?? pendingSessionID) {
                        $0.updatedAt = .now
                        $0.state = .failed
                        $0.latestTaskID = task.id
                    }
                    lastError = message
                case .cancelled:
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
                    updateSession(selectedSessionID ?? pendingSessionID) {
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
            composerDraft: composerText
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
        selectedTaskID = selectedTasks.first?.id
        selectedFileChangeID = selectedTask?.fileChanges.first?.id
    }

    private func replaceTranscriptMessages(sessionID: String, events: [SessionEvent]) {
        messages.removeAll {
            $0.sessionID == sessionID && $0.kind != .task
        }
        messages.append(contentsOf: events.map(Self.mapMessage(event:)))
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

    private static func mapMessage(event: SessionEvent) -> Message {
        let role: MessageRole
        let kind: MessageKind
        let body: String

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
            kind = .status
            body = "```status\n\(event.text)\n```"
        case .toolResult:
            role = .system
            kind = .log
            body = "```log\n\(event.text)\n```"
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
            createdAt: date(from: event.timestamp)
        )
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

    private static func uniqueProfiles(_ values: [String]) -> [String] {
        var seen = Set<String>()
        return values.filter {
            let normalized = $0.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !normalized.isEmpty, !seen.contains(normalized) else { return false }
            seen.insert(normalized)
            return true
        }
    }
}
