import Domain
import Features
import SwiftUI

public struct RootContentView: View {
    @Bindable private var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ZStack {
            tokens.windowBackground
                .ignoresSafeArea()

            switch viewModel.screen {
            case .loading:
                ProgressView("Launching Mosaic…")
                    .controlSize(.large)
            case .workspacePicker:
                WorkspacePickerView(viewModel: viewModel)
            case let .onboarding(workspace):
                OnboardingView(viewModel: viewModel, workspace: workspace)
            case .workbench:
                if let workbench = viewModel.workbench {
                    WorkbenchView(viewModel: workbench)
                }
            case let .error(message):
                ContentUnavailableView("Unable to Open Workspace", systemImage: "exclamationmark.triangle", description: Text(message))
            }
        }
        .alert("Runtime Error", isPresented: Binding(
            get: { viewModel.globalError != nil },
            set: { if !$0 { viewModel.dismissError() } }
        )) {
            Button("OK", role: .cancel) {
                viewModel.dismissError()
            }
        } message: {
            Text(viewModel.globalError ?? "")
        }
        .task {
            await viewModel.bootstrap()
        }
    }
}

public struct WorkspacePickerView: View {
    @Bindable private var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 10) {
                Text("Choose a workspace")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text("Mosaic for macOS is project-first. Pick a local folder to anchor sessions, config, and runtime state.")
                    .font(.system(size: 14))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }

            if !viewModel.recentWorkspaces.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Recent Workspaces")
                        .font(.headline)
                        .foregroundStyle(tokens.secondaryText)
                    ForEach(viewModel.recentWorkspaces) { workspace in
                        Button {
                            Task { await viewModel.selectWorkspace(workspace) }
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(workspace.name)
                                        .foregroundStyle(tokens.primaryText)
                                    Text(workspace.path)
                                        .font(.system(size: 12))
                                        .foregroundStyle(tokens.tertiaryText)
                                        .lineLimit(1)
                                }
                                Spacer()
                                Image(systemName: "arrow.up.right.circle")
                                    .foregroundStyle(tokens.accent)
                            }
                            .padding(14)
                            .frame(maxWidth: .infinity)
                            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Button("Choose Folder…") {
                let panel = NSOpenPanel()
                panel.canChooseDirectories = true
                panel.canChooseFiles = false
                panel.allowsMultipleSelection = false
                panel.prompt = "Use Workspace"
                if panel.runModal() == .OK, let url = panel.url {
                    Task { await viewModel.registerWorkspace(url: url) }
                }
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(32)
        .frame(maxWidth: 720)
    }
}

public struct OnboardingView: View {
    @Bindable private var viewModel: AppViewModel
    public let workspace: WorkspaceReference
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel, workspace: WorkspaceReference) {
        self.viewModel = viewModel
        self.workspace = workspace
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 10) {
                Text("Configure Mosaic")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text("This workspace is not configured yet. Create a project-local Mosaic profile before you start chatting.")
                    .foregroundStyle(tokens.secondaryText)
            }

            GroupBox {
                VStack(alignment: .leading, spacing: 14) {
                    LabeledContent("Workspace") {
                        Text(workspace.path).textSelection(.enabled)
                    }
                    TextField("Base URL", text: $viewModel.onboardingBaseURL)
                    TextField("Model", text: $viewModel.onboardingModel)
                    TextField("API Key Env", text: $viewModel.onboardingAPIKeyEnv)
                }
                .textFieldStyle(.roundedBorder)
                .padding(.top, 8)
            }

            Button("Initialize Workspace") {
                Task { await viewModel.completeOnboarding() }
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(32)
        .frame(maxWidth: 720)
    }
}

public struct WorkbenchView: View {
    @Bindable private var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: WorkbenchViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        NavigationSplitView {
            SidebarContent(viewModel: viewModel)
                .navigationSplitViewColumnWidth(min: 260, ideal: 300)
        } content: {
            ConversationContent(viewModel: viewModel)
                .navigationSplitViewColumnWidth(min: 520, ideal: 760)
        } detail: {
            if viewModel.isInspectorVisible {
                InspectorContent(viewModel: viewModel)
                    .navigationSplitViewColumnWidth(min: 260, ideal: 300)
            } else {
                Color.clear
            }
        }
        .background(tokens.windowBackground)
        .toolbar {
            ToolbarItemGroup(placement: .principal) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(viewModel.state.sidebar.currentWorkspace.name)
                        .font(.headline)
                    Text(viewModel.state.sidebar.currentWorkspace.path)
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                        .lineLimit(1)
                }
            }
            ToolbarItemGroup {
                Button {
                    Task { await viewModel.refresh() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }

                Button {
                    viewModel.toggleInspector()
                } label: {
                    Image(systemName: viewModel.isInspectorVisible ? "sidebar.right" : "sidebar.left")
                }
            }
        }
    }
}

struct SidebarContent: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Current Workspace")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(viewModel.state.sidebar.currentWorkspace.name)
                            .font(.headline)
                        Text(viewModel.state.sidebar.currentWorkspace.path)
                            .font(.caption)
                            .foregroundStyle(tokens.secondaryText)
                            .lineLimit(2)
                    }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                }

                if !viewModel.state.sidebar.recentWorkspaces.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Recent Workspaces")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                        ForEach(viewModel.state.sidebar.recentWorkspaces) { workspace in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(workspace.name)
                                    .foregroundStyle(tokens.primaryText)
                                Text(workspace.path)
                                    .font(.caption)
                                    .foregroundStyle(tokens.tertiaryText)
                                    .lineLimit(1)
                            }
                            .padding(.vertical, 4)
                        }
                    }
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Threads")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    Button("New Thread") {
                        viewModel.newThread()
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(tokens.accent)
                    ForEach(viewModel.state.sidebar.threads) { thread in
                        Button {
                            Task { await viewModel.selectThread(thread.id) }
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Text(thread.title)
                                        .foregroundStyle(tokens.primaryText)
                                        .lineLimit(1)
                                    Spacer()
                                    Text(thread.updatedLabel)
                                        .font(.caption2)
                                        .foregroundStyle(tokens.tertiaryText)
                                }
                                Text(thread.subtitle)
                                    .font(.caption)
                                    .foregroundStyle(tokens.secondaryText)
                                    .lineLimit(1)
                            }
                            .padding(12)
                            .background(
                                RoundedRectangle(cornerRadius: 14, style: .continuous)
                                    .fill(viewModel.state.conversation.sessionID == thread.id ? tokens.elevatedBackground : Color.clear)
                            )
                        }
                        .buttonStyle(.plain)
                        .contextMenu {
                            Button("Open Thread") {
                                Task { await viewModel.selectThread(thread.id) }
                            }
                        }
                    }
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Quick Actions")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    ForEach(viewModel.state.sidebar.quickActions) { action in
                        quickActionRow(for: action, tokens: tokens)
                    }
                }
            }
            .padding(16)
        }
    }

    @ViewBuilder
    private func quickActionRow(for action: QuickAction, tokens: ThemeTokens) -> some View {
        switch action.id {
        case "new-thread":
            Button {
                viewModel.newThread()
            } label: {
                Label(action.title, systemImage: action.systemImage)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .foregroundStyle(tokens.secondaryText)
        case "refresh":
            Button {
                Task { await viewModel.refresh() }
            } label: {
                Label(action.title, systemImage: action.systemImage)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .foregroundStyle(tokens.secondaryText)
        case "settings":
            SettingsLink {
                Label(action.title, systemImage: action.systemImage)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .foregroundStyle(tokens.secondaryText)
        default:
            Label(action.title, systemImage: action.systemImage)
                .foregroundStyle(tokens.secondaryText)
        }
    }
}

struct ConversationContent: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 0) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(viewModel.state.conversation.threadTitle)
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    StatusStripView(status: viewModel.state.conversation.status)
                }
                Spacer()
            }
            .padding(20)
            .background(.ultraThinMaterial)

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 16) {
                        if viewModel.state.conversation.messages.isEmpty {
                            ContentUnavailableView(
                                "No messages yet",
                                systemImage: "bubble.left.and.bubble.right",
                                description: Text("Start a new conversation in this workspace.")
                            )
                            .frame(maxWidth: .infinity, minHeight: 320)
                        } else {
                            ForEach(viewModel.state.conversation.messages) { message in
                                MessageBubbleView(message: message)
                            }
                        }
                    }
                    .padding(24)
                }
                .onChange(of: viewModel.state.conversation.messages.count) {
                    if let last = viewModel.state.conversation.messages.last?.id {
                        proxy.scrollTo(last, anchor: .bottom)
                    }
                }
            }

            if let inlineError = viewModel.state.conversation.inlineError {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                    Text(inlineError)
                    Spacer()
                }
                .font(.caption)
                .foregroundStyle(tokens.failure)
                .padding(.horizontal, 20)
                .padding(.top, 10)
            }

            VStack(spacing: 12) {
                TextEditor(text: $viewModel.composerText)
                    .font(.system(size: 14))
                    .frame(minHeight: 92)
                    .scrollContentBackground(.hidden)
                    .padding(12)
                    .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))

                HStack {
                    Text("Profile-aware CLI runtime")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Button(viewModel.state.conversation.isSending ? "Sending…" : "Send") {
                        Task { await viewModel.sendCurrentPrompt() }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(viewModel.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || viewModel.state.conversation.isSending)
                }
            }
            .padding(20)
            .background(.ultraThinMaterial)
        }
    }
}

struct InspectorContent: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(viewModel.state.inspector.sections) { section in
                    VStack(alignment: .leading, spacing: 10) {
                        Text(section.title)
                            .font(.headline)
                            .foregroundStyle(tokens.primaryText)
                        ForEach(section.rows) { row in
                            HStack(alignment: .top) {
                                Text(row.label)
                                    .foregroundStyle(tokens.secondaryText)
                                Spacer()
                                Text(row.value)
                                    .foregroundStyle(tokens.primaryText)
                                    .multilineTextAlignment(.trailing)
                            }
                            .font(.system(size: 13))
                        }
                    }
                    .padding(14)
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                }
            }
            .padding(16)
        }
    }
}

struct StatusStripView: View {
    let status: RuntimeStripState
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let toneColor: Color = switch status.tone {
        case .quiet: tokens.tertiaryText
        case .success: tokens.success
        case .warning: tokens.warning
        case .failure: tokens.failure
        }

        HStack(spacing: 8) {
            Circle()
                .fill(toneColor)
                .frame(width: 8, height: 8)
            Text(status.title)
                .font(.system(size: 12, weight: .semibold))
            Text(status.detail)
                .font(.system(size: 12))
                .foregroundStyle(tokens.secondaryText)
        }
    }
}

struct MessageBubbleView: View {
    let message: ConversationMessage
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let isUser = message.role == .user
        let bubbleBackground: Color = switch message.role {
        case .assistant:
            tokens.panelBackground
        case .user:
            tokens.elevatedBackground
        case .system:
            tokens.windowBackground.opacity(0.72)
        }

        VStack(alignment: isUser ? .trailing : .leading, spacing: 8) {
            HStack {
                if !isUser { Spacer(minLength: 0) }
                VStack(alignment: .leading, spacing: 8) {
                    Text(message.role.title)
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    Text(message.body)
                        .font(.system(size: 15))
                        .foregroundStyle(tokens.primaryText)
                        .textSelection(.enabled)
                    Text(message.timestampLabel)
                        .font(.caption2)
                        .foregroundStyle(tokens.tertiaryText)
                }
                .padding(16)
                .background(
                    bubbleBackground,
                    in: RoundedRectangle(cornerRadius: 20, style: .continuous)
                )
                .frame(maxWidth: 720, alignment: isUser ? .trailing : .leading)
                if isUser { Spacer(minLength: 0) }
            }
        }
        .id(message.id)
    }
}
