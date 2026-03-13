import AppKit
import Domain
import Features
import SwiftUI

struct ConversationView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var consoleDragStartHeight: CGFloat?

    private var messageTailSignature: String {
        guard let last = viewModel.selectedMessages.last else { return "empty" }
        return "\(last.id.uuidString)|\(last.body)"
    }

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
                .padding(.top, isEmptyThread ? 0 : 12)
                .padding(.bottom, 18)

            if appViewModel.isConsoleDrawerVisible {
                ConsoleDrawerResizeHandle()
                    .gesture(consoleResizeGesture)
                ConsoleDrawer(appViewModel: appViewModel, viewModel: viewModel)
                    .frame(height: appViewModel.consoleHeight)
            }
        }
        .background(tokens.windowBackground)
    }

    private func messageFeed(tokens: ThemeTokens) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let session = viewModel.selectedSession {
                        ThreadSummaryRibbon(session: session, viewModel: viewModel)
                    }

                    ForEach(viewModel.selectedMessages) { message in
                        MessageRow(viewModel: viewModel, message: message, settings: appViewModel.settings)
                            .id(message.id)
                    }
                }
                .frame(maxWidth: WorkbenchChromeMetrics.threadContentWidth, alignment: .leading)
                .padding(.horizontal, 26)
                .padding(.top, 18)
                .padding(.bottom, 36)
                .frame(maxWidth: .infinity, alignment: .center)
            }
            .onChange(of: messageTailSignature) {
                if let lastID = viewModel.selectedMessages.last?.id {
                    proxy.scrollTo(lastID, anchor: .bottom)
                }
            }
            .onChange(of: viewModel.messageRevealToken) {
                if let highlightedMessageID = viewModel.highlightedMessageID {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        proxy.scrollTo(highlightedMessageID, anchor: .center)
                    }
                }
            }
        }
    }

    private var consoleResizeGesture: some Gesture {
        DragGesture(minimumDistance: 2)
            .onChanged { value in
                let startHeight = consoleDragStartHeight ?? appViewModel.consoleHeight
                if consoleDragStartHeight == nil {
                    consoleDragStartHeight = startHeight
                }
                appViewModel.setConsoleHeight(startHeight - value.translation.height)
            }
            .onEnded { value in
                let startHeight = consoleDragStartHeight ?? appViewModel.consoleHeight
                appViewModel.setConsoleHeight(startHeight - value.translation.height, persist: true)
                consoleDragStartHeight = nil
            }
    }
}

private struct EmptyThreadHero: View {
    let projectName: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 14) {
            Image(systemName: "sparkles")
                .font(.system(size: 24, weight: .medium))
                .foregroundStyle(tokens.primaryText)
                .frame(width: 48, height: 48)
                .background(tokens.elevatedBackground, in: Circle())
                .overlay(
                    Circle()
                        .stroke(tokens.border, lineWidth: 1)
                )

            VStack(spacing: 2) {
                Text("Let's build")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                HStack(spacing: 4) {
                    Text(projectName)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 12, weight: .semibold))
                }
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(tokens.secondaryText)
            }
        }
        .frame(maxHeight: .infinity, alignment: .center)
        .padding(.top, 8)
    }
}

private struct ThreadSummaryRibbon: View {
    let session: Session
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 10) {
            if !session.summary.isEmpty {
                Text(session.summary)
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .lineLimit(2)
            }

            HStack(spacing: 8) {
                MetricChip(title: "Provider", value: viewModel.currentProviderLabel, accent: tokens.accent)
                MetricChip(title: "Model", value: viewModel.currentModelLabel, accent: tokens.success)
                if let selectedTask = viewModel.selectedTask {
                    StatusChip(title: selectedTask.status.rawValue.uppercased(), state: selectedTask.status)
                }
                Spacer()
                Text(session.updatedAt.formatted(date: .abbreviated, time: .shortened))
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }

            Divider()
        }
        .padding(.bottom, 2)
    }
}

private struct MessageRow: View {
    @Bindable var viewModel: WorkbenchViewModel
    let message: Message
    let settings: AppSettings
    @Environment(\.colorScheme) private var colorScheme

    private var usesCard: Bool {
        message.role == .user || message.role == .system || message.kind != .markdown
    }

    private var showsMetaHeader: Bool {
        message.kind != .activity && (message.role != .assistant || message.kind != .markdown)
    }

    private var canInspect: Bool {
        message.relatedTaskID != nil
    }

    private var usesDedicatedTapHandler: Bool {
        message.kind == .activity || message.kind == .task
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let isSelected = viewModel.selectedMessageID == message.id
        let isHighlighted = viewModel.highlightedMessageID == message.id

        VStack(alignment: .leading, spacing: usesCard ? 10 : 8) {
            if showsMetaHeader {
                HStack {
                    Text(message.role.title.uppercased())
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Text(message.createdAt.formatted(date: .omitted, time: .shortened))
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                }
            }

            if message.kind == .task,
               let taskID = message.relatedTaskID,
               let task = viewModel.recentTasks.first(where: { $0.id == taskID }) {
                TaskMessageCard(viewModel: viewModel, messageID: message.id, task: task)
            } else if message.kind == .activity,
                      let payload = ActivityMessagePayload.decode(from: message.body) {
                ActivityMessageCard(
                    viewModel: viewModel,
                    messageID: message.id,
                    payload: payload,
                    taskID: message.relatedTaskID,
                    timestamp: message.createdAt
                )
            } else {
                MarkdownRenderer(text: message.body, settings: settings)
            }
        }
        .padding(usesCard ? 13 : 0)
        .frame(maxWidth: maxWidth, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            background(tokens: tokens, isSelected: isSelected, isHighlighted: isHighlighted),
            in: RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .stroke(
                    isHighlighted ? tokens.accent.opacity(0.55) : border(tokens: tokens, isSelected: isSelected),
                    lineWidth: isHighlighted ? 1.5 : (isSelected ? 1.25 : 1)
                )
        )
        .shadow(color: shadow(tokens: tokens, isSelected: isSelected, isHighlighted: isHighlighted), radius: 10, x: 0, y: 2)
        .contentShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
        .onTapGesture {
            guard canInspect, !usesDedicatedTapHandler else { return }
            viewModel.inspectMessage(message.id)
        }
        .animation(.easeInOut(duration: 0.18), value: isHighlighted)
        .animation(.easeInOut(duration: 0.18), value: isSelected)
    }

    private var maxWidth: CGFloat {
        switch message.role {
        case .assistant:
            return WorkbenchChromeMetrics.assistantMessageWidth
        case .user:
            return WorkbenchChromeMetrics.userMessageWidth
        case .system:
            return WorkbenchChromeMetrics.systemMessageWidth
        }
    }

    private var cornerRadius: CGFloat {
        message.role == .user ? 16 : 14
    }

    private func background(tokens: ThemeTokens, isSelected: Bool, isHighlighted: Bool) -> Color {
        if isHighlighted {
            return tokens.accent.opacity(usesCard ? 0.08 : 0.05)
        }
        if isSelected {
            return tokens.accent.opacity(usesCard ? 0.05 : 0.035)
        }
        switch message.role {
        case .user:
            return tokens.panelBackground
        case .assistant:
            return usesCard ? tokens.panelBackground : .clear
        case .system:
            return message.kind == .error ? tokens.failure.opacity(0.08) : tokens.panelBackground
        }
    }

    private func border(tokens: ThemeTokens, isSelected: Bool) -> Color {
        if isSelected {
            return tokens.accent.opacity(0.3)
        }
        switch message.kind {
        case .error:
            return tokens.failure.opacity(0.35)
        case .status:
            return tokens.accent.opacity(0.16)
        default:
            return usesCard ? tokens.border : .clear
        }
    }

    private func shadow(tokens: ThemeTokens, isSelected: Bool, isHighlighted: Bool) -> Color {
        if isHighlighted {
            return tokens.accent.opacity(0.16)
        }
        if isSelected {
            return tokens.accent.opacity(0.08)
        }
        return .clear
    }
}

private struct ActivityMessageCard: View {
    @Bindable var viewModel: WorkbenchViewModel
    let messageID: UUID
    let payload: ActivityMessagePayload
    let taskID: UUID?
    let timestamp: Date
    @Environment(\.colorScheme) private var colorScheme
    @State private var isExpanded = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let accent = accentColor(tokens: tokens)

        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: iconName)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(accent)
                    .frame(width: 28, height: 28)
                    .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 9, style: .continuous))

                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Text(payload.phase.title.uppercased())
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                        Text(timestamp.formatted(date: .omitted, time: .shortened))
                            .font(.system(size: 10, weight: .medium, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                    }
                    Text(payload.name)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(payload.summary)
                        .font(.system(size: 12.5))
                        .foregroundStyle(summaryColor(tokens: tokens))
                        .lineLimit(isExpanded ? nil : 2)
                }
                Spacer()
            }

            if let previewText = payload.previewText, !previewText.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text((payload.previewTitle ?? "Output").uppercased())
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Text(previewText)
                        .font(.system(size: 11.5, design: .monospaced))
                        .foregroundStyle(previewForeground(tokens: tokens))
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(previewBackground(tokens: tokens), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(previewBorder(tokens: tokens), lineWidth: 1)
                )
            }

            if !payload.fields.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(payload.fields, id: \.self) { field in
                        HStack(alignment: .top, spacing: 10) {
                            Text(field.label.uppercased())
                                .font(.system(size: 9, weight: .semibold, design: .monospaced))
                                .foregroundStyle(tokens.tertiaryText)
                                .frame(width: 70, alignment: .leading)
                            Text(field.value)
                                .font(.system(size: 11.5, design: field.label == "Command" ? .monospaced : .default))
                                .foregroundStyle(tokens.primaryText)
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            }

            if let detail = payload.detail, !detail.isEmpty {
                Button {
                    isExpanded.toggle()
                } label: {
                    HStack(spacing: 6) {
                        Text(isExpanded ? "Hide payload" : "Show payload")
                        Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                            .font(.system(size: 10, weight: .semibold))
                    }
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(tokens.accent)
                }
                .buttonStyle(.plain)

                if isExpanded {
                    Text(detail)
                        .font(.system(size: 11.5, design: .monospaced))
                        .foregroundStyle(tokens.secondaryText)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .background(tokens.logBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }
            }

            if taskID != nil {
                HStack {
                    Spacer()
                    Button(actionLabel) {
                        viewModel.inspectMessage(messageID)
                    }
                    .buttonStyle(.borderless)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(tokens.accent)
                }
            }
        }
        .contentShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onTapGesture {
            guard taskID != nil else { return }
            viewModel.inspectMessage(messageID)
        }
    }

    private var iconName: String {
        switch payload.name {
        case "read_file":
            return payload.phase == .toolCall ? "doc.text.magnifyingglass" : "doc.text"
        case "write_file", "edit_file":
            return payload.phase == .toolCall ? "square.and.pencil" : "checkmark.square"
        case "search_text":
            return "magnifyingglass"
        case "run_cmd":
            return payload.phase == .toolCall ? "terminal" : "checkmark.circle"
        default:
            return payload.phase == .toolCall ? "bolt.horizontal.circle" : "checkmark.circle"
        }
    }

    private func accentColor(tokens: ThemeTokens) -> Color {
        switch payload.previewKind {
        case .failure:
            return tokens.failure
        case .warning:
            return tokens.warning
        case .success:
            return tokens.success
        default:
            return payload.phase == .toolCall ? tokens.warning : tokens.success
        }
    }

    private func summaryColor(tokens: ThemeTokens) -> Color {
        switch payload.previewKind {
        case .failure:
            return tokens.failure
        case .warning:
            return tokens.warning
        default:
            return tokens.secondaryText
        }
    }

    private func previewBackground(tokens: ThemeTokens) -> Color {
        switch payload.previewKind {
        case .failure:
            return tokens.failure.opacity(0.08)
        case .warning:
            return tokens.warning.opacity(0.1)
        case .success:
            return tokens.logBackground
        default:
            return tokens.elevatedBackground
        }
    }

    private func previewBorder(tokens: ThemeTokens) -> Color {
        switch payload.previewKind {
        case .failure:
            return tokens.failure.opacity(0.24)
        case .warning:
            return tokens.warning.opacity(0.24)
        case .success:
            return tokens.border
        default:
            return tokens.border
        }
    }

    private func previewForeground(tokens: ThemeTokens) -> Color {
        switch payload.previewKind {
        case .failure:
            return tokens.failure
        default:
            return tokens.secondaryText
        }
    }

    private var actionLabel: String {
        switch payload.name {
        case "run_cmd":
            return payload.phase == .toolCall ? "Show Command" : "Show Logs"
        case "write_file", "edit_file":
            return "Show Diff"
        case "read_file", "search_text":
            return "Show Timeline"
        default:
            return "Inspect"
        }
    }
}

private struct TaskMessageCard: View {
    @Bindable var viewModel: WorkbenchViewModel
    let messageID: UUID
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
                    viewModel.inspectMessage(messageID)
                }
                .buttonStyle(.borderless)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(tokens.accent)
            }
        }
        .contentShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onTapGesture {
            viewModel.inspectMessage(messageID)
        }
    }
}

private struct ConsoleDrawer: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let logs = Array((viewModel.selectedTask?.cliEvents ?? []).suffix(10))
        let shellName = URL(fileURLWithPath: ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh").lastPathComponent

        VStack(alignment: .leading, spacing: 0) {
            HStack {
                HStack(spacing: 8) {
                    Text("Terminal")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(shellName)
                        .font(.system(size: 11))
                        .foregroundStyle(tokens.tertiaryText)
                }
                Spacer()
                if let command = viewModel.selectedTask?.commands.last {
                    TerminalActionButton(title: "Copy") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(command.displayCommand, forType: .string)
                    }
                }
                TerminalActionButton(systemImage: "xmark") {
                    appViewModel.toggleConsoleDrawer()
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 9)
            .padding(.bottom, 8)

            Divider()
                .padding(.bottom, 8)

            VStack(alignment: .leading, spacing: 9) {
                if let command = viewModel.selectedTask?.commands.last {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("LAST COMMAND")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                        Text("$ \(command.displayCommand)")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(tokens.primaryText)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }

                if logs.isEmpty {
                    Text("No CLI output yet.")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(tokens.secondaryText)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .background(tokens.logBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                } else {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(logs) { event in
                                VStack(alignment: .leading, spacing: 6) {
                                    HStack {
                                        Text(event.stream.rawValue.uppercased())
                                            .font(.system(size: 9, weight: .bold, design: .monospaced))
                                            .foregroundStyle(streamColor(event, tokens: tokens))
                                            .padding(.horizontal, 6)
                                            .padding(.vertical, 3)
                                            .background(tokens.elevatedBackground, in: Capsule())
                                        Spacer()
                                        Text(event.timestamp.formatted(date: .omitted, time: .standard))
                                            .font(.system(size: 9, weight: .medium, design: .monospaced))
                                            .foregroundStyle(tokens.tertiaryText)
                                    }

                                    Text(event.text)
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundStyle(streamColor(event, tokens: tokens))
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                        .textSelection(.enabled)
                                }
                                .padding(.horizontal, 10)
                                .padding(.vertical, 8)
                                .background(tokens.logBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 12)

            Divider()

            HStack(spacing: 10) {
                Text("\(shellPromptPrefix) $")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.secondaryText)
                Spacer()
                Text(viewModel.selectedSession?.state.rawValue.capitalized ?? "Idle")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 9)
            .background(tokens.elevatedBackground.opacity(0.78))
        }
        .background(tokens.panelBackground)
    }

    private var shellPromptPrefix: String {
        let project = viewModel.project.name
        let state = viewModel.selectedSession?.state.rawValue ?? "idle"
        return "Mac:\(project) (\(state))"
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
            return tokens.secondaryText
        }
    }
}

private struct TerminalActionButton: View {
    let title: String?
    let systemImage: String?
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    init(title: String, action: @escaping () -> Void) {
        self.title = title
        self.systemImage = nil
        self.action = action
    }

    init(systemImage: String, action: @escaping () -> Void) {
        self.title = nil
        self.systemImage = systemImage
        self.action = action
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            Group {
                if let title {
                    Text(title)
                        .font(.system(size: 10, weight: .semibold))
                } else if let systemImage {
                    Image(systemName: systemImage)
                        .font(.system(size: 10, weight: .semibold))
                }
            }
            .foregroundStyle(tokens.secondaryText)
            .padding(.horizontal, title == nil ? 7 : 8)
            .padding(.vertical, 5)
            .background((isHovered ? tokens.panelBackground : tokens.elevatedBackground), in: Capsule())
            .overlay(
                Capsule()
                    .stroke(tokens.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct ConsoleDrawerResizeHandle: View {
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovering = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Rectangle()
            .fill(isHovering ? tokens.accent.opacity(0.18) : Color.clear)
            .frame(height: 7)
            .overlay(
                Capsule()
                    .fill(isHovering ? tokens.accent.opacity(0.6) : tokens.border)
                    .frame(width: 74, height: 4)
            )
            .contentShape(Rectangle())
            .onHover { isHovering = $0 }
            .help("Drag to resize terminal")
    }
}
