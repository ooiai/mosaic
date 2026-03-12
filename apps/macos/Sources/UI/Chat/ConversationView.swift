import Domain
import Features
import SwiftUI

struct ConversationView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    private var isEmptyThread: Bool {
        viewModel.selectedSessionID == nil || viewModel.selectedMessages.isEmpty
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 0) {
            Group {
                if isEmptyThread {
                    EmptyThreadHero(projectName: viewModel.project.name)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    messageFeed(tokens: tokens)
                }
            }

            ComposerDock(appViewModel: appViewModel, viewModel: viewModel)
                .padding(.horizontal, 24)
                .padding(.top, isEmptyThread ? 0 : 10)
                .padding(.bottom, 18)

            if appViewModel.isConsoleDrawerVisible {
                Divider()
                ConsoleDrawer(viewModel: viewModel)
                    .frame(height: 170)
            }
        }
        .background(tokens.windowBackground)
    }

    private func messageFeed(tokens: ThemeTokens) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    if let session = viewModel.selectedSession {
                        ThreadSummaryRibbon(session: session, viewModel: viewModel)
                    }

                    ForEach(viewModel.selectedMessages) { message in
                        MessageRow(viewModel: viewModel, message: message, settings: appViewModel.settings)
                            .id(message.id)
                    }
                }
                .frame(maxWidth: 860, alignment: .leading)
                .padding(.horizontal, 32)
                .padding(.top, 28)
                .padding(.bottom, 40)
                .frame(maxWidth: .infinity, alignment: .center)
            }
            .onChange(of: viewModel.selectedMessages.count) {
                if let lastID = viewModel.selectedMessages.last?.id {
                    proxy.scrollTo(lastID, anchor: .bottom)
                }
            }
        }
    }
}

private struct EmptyThreadHero: View {
    let projectName: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 18) {
            Image(systemName: "sparkles")
                .font(.system(size: 26, weight: .medium))
                .foregroundStyle(tokens.primaryText)
                .frame(width: 52, height: 52)
                .background(tokens.panelBackground, in: Circle())
                .overlay(
                    Circle()
                        .stroke(tokens.border, lineWidth: 1)
                )

            VStack(spacing: 4) {
                Text("Let's build")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(projectName)
                    .font(.system(size: 17, weight: .medium))
                    .foregroundStyle(tokens.secondaryText)
            }
        }
        .frame(maxHeight: .infinity, alignment: .center)
        .padding(.top, 30)
    }
}

private struct ThreadSummaryRibbon: View {
    let session: Session
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 8) {
                Text(session.title)
                    .font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(session.summary.isEmpty ? "Agent session" : session.summary)
                    .font(.system(size: 14))
                    .foregroundStyle(tokens.secondaryText)
            }

            Spacer()

            VStack(alignment: .trailing, spacing: 8) {
                HStack(spacing: 8) {
                    MetricChip(title: "Provider", value: viewModel.currentProviderLabel, accent: tokens.accent)
                    MetricChip(title: "Model", value: viewModel.currentModelLabel, accent: tokens.success)
                }

                if let selectedTask = viewModel.selectedTask {
                    StatusChip(title: selectedTask.status.rawValue.uppercased(), state: selectedTask.status)
                }
            }
        }
        .padding(18)
        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}

private struct MessageRow: View {
    @Bindable var viewModel: WorkbenchViewModel
    let message: Message
    let settings: AppSettings
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let usesCard = message.role != .assistant || message.kind != .markdown

        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(message.role.title.uppercased())
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Spacer()
                Text(message.createdAt.formatted(date: .omitted, time: .shortened))
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }

            if message.kind == .task,
               let taskID = message.relatedTaskID,
               let task = viewModel.recentTasks.first(where: { $0.id == taskID }) {
                TaskMessageCard(viewModel: viewModel, task: task)
            } else {
                MarkdownRenderer(text: message.body, settings: settings)
            }
        }
        .padding(usesCard ? 18 : 0)
        .background(
            usesCard ? background(tokens: tokens) : Color.clear,
            in: RoundedRectangle(cornerRadius: 20, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .stroke(usesCard ? border(tokens: tokens) : .clear, lineWidth: 1)
        )
    }

    private func background(tokens: ThemeTokens) -> Color {
        switch message.role {
        case .user:
            return tokens.elevatedBackground
        case .assistant:
            return tokens.panelBackground
        case .system:
            return tokens.panelBackground
        }
    }

    private func border(tokens: ThemeTokens) -> Color {
        switch message.kind {
        case .error:
            return tokens.failure.opacity(0.45)
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

        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 5) {
                    Text(task.title)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(task.summary.isEmpty ? task.prompt : task.summary)
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(3)
                }
                Spacer()
                StatusChip(title: task.status.rawValue.uppercased(), state: task.status)
            }

            if let latest = task.timeline.last {
                Text("\(latest.title) · \(latest.detail)")
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
            }

            HStack(spacing: 8) {
                MetricChip(title: "Logs", value: "\(task.cliEvents.count)", accent: tokens.accent)
                MetricChip(title: "Commands", value: "\(task.commands.count)", accent: tokens.warning)
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

private struct ConsoleDrawer: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let logs = Array((viewModel.selectedTask?.cliEvents ?? []).suffix(10))

        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Console")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text("runtime")
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.tertiaryText)
                Spacer()
            }

            if let command = viewModel.selectedTask?.commands.last {
                Text(command.displayCommand)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(tokens.primaryText)
                    .textSelection(.enabled)
            }

            if logs.isEmpty {
                Text("No CLI output yet.")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(tokens.secondaryText)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(logs) { event in
                            Text(event.text)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(event.stream == .stderr ? tokens.failure : tokens.secondaryText)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .background(tokens.panelBackground)
    }
}
