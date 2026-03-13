import Domain
import Features
import Infrastructure
import XCTest

actor AsyncGate {
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func wait() async {
        await withCheckedContinuation { continuation in
            waiters.append(continuation)
        }
    }

    func open() {
        let continuations = waiters
        waiters.removeAll()
        continuations.forEach { $0.resume() }
    }
}

@MainActor
final class WorkbenchViewModelTests: XCTestCase {
    func testRefreshLoadsSnapshotIntoSessionsAndMessages() async {
        let runtime = MockWorkbenchRuntime()
        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: runtime,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.sessions.first?.id, PreviewFixtures.session.id)
        XCTAssertTrue(viewModel.selectedMessages.contains(where: { $0.role == .assistant }))
        XCTAssertEqual(viewModel.currentModelLabel, PreviewFixtures.modelsStatusSummary.effectiveModel)
    }

    func testSendCurrentPromptConsumesRuntimeExecution() async throws {
        let runtime = MockWorkbenchRuntime()
        var didSend = false
        let newFile = FileChange(
            path: "apps/macos/Sources/UI/App/WorkbenchView.swift",
            status: .modified,
            additions: 8,
            deletions: 2,
            diff: "@@ -1,1 +1,1 @@"
        )
        runtime.snapshotHandler = { _, _ in
            if didSend {
                return ProjectSnapshot(
                    status: PreviewFixtures.statusSummary,
                    health: PreviewFixtures.healthSummary,
                    configuration: PreviewFixtures.configurationSummary,
                    modelsStatus: PreviewFixtures.modelsStatusSummary,
                    availableModels: PreviewFixtures.modelList,
                    sessions: PreviewFixtures.sessions,
                    transcript: SessionTranscript(
                        sessionID: PreviewFixtures.session.id,
                        events: PreviewFixtures.transcript.events + [
                            SessionEvent(
                                id: "msg-5",
                                sessionID: PreviewFixtures.session.id,
                                type: .assistant,
                                timestamp: "2026-03-10T09:19:00Z",
                                text: "Agent completed the task."
                            ),
                        ]
                    )
                )
            }
            return PreviewFixtures.projectSnapshot
        }
        runtime.startTaskHandler = { request in
            didSend = true
            let command = CommandInvocation(
                displayCommand: "mosaic chat --emit-events",
                executablePath: "/usr/local/bin/mosaic",
                arguments: ["chat", "--emit-events"],
                workingDirectory: request.project.workspacePath,
                status: .running
            )
            return RuntimeExecution(
                id: UUID(),
                events: AsyncThrowingStream { continuation in
                    continuation.yield(.command(command))
                    continuation.yield(.timeline(TimelineEntry(title: "Started", detail: "Mock runtime started")))
                    continuation.yield(.sessionStarted(PreviewFixtures.session.id))
                    continuation.yield(.toolCall(name: "read_file", detail: "{\n  \"path\": \"README.md\"\n}"))
                    continuation.yield(.toolResult(name: "read_file", detail: "{\n  \"path\": \"/tmp/workspace/README.md\",\n  \"content\": \"# README\\nAgent desktop notes\"\n}"))
                    continuation.yield(.toolCall(name: "write_file", detail: "{\n  \"path\": \"notes/plan.md\",\n  \"content\": \"# Plan\\n- Refine UI\"\n}"))
                    continuation.yield(.toolResult(name: "write_file", detail: "{\n  \"path\": \"/tmp/workspace/notes/plan.md\",\n  \"written\": true\n}"))
                    continuation.yield(.toolCall(name: "search_text", detail: "{\n  \"query\": \"TODO\",\n  \"path\": \"apps\"\n}"))
                    continuation.yield(.toolResult(name: "search_text", detail: "{\n  \"matches\": [\n    {\n      \"path\": \"/tmp/workspace/apps/macos/README.md\",\n      \"line_number\": 12,\n      \"line\": \"TODO: refine runtime cards\"\n    },\n    {\n      \"path\": \"/tmp/workspace/apps/macos/Sources/UI/ConversationView.swift\",\n      \"line_number\": 48,\n      \"line\": \"// TODO: compact tool cards\"\n    }\n  ],\n  \"truncated\": false\n}"))
                    continuation.yield(.toolCall(name: "run_cmd", detail: "{\n  \"command\": \"swift test\"\n}"))
                    continuation.yield(.toolResult(name: "run_cmd", detail: "{\n  \"command\": \"swift test\",\n  \"cwd\": \"/tmp/workspace\",\n  \"approved_by\": \"flag_yes\",\n  \"stdout\": \"\",\n  \"stderr\": \"Tests failed\\nExpected 1 passing test.\",\n  \"exit_code\": 1,\n  \"duration_ms\": 1280\n}"))
                    continuation.yield(.cliEvent(CLIEvent(taskID: UUID(), stream: .stdout, text: "stdout line")))
                    continuation.yield(.messageDelta("Agent completed the task."))
                    continuation.yield(.fileChanges([newFile]))
                    continuation.yield(.completed(
                        PromptResponse(
                            sessionID: PreviewFixtures.session.id,
                            response: "Agent completed the task.",
                            profile: "default",
                            agentID: "writer",
                            turns: 2
                        ),
                        exitCode: 0
                    ))
                    continuation.finish()
                },
                cancel: {}
            )
        }

        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: runtime,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        await viewModel.bootstrap()
        viewModel.composerText = "Finish the App Shell refactor."
        await viewModel.sendCurrentPrompt(settings: AppSettings())

        XCTAssertEqual(viewModel.selectedTask?.status, .done)
        XCTAssertEqual(viewModel.selectedTask?.fileChanges.first?.path, newFile.path)
        XCTAssertTrue(viewModel.selectedMessages.contains(where: { $0.role == .assistant && $0.body.contains("Agent completed") }))
        let activityPayloads = viewModel.selectedMessages
            .filter { $0.kind == .activity }
            .compactMap { ActivityMessagePayload.decode(from: $0.body) }
        XCTAssertTrue(activityPayloads.contains(where: {
            $0.phase == .toolCall && $0.name == "read_file" && $0.summary.contains("README.md")
        }))
        XCTAssertTrue(activityPayloads.contains(where: {
            $0.phase == .toolResult
                && $0.name == "read_file"
                && $0.summary.contains("README.md")
                && $0.previewTitle == "content"
                && $0.previewText?.contains("Agent desktop notes") == true
        }))
        XCTAssertTrue(activityPayloads.contains(where: {
            $0.phase == .toolCall
                && $0.name == "write_file"
                && $0.fields.contains(ActivityMessageField(label: "Path", value: "notes/plan.md"))
                && $0.previewTitle == "content"
        }))
        XCTAssertTrue(activityPayloads.contains(where: {
            $0.phase == .toolResult
                && $0.name == "write_file"
                && $0.summary.contains("notes/plan.md")
                && $0.previewTitle == "status"
                && $0.previewText == "File write completed."
                && $0.fields.contains(ActivityMessageField(label: "Status", value: "written"))
        }))
        XCTAssertTrue(activityPayloads.contains(where: {
            $0.phase == .toolResult
                && $0.name == "search_text"
                && $0.summary == "2 matches"
                && $0.previewTitle == "matches"
                && $0.previewText?.contains("README.md:12") == true
                && $0.fields.contains(ActivityMessageField(label: "Matches", value: "2"))
        }))
        XCTAssertTrue(activityPayloads.contains(where: {
            $0.phase == .toolResult
                && $0.name == "run_cmd"
                && $0.summary.contains("Tests failed")
                && $0.previewTitle == "stderr"
                && $0.previewText?.contains("Expected 1 passing test.") == true
                && $0.previewKind == .failure
                && $0.fields.contains(ActivityMessageField(label: "Command", value: "swift test"))
                && $0.fields.contains(ActivityMessageField(label: "Directory", value: "/tmp/workspace"))
                && $0.fields.contains(ActivityMessageField(label: "Exit", value: "1"))
                && $0.fields.contains(ActivityMessageField(label: "Duration", value: "1.28s"))
        }))
        XCTAssertTrue(viewModel.selectedMessages.contains(where: {
            $0.kind == .log && $0.body.contains("mosaic chat --emit-events")
        }))
    }

    func testSendCurrentPromptStreamsAssistantDraftWithoutDuplicateMessage() async throws {
        let runtime = MockWorkbenchRuntime()
        let completionGate = AsyncGate()
        var didSend = false

        runtime.snapshotHandler = { _, _ in
            if didSend {
                return ProjectSnapshot(
                    status: PreviewFixtures.statusSummary,
                    health: PreviewFixtures.healthSummary,
                    configuration: PreviewFixtures.configurationSummary,
                    modelsStatus: PreviewFixtures.modelsStatusSummary,
                    availableModels: PreviewFixtures.modelList,
                    sessions: PreviewFixtures.sessions,
                    transcript: SessionTranscript(
                        sessionID: PreviewFixtures.session.id,
                        events: PreviewFixtures.transcript.events + [
                            SessionEvent(
                                id: "msg-5",
                                sessionID: PreviewFixtures.session.id,
                                type: .assistant,
                                timestamp: "2026-03-10T09:19:00Z",
                                text: "Agent completed the task."
                            ),
                        ]
                    )
                )
            }
            return PreviewFixtures.projectSnapshot
        }

        runtime.startTaskHandler = { request in
            didSend = true
            let command = CommandInvocation(
                displayCommand: "mosaic chat --emit-events",
                executablePath: "/usr/local/bin/mosaic",
                arguments: ["chat", "--emit-events"],
                workingDirectory: request.project.workspacePath,
                status: .running
            )
            return RuntimeExecution(
                id: UUID(),
                events: AsyncThrowingStream { continuation in
                    Task {
                        continuation.yield(.command(command))
                        continuation.yield(.sessionStarted(PreviewFixtures.session.id))
                        continuation.yield(.messageDelta("Agent completed "))
                        await completionGate.wait()
                        continuation.yield(.messageDelta("the task."))
                        continuation.yield(.completed(
                            PromptResponse(
                                sessionID: PreviewFixtures.session.id,
                                response: "Agent completed the task.",
                                profile: "default",
                                agentID: "writer",
                                turns: 2
                            ),
                            exitCode: 0
                        ))
                        continuation.finish()
                    }
                },
                cancel: {}
            )
        }

        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: runtime,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        await viewModel.bootstrap()
        viewModel.composerText = "Finish the App Shell refactor."

        let sendTask = Task {
            await viewModel.sendCurrentPrompt(settings: AppSettings())
        }

        for _ in 0 ..< 30 {
            if viewModel.selectedMessages.contains(where: {
                $0.role == .assistant && $0.body == "Agent completed "
            }) {
                break
            }
            try await Task.sleep(for: .milliseconds(20))
        }

        let streamedMessages = viewModel.selectedMessages.filter {
            $0.role == .assistant && $0.relatedTaskID == viewModel.selectedTask?.id
        }
        XCTAssertEqual(streamedMessages.count, 1)
        XCTAssertEqual(streamedMessages.first?.body, "Agent completed ")

        await completionGate.open()
        await sendTask.value

        let completedMessages = viewModel.selectedMessages.filter {
            $0.role == .assistant && $0.body == "Agent completed the task."
        }
        XCTAssertEqual(completedMessages.count, 1)
        XCTAssertEqual(viewModel.selectedTask?.responseText, "Agent completed the task.")
    }

    func testTogglePinnedPersistsState() async {
        let pinnedStore = InMemoryPinnedSessionStore()
        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: PreviewFixtures.projectArchive,
            runtime: MockWorkbenchRuntime(),
            pinnedSessionsStore: pinnedStore
        ) { _ in }

        await viewModel.togglePinned(sessionID: PreviewFixtures.session.id)

        XCTAssertTrue(viewModel.sessions.first(where: { $0.id == PreviewFixtures.session.id })?.isPinned == false)
        let stored = await pinnedStore.pinnedSessionIDs(for: PreviewFixtures.project.id)
        XCTAssertTrue(stored.isEmpty)
    }

    func testInspectActivityRoutesInspectorToRelevantPanel() async {
        let timelineEntry = TimelineEntry(title: "Completed", detail: "Mock task completed", level: .success)
        let cliEvent = CLIEvent(taskID: PreviewFixtures.task.id, stream: .stderr, text: "fatal: failed")
        let task = AgentTask(
            id: PreviewFixtures.task.id,
            sessionID: PreviewFixtures.task.sessionID,
            title: PreviewFixtures.task.title,
            prompt: PreviewFixtures.task.prompt,
            summary: PreviewFixtures.task.summary,
            status: PreviewFixtures.task.status,
            createdAt: PreviewFixtures.task.createdAt,
            startedAt: PreviewFixtures.task.startedAt,
            finishedAt: PreviewFixtures.task.finishedAt,
            responseText: PreviewFixtures.task.responseText,
            errorText: PreviewFixtures.task.errorText,
            retryCount: PreviewFixtures.task.retryCount,
            commands: PreviewFixtures.task.commands,
            timeline: [timelineEntry],
            cliEvents: [cliEvent],
            fileChanges: PreviewFixtures.task.fileChanges,
            metadata: PreviewFixtures.task.metadata,
            exitCode: PreviewFixtures.task.exitCode
        )
        let commandMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .log,
            body: "```log\n$ \(PreviewFixtures.task.commands[0].displayCommand)\n```",
            relatedTaskID: PreviewFixtures.task.id
        )
        let statusMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .status,
            body: "```status\n\(timelineEntry.title)\n\(timelineEntry.detail)\n```",
            relatedTaskID: PreviewFixtures.task.id
        )
        let logMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .log,
            body: "```log\n[stderr] \(cliEvent.text)\n```",
            relatedTaskID: PreviewFixtures.task.id
        )
        let activityMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .activity,
            body: ActivityMessagePayload(
                phase: .toolResult,
                name: "write_file",
                summary: "Wrote file",
                fields: [ActivityMessageField(
                    label: "Path",
                    value: "/tmp/workspace/\(PreviewFixtures.task.fileChanges[0].path)"
                )]
            ).encodedString(),
            relatedTaskID: PreviewFixtures.task.id
        )
        let archive = ProjectArchive(
            project: PreviewFixtures.project,
            sessions: [PreviewFixtures.session],
            messages: PreviewFixtures.projectArchive.messages + [commandMessage, statusMessage, logMessage, activityMessage],
            tasks: [task],
            selectedSessionID: PreviewFixtures.session.id
        )
        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: archive,
            runtime: MockWorkbenchRuntime(),
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        let commandPayload = ActivityMessagePayload(
            phase: .toolResult,
            name: "run_cmd",
            summary: "Tests failed",
            fields: [ActivityMessageField(label: "Command", value: "swift test")]
        )
        viewModel.inspectActivity(commandPayload, taskID: PreviewFixtures.task.id)

        XCTAssertTrue(viewModel.isInspectorVisible)
        XCTAssertEqual(viewModel.inspectorPanel, InspectorPanel.logs)
        XCTAssertEqual(viewModel.selectedTask?.id, PreviewFixtures.task.id)

        let filePayload = ActivityMessagePayload(
            phase: .toolResult,
            name: "write_file",
            summary: "Wrote file",
            fields: [ActivityMessageField(
                label: "Path",
                value: "/tmp/workspace/\(PreviewFixtures.task.fileChanges[0].path)"
            )]
        )
        viewModel.inspectActivity(filePayload, taskID: PreviewFixtures.task.id)

        XCTAssertEqual(viewModel.inspectorPanel, InspectorPanel.changes)
        XCTAssertEqual(viewModel.selectedFileChangeID, PreviewFixtures.task.fileChanges.first?.id)

        viewModel.inspectCommand(PreviewFixtures.task.commands[0].id)
        XCTAssertEqual(viewModel.inspectorPanel, InspectorPanel.commands)
        XCTAssertEqual(viewModel.selectedCommandID, PreviewFixtures.task.commands[0].id)
        XCTAssertEqual(viewModel.highlightedMessageID, commandMessage.id)

        viewModel.inspectFileChange(PreviewFixtures.task.fileChanges[0].id)
        XCTAssertEqual(viewModel.inspectorPanel, InspectorPanel.changes)
        XCTAssertEqual(viewModel.highlightedMessageID, activityMessage.id)

        viewModel.inspectTimelineEntry(timelineEntry.id)
        XCTAssertEqual(viewModel.inspectorPanel, InspectorPanel.timeline)
        XCTAssertEqual(viewModel.selectedTimelineEntryID, timelineEntry.id)
        XCTAssertEqual(viewModel.highlightedMessageID, statusMessage.id)

        viewModel.inspectCLIEvent(cliEvent.id)
        XCTAssertEqual(viewModel.inspectorPanel, InspectorPanel.logs)
        XCTAssertEqual(viewModel.selectedCLIEventID, cliEvent.id)
        XCTAssertEqual(viewModel.highlightedMessageID, logMessage.id)
    }

    func testInspectMessageRoutesInspectorFromThreadMessages() async {
        let timelineEntry = TimelineEntry(title: "Completed", detail: "Mock task completed", level: .success)
        let task = AgentTask(
            id: PreviewFixtures.task.id,
            sessionID: PreviewFixtures.task.sessionID,
            title: PreviewFixtures.task.title,
            prompt: PreviewFixtures.task.prompt,
            summary: PreviewFixtures.task.summary,
            status: PreviewFixtures.task.status,
            createdAt: PreviewFixtures.task.createdAt,
            startedAt: PreviewFixtures.task.startedAt,
            finishedAt: PreviewFixtures.task.finishedAt,
            responseText: PreviewFixtures.task.responseText,
            errorText: PreviewFixtures.task.errorText,
            retryCount: PreviewFixtures.task.retryCount,
            commands: PreviewFixtures.task.commands,
            timeline: [timelineEntry],
            cliEvents: PreviewFixtures.task.cliEvents,
            fileChanges: PreviewFixtures.task.fileChanges,
            metadata: PreviewFixtures.task.metadata,
            exitCode: PreviewFixtures.task.exitCode
        )
        let taskMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .task,
            body: task.title,
            relatedTaskID: task.id
        )
        let statusMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .status,
            body: "```status\n\(timelineEntry.title)\n\(timelineEntry.detail)\n```",
            relatedTaskID: task.id
        )
        let logMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .log,
            body: "```log\n$ \(PreviewFixtures.task.commands[0].displayCommand)\n```",
            relatedTaskID: task.id
        )
        let activityMessage = Message(
            sessionID: PreviewFixtures.session.id,
            role: .system,
            kind: .activity,
            body: ActivityMessagePayload(
                phase: .toolResult,
                name: "write_file",
                summary: "Wrote file",
                fields: [ActivityMessageField(
                    label: "Path",
                    value: "/tmp/workspace/\(PreviewFixtures.task.fileChanges[0].path)"
                )]
            ).encodedString(),
            relatedTaskID: task.id
        )
        let archive = ProjectArchive(
            project: PreviewFixtures.project,
            sessions: [PreviewFixtures.session],
            messages: PreviewFixtures.projectArchive.messages + [taskMessage, statusMessage, logMessage, activityMessage],
            tasks: [task],
            selectedSessionID: PreviewFixtures.session.id
        )
        let viewModel = WorkbenchViewModel(
            project: PreviewFixtures.project,
            archive: archive,
            runtime: MockWorkbenchRuntime(),
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        ) { _ in }

        viewModel.inspectMessage(taskMessage.id)
        XCTAssertEqual(viewModel.selectedMessageID, taskMessage.id)
        XCTAssertEqual(viewModel.selectedTask?.id, task.id)
        XCTAssertEqual(viewModel.inspectorPanel, .overview)
        XCTAssertNil(viewModel.selectedTimelineEntryID)
        XCTAssertNil(viewModel.selectedCLIEventID)
        XCTAssertNil(viewModel.selectedCommandID)

        viewModel.inspectMessage(statusMessage.id)
        XCTAssertEqual(viewModel.selectedMessageID, statusMessage.id)
        XCTAssertEqual(viewModel.inspectorPanel, .timeline)
        XCTAssertEqual(viewModel.selectedTimelineEntryID, timelineEntry.id)

        viewModel.inspectMessage(logMessage.id)
        XCTAssertEqual(viewModel.selectedMessageID, logMessage.id)
        XCTAssertEqual(viewModel.inspectorPanel, .commands)
        XCTAssertEqual(viewModel.selectedCommandID, PreviewFixtures.task.commands[0].id)

        viewModel.inspectMessage(activityMessage.id)
        XCTAssertEqual(viewModel.selectedMessageID, activityMessage.id)
        XCTAssertEqual(viewModel.inspectorPanel, .changes)
        XCTAssertEqual(viewModel.selectedFileChangeID, task.fileChanges.first?.id)
    }
}
