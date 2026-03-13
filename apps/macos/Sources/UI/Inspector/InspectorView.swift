import Domain
import Features
import SwiftUI

struct InspectorView: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    private var selectionSignature: String {
        [
            viewModel.inspectorPanel.rawValue,
            viewModel.inspectorSelectionAnchorID ?? "none",
        ].joined(separator: ":")
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 10) {
            if let selectedTask = viewModel.selectedTask {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Inspector")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                        Text(selectedTask.title)
                            .font(.system(size: 11))
                            .foregroundStyle(tokens.secondaryText)
                            .lineLimit(2)
                    }
                    Spacer()
                    StatusChip(title: selectedTask.status.rawValue.uppercased(), state: selectedTask.status)
                }
            } else {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Inspector")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text("Select a task to inspect timeline, logs, commands, and changed files.")
                        .font(.system(size: 11))
                        .foregroundStyle(tokens.secondaryText)
                }
            }

            Picker("Inspector", selection: $viewModel.inspectorPanel) {
                ForEach(InspectorPanel.allCases) { panel in
                    Text(panel.title).tag(panel)
                }
            }
            .pickerStyle(.segmented)
            .controlSize(.small)

            if viewModel.selectedTask == nil {
                ContentUnavailableView(
                    "No Task Selected",
                    systemImage: "sidebar.right",
                    description: Text("Run or select a task from the thread list to populate the inspector.")
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 10) {
                            switch viewModel.inspectorPanel {
                            case .overview:
                                overviewContent(tokens: tokens)
                            case .timeline:
                                timelineContent(tokens: tokens)
                            case .logs:
                                logsContent(tokens: tokens)
                            case .commands:
                                commandsContent(tokens: tokens)
                            case .changes:
                                changesContent(tokens: tokens)
                            case .metadata:
                                metadataContent(tokens: tokens)
                            }
                        }
                        .padding(.bottom, 18)
                    }
                    .onChange(of: selectionSignature) {
                        guard let anchorID = viewModel.inspectorSelectionAnchorID else { return }
                        withAnimation(.easeInOut(duration: 0.2)) {
                            proxy.scrollTo(anchorID, anchor: .center)
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.top, 12)
        .padding(.bottom, 10)
        .background(tokens.sidebarBackground)
    }

    @ViewBuilder
    private func overviewContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            InspectorSectionCard {
                VStack(alignment: .leading, spacing: 12) {
                    SectionHeader("Task Overview")
                    HStack {
                        Text(task.title)
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                        Spacer()
                        StatusChip(title: task.status.rawValue.uppercased(), state: task.status)
                    }
                    Text(task.summary.isEmpty ? task.prompt : task.summary)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                    HStack(spacing: 8) {
                        MetricChip(title: "Events", value: "\(task.cliEvents.count)", accent: tokens.accent)
                        MetricChip(title: "Files", value: "\(task.fileChanges.count)", accent: tokens.success)
                        MetricChip(title: "Exit", value: task.exitCode.map(String.init) ?? "—", accent: tokens.warning)
                    }
                }
            }
        }

        InspectorSectionCard {
            VStack(alignment: .leading, spacing: 10) {
                SectionHeader("Runtime")
                keyValueRow("Workspace", viewModel.project.workspacePath, tokens: tokens)
                keyValueRow("Profile", viewModel.selectedProfile, tokens: tokens)
                keyValueRow("Provider", viewModel.currentProviderLabel, tokens: tokens)
                keyValueRow("Model", viewModel.currentModelLabel, tokens: tokens)
                keyValueRow("Health", viewModel.currentHealthLabel, tokens: tokens)
            }
        }
    }

    @ViewBuilder
    private func timelineContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            InspectorSectionCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("Timeline", trailing: "\(task.timeline.count)")
                    ForEach(task.timeline) { entry in
                        Button {
                            viewModel.inspectTimelineEntry(entry.id)
                        } label: {
                            HStack(alignment: .top, spacing: 10) {
                                Circle()
                                    .fill(color(for: entry.level, tokens: tokens))
                                    .frame(width: 7, height: 7)
                                    .padding(.top, 5)
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(entry.title)
                                        .font(.system(size: 12, weight: .semibold))
                                        .foregroundStyle(tokens.primaryText)
                                    Text(entry.detail)
                                        .font(.system(size: 11))
                                        .foregroundStyle(tokens.secondaryText)
                                    Text(entry.timestamp.formatted(date: .omitted, time: .standard))
                                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                                        .foregroundStyle(tokens.tertiaryText)
                                }
                            }
                            .padding(9)
                            .background(selectionBackground(isSelected: viewModel.selectedTimelineEntryID == entry.id, tokens: tokens), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                            .overlay(
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .stroke(selectionBorder(isSelected: viewModel.selectedTimelineEntryID == entry.id, tokens: tokens), lineWidth: 1)
                            )
                        }
                        .id("timeline-\(entry.id.uuidString)")
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func logsContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            InspectorSectionCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("CLI Logs", trailing: "\(task.cliEvents.count)")
                    if task.cliEvents.isEmpty {
                        Text("No CLI events captured.")
                            .font(.system(size: 12))
                            .foregroundStyle(tokens.secondaryText)
                    } else {
                        ForEach(task.cliEvents) { event in
                            Button {
                                viewModel.inspectCLIEvent(event.id)
                            } label: {
                                VStack(alignment: .leading, spacing: 7) {
                                    HStack {
                                    Text(event.stream.rawValue.uppercased())
                                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                        .foregroundStyle(streamColor(event, tokens: tokens))
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 3)
                                        .background(tokens.elevatedBackground, in: Capsule())
                                    Spacer()
                                    if event.isImportant {
                                        Text("IMPORTANT")
                                            .font(.system(size: 9, weight: .bold, design: .monospaced))
                                            .foregroundStyle(tokens.warning)
                                    }
                                }
                                Text(event.text)
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(streamColor(event, tokens: tokens))
                                    .textSelection(.enabled)
                                }
                                .padding(9)
                                .background(selectionBackground(isSelected: viewModel.selectedCLIEventID == event.id, tokens: tokens), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                                        .stroke(selectionBorder(isSelected: viewModel.selectedCLIEventID == event.id, tokens: tokens), lineWidth: 1)
                                )
                            }
                            .id("log-\(event.id.uuidString)")
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func commandsContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            InspectorSectionCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("Commands", trailing: "\(task.commands.count)")
                    ForEach(task.commands) { command in
                        Button {
                            viewModel.inspectCommand(command.id)
                        } label: {
                            VStack(alignment: .leading, spacing: 6) {
                                Text(command.displayCommand)
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(tokens.primaryText)
                                    .textSelection(.enabled)
                                Text(command.workingDirectory)
                                    .font(.system(size: 10))
                                    .foregroundStyle(tokens.secondaryText)
                                HStack(spacing: 8) {
                                    StatusChip(title: command.status.rawValue.uppercased(), state: command.status)
                                    Text(command.exitCode.map { "exit \($0)" } ?? "running")
                                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                        .foregroundStyle(tokens.tertiaryText)
                                }
                            }
                            .padding(9)
                            .background(selectionBackground(isSelected: viewModel.selectedCommandID == command.id, tokens: tokens), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                            .overlay(
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .stroke(selectionBorder(isSelected: viewModel.selectedCommandID == command.id, tokens: tokens), lineWidth: 1)
                            )
                        }
                        .id("command-\(command.id.uuidString)")
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func changesContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            if !task.fileChanges.isEmpty {
                InspectorSectionCard {
                    VStack(alignment: .leading, spacing: 10) {
                        SectionHeader("Files Changed", trailing: "\(task.fileChanges.count)")
                        ForEach(task.fileChanges) { change in
                            Button {
                                viewModel.inspectFileChange(change.id)
                            } label: {
                                HStack {
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(change.path)
                                            .font(.system(size: 11, weight: .semibold))
                                            .foregroundStyle(tokens.primaryText)
                                            .lineLimit(1)
                                        Text("\(change.additions)+ / \(change.deletions)-")
                                            .font(.system(size: 10, weight: .medium, design: .monospaced))
                                            .foregroundStyle(tokens.secondaryText)
                                    }
                                    Spacer()
                                    Text(change.status.rawValue.uppercased())
                                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                        .foregroundStyle(tokens.tertiaryText)
                                }
                                .padding(10)
                                .background(background(for: change, tokens: tokens), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                            }
                            .id("change-\(change.id.uuidString)")
                            .buttonStyle(.plain)
                        }
                    }
                }
            }

            if let diff = viewModel.selectedFileChange?.diff, !diff.isEmpty {
                InspectorSectionCard {
                    VStack(alignment: .leading, spacing: 10) {
                        SectionHeader("Diff Preview")
                        if let selectedFileChange = viewModel.selectedFileChange {
                            DiffViewer(change: selectedFileChange)
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func metadataContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            InspectorSectionCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("Metadata", trailing: "\(task.metadata.count)")
                    ForEach(task.metadata) { item in
                        keyValueRow(item.key, item.value, tokens: tokens)
                    }
                }
            }
        }
    }

    private func keyValueRow(_ key: String, _ value: String, tokens: ThemeTokens) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Text(key.uppercased())
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
                .frame(width: 76, alignment: .leading)
            Text(value)
                .font(.system(size: 11))
                .foregroundStyle(tokens.primaryText)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 2)
    }

    private func color(for level: TimelineLevel, tokens: ThemeTokens) -> Color {
        switch level {
        case .info: tokens.accent
        case .success: tokens.success
        case .warning: tokens.warning
        case .failure: tokens.failure
        }
    }

    private func streamColor(_ event: CLIEvent, tokens: ThemeTokens) -> Color {
        switch event.stream {
        case .stderr:
            return tokens.failure
        case .status:
            return tokens.accent
        case .command:
            return tokens.warning
        case .stdout, .system:
            return tokens.primaryText
        }
    }

    private func background(for change: FileChange, tokens: ThemeTokens) -> Color {
        change.id == viewModel.selectedFileChange?.id ? tokens.selection : tokens.elevatedBackground
    }

    private func selectionBackground(isSelected: Bool, tokens: ThemeTokens) -> Color {
        isSelected ? tokens.selection : tokens.elevatedBackground
    }

    private func selectionBorder(isSelected: Bool, tokens: ThemeTokens) -> Color {
        isSelected ? tokens.accent.opacity(0.3) : tokens.border
    }
}

private struct InspectorSectionCard<Content: View>: View {
    let content: Content
    @Environment(\.colorScheme) private var colorScheme

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        content
            .padding(14)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
    }
}

private struct InspectorItemSurface<Content: View>: View {
    let content: Content
    @Environment(\.colorScheme) private var colorScheme

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        content
            .padding(9)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
    }
}
