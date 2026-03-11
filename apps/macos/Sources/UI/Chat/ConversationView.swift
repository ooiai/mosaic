import Domain
import Features
import SwiftUI

struct ConversationView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 0) {
            conversationHeader(tokens: tokens)
                .padding(.horizontal, 24)
                .padding(.top, 16)
                .padding(.bottom, 8)

            Divider()

            if viewModel.selectedSessionID == nil && viewModel.selectedMessages.isEmpty {
                ScrollView {
                    EmptyStateCard(
                        eyebrow: "Command Center",
                        title: "Run the next task from project context.",
                        detail: "Use the composer to inspect code, review changes, or drive a concrete implementation task through `mosaic-cli`.",
                        actionTitle: "Choose Workspace"
                    ) {
                        appViewModel.showSetupHub()
                    }
                    .padding(32)
                }
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 18) {
                            ForEach(viewModel.selectedMessages) { message in
                                MessageRow(viewModel: viewModel, message: message, settings: appViewModel.settings)
                                    .id(message.id)
                            }
                        }
                        .padding(.horizontal, 24)
                        .padding(.vertical, 18)
                    }
                    .onChange(of: viewModel.selectedMessages.count) {
                        if let lastID = viewModel.selectedMessages.last?.id {
                            proxy.scrollTo(lastID, anchor: .bottom)
                        }
                    }
                }
            }

            Divider()

            ComposerDock(appViewModel: appViewModel, viewModel: viewModel)
                .padding(.horizontal, 20)
                .padding(.vertical, 16)
        }
        .background(tokens.windowBackground)
    }

    private func conversationHeader(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                VStack(alignment: .leading, spacing: 5) {
                    Text((viewModel.selectedSession?.title ?? "No Session").uppercased())
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Text(viewModel.selectedSession?.summary.isEmpty == false ? viewModel.selectedSession?.summary ?? "" : "AI agent command center")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                }
                Spacer()
                HStack(spacing: 8) {
                    MetricChip(title: "Provider", value: viewModel.currentProviderLabel, accent: tokens.accent)
                    MetricChip(title: "Model", value: viewModel.currentModelLabel, accent: tokens.success)
                    MetricChip(title: "Health", value: viewModel.currentHealthLabel, accent: tokens.warning)
                }
            }

            if let selectedTask = viewModel.selectedTask {
                HStack(spacing: 10) {
                    StatusChip(title: selectedTask.status.rawValue.uppercased(), state: selectedTask.status)
                    Text(selectedTask.title)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(1)
                }
            }
        }
    }
}

private struct MessageRow: View {
    @Bindable var viewModel: WorkbenchViewModel
    let message: Message
    let settings: AppSettings
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        HStack(alignment: .top, spacing: 12) {
            if message.role == .user { Spacer(minLength: 80) }
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(message.role.title.uppercased())
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Text(message.createdAt.formatted(date: .omitted, time: .shortened))
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                }

                if message.kind == .task, let taskID = message.relatedTaskID, let task = viewModel.recentTasks.first(where: { $0.id == taskID }) {
                    TaskMessageCard(viewModel: viewModel, task: task)
                } else {
                    MarkdownRenderer(text: message.body, settings: settings)
                }
            }
            .padding(14)
            .frame(maxWidth: 760, alignment: .leading)
            .background(background(tokens: tokens), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(border(tokens: tokens), lineWidth: 1)
            )
            if message.role != .user { Spacer(minLength: 80) }
        }
    }

    private func background(tokens: ThemeTokens) -> Color {
        switch message.role {
        case .user:
            return tokens.selection
        case .assistant:
            return tokens.panelBackground
        case .system:
            return tokens.elevatedBackground
        }
    }

    private func border(tokens: ThemeTokens) -> Color {
        switch message.kind {
        case .error:
            return tokens.failure.opacity(0.4)
        default:
            return tokens.border
        }
    }
}

private struct TaskMessageCard: View {
    @Bindable var viewModel: WorkbenchViewModel
    let task: AgentTask
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 10) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(task.title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(task.summary.isEmpty ? task.prompt : task.summary)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(3)
                }
                Spacer()
                StatusChip(title: task.status.rawValue.uppercased(), state: task.status)
            }

            if let latest = task.timeline.last {
                Text("\(latest.title) · \(latest.detail)")
                    .font(.system(size: 11))
                    .foregroundStyle(tokens.secondaryText)
            }

            HStack(spacing: 8) {
                MetricChip(title: "Logs", value: "\(task.cliEvents.count)", accent: tokens.accent)
                MetricChip(title: "Cmds", value: "\(task.commands.count)", accent: tokens.warning)
                MetricChip(title: "Files", value: "\(task.fileChanges.count)", accent: tokens.success)
                Spacer()
                Button("Inspect") {
                    viewModel.selectTask(task.id)
                    viewModel.inspectorPanel = .overview
                }
                .buttonStyle(.borderless)
                .foregroundStyle(tokens.accent)
            }
        }
    }
}
