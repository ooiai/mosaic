import Domain
import Features
import SwiftUI

struct InspectorView: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 14) {
            Picker("Inspector", selection: $viewModel.inspectorPanel) {
                ForEach(InspectorPanel.allCases) { panel in
                    Text(panel.title).tag(panel)
                }
            }
            .pickerStyle(.segmented)

            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
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
        }
        .padding(14)
        .background(tokens.sidebarBackground)
    }

    @ViewBuilder
    private func overviewContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            PanelCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("Task Overview")
                    HStack {
                        Text(task.title)
                            .font(.system(size: 15, weight: .semibold))
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

        PanelCard {
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
            PanelCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("Timeline", trailing: "\(task.timeline.count)")
                    ForEach(task.timeline) { entry in
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
                        Divider()
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func logsContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            PanelCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("CLI Logs", trailing: "\(task.cliEvents.count)")
                    if task.cliEvents.isEmpty {
                        Text("No CLI events captured.")
                            .font(.system(size: 12))
                            .foregroundStyle(tokens.secondaryText)
                    } else {
                        ForEach(task.cliEvents) { event in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(event.stream.rawValue.uppercased())
                                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(tokens.tertiaryText)
                                Text(event.text)
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(tokens.primaryText)
                                    .textSelection(.enabled)
                            }
                            Divider()
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func commandsContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            PanelCard {
                VStack(alignment: .leading, spacing: 10) {
                    SectionHeader("Commands", trailing: "\(task.commands.count)")
                    ForEach(task.commands) { command in
                        VStack(alignment: .leading, spacing: 4) {
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
                        Divider()
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func changesContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            if !task.fileChanges.isEmpty {
                PanelCard {
                    VStack(alignment: .leading, spacing: 10) {
                        SectionHeader("Files Changed", trailing: "\(task.fileChanges.count)")
                        ForEach(task.fileChanges) { change in
                            Button {
                                viewModel.selectFileChange(change.id)
                            } label: {
                                HStack {
                                    VStack(alignment: .leading, spacing: 3) {
                                        Text(change.path)
                                            .font(.system(size: 12, weight: .semibold))
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
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }

            if let diff = viewModel.selectedFileChange?.diff, !diff.isEmpty {
                PanelCard {
                    VStack(alignment: .leading, spacing: 10) {
                        SectionHeader("Diff Preview")
                        MarkdownRenderer(
                            text: "```diff\n\(diff)\n```",
                            settings: AppSettings()
                        )
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func metadataContent(tokens: ThemeTokens) -> some View {
        if let task = viewModel.selectedTask {
            PanelCard {
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
        VStack(alignment: .leading, spacing: 2) {
            Text(key.uppercased())
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            Text(value)
                .font(.system(size: 12))
                .foregroundStyle(tokens.primaryText)
                .textSelection(.enabled)
        }
    }
}
