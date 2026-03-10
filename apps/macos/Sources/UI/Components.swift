import AppKit
import Domain
import Features
import Foundation
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
            if case .setupHub = viewModel.screen {
                SetupBackdrop(tokens: tokens)
                    .ignoresSafeArea()
            } else {
                tokens.windowBackground
                    .ignoresSafeArea()
            }

            switch viewModel.screen {
            case .loading:
                ProgressView("Launching Mosaic…")
                    .controlSize(.large)
            case .setupHub:
                SetupHubView(viewModel: viewModel)
            case .workbench:
                if let workbench = viewModel.workbench {
                    WorkbenchView(appViewModel: viewModel, viewModel: workbench)
                }
            case let .error(message):
                ContentUnavailableView(
                    "Unable to Open Workspace",
                    systemImage: "exclamationmark.triangle",
                    description: Text(message)
                )
            }

            if viewModel.isCommandPalettePresented {
                CommandPaletteOverlay(viewModel: viewModel)
                    .transition(.opacity.combined(with: .scale(scale: 0.98, anchor: .top)))
                    .zIndex(1)
            }
        }
        .animation(.spring(response: 0.24, dampingFraction: 0.88), value: viewModel.isCommandPalettePresented)
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

public struct SetupHubView: View {
    @Bindable private var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                SetupHeroDeck(viewModel: viewModel)

                HStack(alignment: .top, spacing: 20) {
                    SetupWorkspaceColumn(viewModel: viewModel)
                    SetupActionColumn(viewModel: viewModel)
                }

                if !viewModel.recentWorkspaces.isEmpty {
                    RecentWorkspaceSection(viewModel: viewModel)
                }
            }
            .padding(28)
            .frame(maxWidth: 1180, alignment: .leading)
        }
        .scrollIndicators(.hidden)
    }
}

private struct SetupBackdrop: View {
    let tokens: ThemeTokens

    var body: some View {
        ZStack {
            tokens.windowBackground

            LinearGradient(
                colors: [
                    tokens.accent.opacity(0.14),
                    Color.clear,
                    tokens.success.opacity(0.08),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )

            Circle()
                .fill(tokens.accent.opacity(0.14))
                .frame(width: 420, height: 420)
                .blur(radius: 70)
                .offset(x: -240, y: -260)

            Circle()
                .fill(tokens.success.opacity(0.10))
                .frame(width: 360, height: 360)
                .blur(radius: 72)
                .offset(x: 360, y: 260)
        }
    }
}

private struct SetupHeroDeck: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top, spacing: 24) {
                VStack(alignment: .leading, spacing: 14) {
                    Text("AZURE-FIRST PROJECT SETUP")
                        .font(.system(size: 12, weight: .semibold, design: .rounded))
                        .tracking(1.2)
                        .foregroundStyle(tokens.accent)

                    Text("Choose a project, connect Azure, and open a real workspace.")
                        .font(.system(size: 34, weight: .bold, design: .rounded))
                        .foregroundStyle(tokens.primaryText)
                        .fixedSize(horizontal: false, vertical: true)

                    Text("The flow stays project-first like Codex, but the default runtime path now starts from Azure OpenAI. The UI exposes the Azure resource directly so setup feels closer to LM Studio’s workstation control surface and less like a raw config form.")
                        .font(.system(size: 14))
                        .foregroundStyle(tokens.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)

                    HStack(spacing: 10) {
                        SetupStageChip(index: 1, title: "Project", detail: "Pick a local workspace")
                        SetupStageChip(index: 2, title: "Azure", detail: "Enter the resource host")
                        SetupStageChip(index: 3, title: "Launch", detail: "Initialize and open the workbench")
                    }

                    HStack(spacing: 10) {
                        SetupMetricPill(title: "Provider", value: viewModel.onboardingProviderLabel)
                        SetupMetricPill(title: "Model", value: viewModel.onboardingModel)
                        SetupMetricPill(title: "API env", value: viewModel.onboardingAPIKeyEnv)
                    }
                }

                VStack(alignment: .leading, spacing: 12) {
                    Text("Recommended Azure bootstrap")
                        .font(.headline)
                        .foregroundStyle(tokens.primaryText)

                    SetupValueRow(label: "Resource URL", value: viewModel.onboardingBaseURL)
                    SetupValueRow(label: "Model / deployment", value: viewModel.onboardingModel)
                    SetupValueRow(label: "API key env", value: viewModel.onboardingAPIKeyEnv)

                    if viewModel.onboardingRequiresAzureResourceHost {
                        SetupHintRow(
                            systemImage: "exclamationmark.triangle.fill",
                            text: "Replace `YOUR_RESOURCE_NAME` with your Azure resource host before initializing."
                        )
                    } else {
                        SetupHintRow(
                            systemImage: "checkmark.circle.fill",
                            text: "Azure resource host looks concrete. You can initialize the workspace now."
                        )
                    }
                }
                .padding(16)
                .frame(maxWidth: 360, alignment: .leading)
                .background(tokens.panelBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            }

            StatusHeroView(
                title: viewModel.setupStatusTitle,
                detail: viewModel.setupStatusDetail,
                tone: viewModel.setupStatusTone
            )
        }
        .padding(26)
        .background(
            LinearGradient(
                colors: [
                    tokens.panelBackground.opacity(0.90),
                    tokens.elevatedBackground.opacity(0.84),
                    tokens.accent.opacity(0.14),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            ),
            in: RoundedRectangle(cornerRadius: 30, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 30, style: .continuous)
                .stroke(tokens.border.opacity(0.8), lineWidth: 1)
        )
        .shadow(color: Color.black.opacity(colorScheme == .dark ? 0.16 : 0.08), radius: 28, y: 10)
    }
}

private struct SetupWorkspaceColumn: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            Text("Workspace Root")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            VStack(alignment: .leading, spacing: 12) {
                if let workspace = viewModel.selectedWorkspace {
                    Text(workspace.name)
                        .font(.system(size: 24, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(workspace.path)
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                        .textSelection(.enabled)
                } else {
                    Text("No workspace selected")
                        .font(.system(size: 24, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text("Choose a local project folder first. Mosaic will keep runtime state, sessions, and config anchored to this workspace.")
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                }
            }

            HStack(spacing: 10) {
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

                Button("Refresh Status") {
                    Task { await viewModel.refreshSelectedWorkspace() }
                }
                .buttonStyle(.bordered)
                .disabled(viewModel.selectedWorkspace == nil || viewModel.isPreparingWorkspace)

                Button("Reveal in Finder") {
                    viewModel.revealSelectedWorkspaceInFinder()
                }
                .buttonStyle(.bordered)
                .disabled(viewModel.selectedWorkspace == nil)
            }

            Divider()

            VStack(alignment: .leading, spacing: 12) {
                Text("What Mosaic anchors to this project")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(tokens.primaryText)
                SetupChecklistItem(text: "The existing project directory as your workspace root")
                SetupChecklistItem(text: "A project-local Mosaic config once you initialize")
                SetupChecklistItem(text: "Session history and runtime health scoped to this workspace")
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tokens.panelBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(tokens.border.opacity(0.85), lineWidth: 1)
        )
    }
}

private struct SetupActionColumn: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            Text("Runtime Setup")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            HStack(spacing: 10) {
                SetupMetricPill(title: "Profile", value: viewModel.currentProfileLabel)
                SetupMetricPill(title: "Model", value: viewModel.currentModelLabel)
                SetupMetricPill(title: "Health", value: viewModel.currentHealthLabel)
            }

            if viewModel.selectedWorkspaceConfigured {
                VStack(alignment: .leading, spacing: 12) {
                    Text("This workspace is already configured. Open it directly, continue the latest thread, or tune the runtime without dropping to the CLI.")
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                    SetupValueRow(label: "Provider", value: viewModel.currentProviderLabel)
                    SetupValueRow(label: "Base URL", value: viewModel.currentBaseURLLabel)
                    SetupValueRow(label: "API env", value: viewModel.currentAPIKeyEnvLabel)
                    Button(viewModel.isPreparingWorkspace ? "Opening…" : "Open Workspace") {
                        Task { await viewModel.openSelectedWorkspace() }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!viewModel.canOpenWorkspace)

                    RuntimeControlsCard(viewModel: viewModel, title: "Adjust runtime")
                }
            } else {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Azure OpenAI is now the default initialization path. Pick a workspace, replace the Azure resource host, and initialize the project-local Mosaic config.")
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)

                    ProviderPresetPicker(
                        azureSelected: viewModel.onboardingUsesAzurePreset,
                        azureAction: viewModel.applyAzurePreset,
                        localAction: viewModel.applyLocalPreset
                    )

                    if viewModel.onboardingUsesAzurePreset {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Azure resource")
                                .font(.caption)
                                .foregroundStyle(tokens.tertiaryText)
                            TextField(
                                "my-resource",
                                text: Binding(
                                    get: { viewModel.onboardingAzureResourceName },
                                    set: { viewModel.onboardingAzureResourceName = $0 }
                                )
                            )
                            .textFieldStyle(.roundedBorder)

                            SetupValueRow(label: "Resolved URL", value: viewModel.onboardingBaseURL)
                        }
                    } else {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Base URL")
                                .font(.caption)
                                .foregroundStyle(tokens.tertiaryText)
                            TextField("Base URL", text: $viewModel.onboardingBaseURL)
                                .textFieldStyle(.roundedBorder)
                        }
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text(viewModel.onboardingUsesAzurePreset ? "Model / deployment" : "Model")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                        ModelChoiceStrip(
                            selectedModel: viewModel.onboardingModel,
                            options: viewModel.setupModelChoices,
                            onSelect: viewModel.selectOnboardingModel
                        )
                        TextField("Model", text: $viewModel.onboardingModel)
                            .textFieldStyle(.roundedBorder)
                    }

                    VStack(alignment: .leading, spacing: 8) {
                        Text("API key env")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                        TextField("API Key Env", text: $viewModel.onboardingAPIKeyEnv)
                            .textFieldStyle(.roundedBorder)
                    }

                    if viewModel.onboardingRequiresAzureResourceHost {
                        SetupHintRow(
                            systemImage: "exclamationmark.triangle.fill",
                            text: "Initialization stays disabled until the Azure base URL points at a real resource host."
                        )
                    }

                    Button(viewModel.isInitializingWorkspace ? "Initializing…" : "Initialize with Recommended Defaults") {
                        Task { await viewModel.completeOnboarding() }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!viewModel.canInitializeWorkspace)

                    DisclosureGroup("Advanced setup", isExpanded: $viewModel.showAdvancedSetup) {
                        VStack(alignment: .leading, spacing: 10) {
                            SetupValueRow(label: "Provider", value: viewModel.onboardingProviderLabel)
                            TextField("Base URL", text: $viewModel.onboardingBaseURL)
                            TextField("Model", text: $viewModel.onboardingModel)
                            TextField("API Key Env", text: $viewModel.onboardingAPIKeyEnv)
                        }
                        .textFieldStyle(.roundedBorder)
                        .padding(.top, 10)
                    }
                    .disabled(viewModel.selectedWorkspace == nil)
                }
            }

            Divider()

            VStack(alignment: .leading, spacing: 8) {
                Text("Why this flow")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(tokens.primaryText)
                Text("The app leads with project context, but the runtime defaults now bias toward Azure OpenAI or a local OpenAI-compatible server instead of a generic cloud preset.")
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tokens.panelBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(tokens.border.opacity(0.85), lineWidth: 1)
        )
    }
}

private struct RecentWorkspaceSection: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 14) {
            Text("Recent Workspaces")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), spacing: 14)], spacing: 14) {
                ForEach(viewModel.recentWorkspaces) { workspace in
                    Button {
                        Task { await viewModel.previewWorkspace(workspace) }
                    } label: {
                        VStack(alignment: .leading, spacing: 8) {
                            Text(workspace.name)
                                .font(.subheadline.weight(.semibold))
                                .foregroundStyle(tokens.primaryText)
                                .lineLimit(1)
                            Text(workspace.path)
                                .font(.caption)
                                .foregroundStyle(tokens.secondaryText)
                                .lineLimit(2)
                            if let lastOpenedAt = workspace.lastOpenedAt {
                                Text(relativeDateLabel(for: lastOpenedAt))
                                    .font(.caption2)
                                    .foregroundStyle(tokens.tertiaryText)
                            }
                        }
                        .padding(16)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 18, style: .continuous)
                                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
                        )
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func relativeDateLabel(for date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .full
        return "Opened \(formatter.localizedString(for: date, relativeTo: Date()))"
    }
}

private struct StatusHeroView: View {
    let title: String
    let detail: String
    let tone: RuntimeStripState.Tone
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let toneColor: Color = switch tone {
        case .quiet: tokens.tertiaryText
        case .success: tokens.success
        case .warning: tokens.warning
        case .failure: tokens.failure
        }

        HStack(alignment: .top, spacing: 16) {
            Circle()
                .fill(toneColor)
                .frame(width: 10, height: 10)
                .padding(.top, 5)
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(detail)
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
        }
        .padding(18)
        .background(tokens.panelBackground.opacity(0.86), in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(tokens.border.opacity(0.78), lineWidth: 1)
        )
    }
}

private struct SetupMetricPill: View {
    let title: String
    let value: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption2)
                .foregroundStyle(tokens.tertiaryText)
            Text(value)
                .font(.caption.weight(.semibold))
                .foregroundStyle(tokens.primaryText)
                .lineLimit(1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(tokens.panelBackground, in: Capsule())
    }
}

private struct SetupStageChip: View {
    let index: Int
    let title: String
    let detail: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 10) {
            Text("\(index)")
                .font(.caption.weight(.bold))
                .foregroundStyle(tokens.primaryText)
                .frame(width: 22, height: 22)
                .background(tokens.accent.opacity(0.2), in: Circle())

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(detail)
                    .font(.caption2)
                    .foregroundStyle(tokens.secondaryText)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(tokens.panelBackground.opacity(0.86), in: Capsule())
    }
}

private struct ProviderPresetPicker: View {
    let azureSelected: Bool
    let azureAction: () -> Void
    let localAction: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 10) {
            ProviderPresetButton(
                title: "Azure OpenAI",
                subtitle: "Hosted runtime",
                systemImage: "cloud.fill",
                isSelected: azureSelected,
                action: azureAction
            )

            ProviderPresetButton(
                title: "Local Server",
                subtitle: "localhost / LM Studio",
                systemImage: "desktopcomputer",
                isSelected: !azureSelected,
                action: localAction
            )
        }
        .padding(4)
        .background(tokens.windowBackground.opacity(0.38), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }
}

private struct ProviderPresetButton: View {
    let title: String
    let subtitle: String
    let systemImage: String
    let isSelected: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: systemImage)
                    .foregroundStyle(isSelected ? tokens.primaryText : tokens.accent)
                    .frame(width: 20)

                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(subtitle)
                        .font(.caption2)
                        .foregroundStyle(tokens.secondaryText)
                }

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(
                (isSelected ? tokens.elevatedBackground : Color.clear),
                in: RoundedRectangle(cornerRadius: 14, style: .continuous)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(isSelected ? tokens.accent.opacity(0.45) : Color.clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }
}

private struct SetupHintRow: View {
    let systemImage: String
    let text: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 8) {
            Image(systemName: systemImage)
                .foregroundStyle(systemImage.contains("exclamationmark") ? tokens.warning : tokens.success)
            Text(text)
                .font(.system(size: 12))
                .foregroundStyle(tokens.secondaryText)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
        .background(tokens.elevatedBackground.opacity(0.8), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

private struct SetupChecklistItem: View {
    let text: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(tokens.accent)
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(tokens.secondaryText)
        }
    }
}

private struct SetupValueRow: View {
    let label: String
    let value: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack {
            Text(label)
                .foregroundStyle(tokens.secondaryText)
            Spacer()
            Text(value)
                .foregroundStyle(tokens.primaryText)
                .font(.system(size: 13, design: .monospaced))
        }
        .font(.system(size: 13))
    }
}

private struct CommandPaletteItem: Identifiable {
    enum Group: String {
        case recent = "Recent Actions"
        case workspace = "Workspace"
        case runtime = "Runtime"
        case conversation = "Conversation"
        case projects = "Projects"
        case system = "System"
    }

    let id: String
    let title: String
    let subtitle: String
    let systemImage: String
    let shortcutLabel: String?
    let enabled: Bool
    let group: Group
    let keywords: [String]
    let perform: () -> Void

    func matches(_ query: String) -> Bool {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !normalized.isEmpty else { return true }
        let haystack = ([title, subtitle] + keywords).joined(separator: " ").lowercased()
        return haystack.contains(normalized)
    }
}

private struct CommandPaletteSection: Identifiable {
    let id: CommandPaletteItem.Group
    let title: String
    let items: [CommandPaletteItem]
}

private struct CommandPaletteOverlay: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openSettings) private var openSettings
    @FocusState private var isSearchFocused: Bool
    @State private var query = ""
    @State private var highlightedItemID: String?

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let filteredItems = filteredPaletteItems
        let filteredItemIDs = filteredItems.map(\.id)

        ZStack(alignment: .top) {
            Color.black.opacity(colorScheme == .dark ? 0.34 : 0.14)
                .ignoresSafeArea()
                .onTapGesture {
                    viewModel.dismissCommandPalette()
                }

            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 12) {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(tokens.tertiaryText)

                    TextField("Search commands, workspaces, and actions", text: $query)
                        .textFieldStyle(.plain)
                        .font(.system(size: 16))
                        .focused($isSearchFocused)
                        .onSubmit {
                            executeHighlightedItem(in: filteredItems)
                        }

                    HStack(spacing: 6) {
                        CommandKeycap(label: "↑↓")
                        CommandKeycap(label: "↩")
                        CommandKeycap(label: "Esc")
                    }
                }
                .padding(18)

                Divider()

                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 14) {
                            if filteredItems.isEmpty {
                                ContentUnavailableView(
                                    "No matching commands",
                                    systemImage: "magnifyingglass",
                                    description: Text("Try searching for workspace, thread, runtime, or settings.")
                                )
                                .frame(maxWidth: .infinity, minHeight: 220)
                            } else {
                                ForEach(filteredSections) { section in
                                    VStack(alignment: .leading, spacing: 8) {
                                        Text(section.title)
                                            .font(.caption.weight(.semibold))
                                            .foregroundStyle(tokens.tertiaryText)
                                            .padding(.horizontal, 2)

                                        ForEach(section.items) { item in
                                            Button {
                                                highlightedItemID = item.id
                                                execute(item)
                                            } label: {
                                                CommandPaletteRow(
                                                    item: item,
                                                    isHighlighted: item.id == highlightedItemID
                                                )
                                            }
                                            .buttonStyle(.plain)
                                            .disabled(!item.enabled)
                                            .opacity(item.enabled ? 1 : 0.42)
                                            .onHover { isHovering in
                                                guard isHovering, item.enabled else { return }
                                                highlightedItemID = item.id
                                            }
                                            .id(item.id)
                                        }
                                    }
                                }
                            }
                        }
                        .padding(12)
                    }
                    .frame(maxHeight: 420)
                    .onChange(of: highlightedItemID) {
                        guard let highlightedItemID else { return }
                        withAnimation(.easeInOut(duration: 0.12)) {
                            proxy.scrollTo(highlightedItemID, anchor: .center)
                        }
                    }
                }

                Button("") {
                    viewModel.dismissCommandPalette()
                }
                .keyboardShortcut(.cancelAction)
                .frame(width: 0, height: 0)
                .opacity(0)
            }
            .frame(maxWidth: 760)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 26, style: .continuous)
                    .stroke(tokens.elevatedBackground.opacity(0.7), lineWidth: 1)
            )
            .shadow(color: Color.black.opacity(colorScheme == .dark ? 0.32 : 0.16), radius: 24, y: 10)
            .padding(.horizontal, 28)
            .padding(.top, 36)
        }
        .background(
            CommandPaletteKeyMonitor { event in
                handleKeyEvent(event, items: filteredItems)
            }
        )
        .onAppear {
            query = ""
            syncHighlightedItem(with: filteredItems)
            DispatchQueue.main.async {
                isSearchFocused = true
            }
        }
        .onChange(of: query) {
            syncHighlightedItem(with: filteredItems)
        }
        .onChange(of: filteredItemIDs) {
            syncHighlightedItem(with: filteredItems)
        }
    }

    private var paletteItems: [CommandPaletteItem] {
        var items: [CommandPaletteItem] = [
            CommandPaletteItem(
                id: "choose-workspace",
                title: "Choose Workspace",
                subtitle: "Return to the setup hub and switch projects.",
                systemImage: "folder",
                shortcutLabel: "Cmd+Shift+O",
                enabled: true,
                group: .workspace,
                keywords: ["workspace", "project", "folder"]
            ) {
                viewModel.dismissCommandPalette()
                viewModel.showSetupHub()
            },
            CommandPaletteItem(
                id: "settings",
                title: "Open Settings",
                subtitle: "Review runtime defaults, recent workspaces, and desktop configuration.",
                systemImage: "gearshape",
                shortcutLabel: "Cmd+,",
                enabled: true,
                group: .system,
                keywords: ["settings", "preferences", "runtime"]
            ) {
                viewModel.dismissCommandPalette()
                openSettings()
            },
        ]

        if let workspace = viewModel.selectedWorkspace {
            items.append(
                CommandPaletteItem(
                    id: "refresh-workspace",
                    title: "Refresh Workspace",
                    subtitle: "Reload runtime status, health, sessions, and model state for \(workspace.name).",
                    systemImage: "arrow.clockwise",
                    shortcutLabel: "Cmd+Shift+R",
                    enabled: !viewModel.isPreparingWorkspace,
                    group: .workspace,
                    keywords: ["refresh", "status", "health", workspace.name]
                ) {
                    viewModel.dismissCommandPalette()
                    Task { await viewModel.refreshActiveWorkspace() }
                }
            )

            items.append(
                CommandPaletteItem(
                    id: "reveal-workspace",
                    title: "Reveal in Finder",
                    subtitle: workspace.path,
                    systemImage: "folder.badge.gearshape",
                    shortcutLabel: "Cmd+Opt+O",
                    enabled: true,
                    group: .workspace,
                    keywords: ["finder", "path", workspace.name]
                ) {
                    viewModel.dismissCommandPalette()
                    viewModel.revealSelectedWorkspaceInFinder()
                }
            )

            if viewModel.selectedWorkspaceConfigured {
                items.append(
                    CommandPaletteItem(
                        id: "open-workspace",
                        title: "Open Workspace",
                        subtitle: "Enter the main workbench and resume the latest thread.",
                        systemImage: "play.rectangle",
                        shortcutLabel: nil,
                        enabled: viewModel.canOpenWorkspace,
                        group: .workspace,
                        keywords: ["open", "resume", "workbench", workspace.name]
                    ) {
                        viewModel.dismissCommandPalette()
                        Task { await viewModel.openSelectedWorkspace() }
                    }
                )
            } else {
                items.append(
                    CommandPaletteItem(
                        id: "initialize-workspace",
                        title: "Initialize Workspace",
                        subtitle: "Create the project-local Mosaic config with the recommended defaults.",
                        systemImage: "wand.and.stars",
                        shortcutLabel: nil,
                        enabled: viewModel.canInitializeWorkspace,
                        group: .workspace,
                        keywords: ["setup", "initialize", "onboarding", workspace.name]
                    ) {
                        viewModel.dismissCommandPalette()
                        Task { await viewModel.completeOnboarding() }
                    }
                )
            }
        }

        if viewModel.selectedWorkspaceConfigured {
            items.append(
                CommandPaletteItem(
                    id: "save-runtime",
                    title: "Save Runtime Settings",
                    subtitle: "Persist the current runtime draft back into the workspace config.",
                    systemImage: "internaldrive",
                    shortcutLabel: nil,
                    enabled: viewModel.canSaveRuntimeSettings,
                    group: .runtime,
                    keywords: ["save", "runtime", "model", "base url", "api key env"]
                ) {
                    viewModel.dismissCommandPalette()
                    Task { await viewModel.saveRuntimeSettings() }
                }
            )
        }

        if let workbench = viewModel.workbench {
            items.append(
                CommandPaletteItem(
                    id: "new-thread",
                    title: "New Thread",
                    subtitle: "Start a fresh conversation without leaving the current workspace.",
                    systemImage: "square.and.pencil",
                    shortcutLabel: "Cmd+N",
                    enabled: true,
                    group: .conversation,
                    keywords: ["thread", "conversation", "chat"]
                ) {
                    viewModel.dismissCommandPalette()
                    viewModel.createNewThread()
                }
            )

            items.append(
                CommandPaletteItem(
                    id: "send-prompt",
                    title: "Send Prompt",
                    subtitle: "Submit the current composer text.",
                    systemImage: "paperplane",
                    shortcutLabel: "Cmd+Return",
                    enabled: !workbench.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !workbench.state.conversation.isSending,
                    group: .conversation,
                    keywords: ["send", "prompt", "composer", "chat"]
                ) {
                    viewModel.dismissCommandPalette()
                    Task { await viewModel.sendCurrentPrompt() }
                }
            )

            items.append(
                CommandPaletteItem(
                    id: "clear-thread",
                    title: "Clear Selected Thread",
                    subtitle: "Remove the active session and move to the next available thread.",
                    systemImage: "trash",
                    shortcutLabel: "Cmd+Delete",
                    enabled: workbench.canClearSelectedThread,
                    group: .conversation,
                    keywords: ["clear", "delete", "session", "thread"]
                ) {
                    viewModel.dismissCommandPalette()
                    Task { await viewModel.clearCurrentThread() }
                }
            )

            items.append(
                CommandPaletteItem(
                    id: "toggle-inspector",
                    title: workbench.isInspectorVisible ? "Hide Inspector" : "Show Inspector",
                    subtitle: "Toggle the runtime/context side panel.",
                    systemImage: workbench.isInspectorVisible ? "sidebar.right" : "sidebar.left",
                    shortcutLabel: "Cmd+Opt+I",
                    enabled: true,
                    group: .runtime,
                    keywords: ["inspector", "panel", "sidebar"]
                ) {
                    viewModel.dismissCommandPalette()
                    viewModel.toggleInspector()
                }
            )
        }

        for workspace in viewModel.recentWorkspaces where workspace.id != viewModel.selectedWorkspace?.id {
            items.append(
                CommandPaletteItem(
                    id: "workspace-\(workspace.id.uuidString)",
                    title: "Switch to \(workspace.name)",
                    subtitle: workspace.path,
                    systemImage: "folder",
                    shortcutLabel: nil,
                    enabled: true,
                    group: .projects,
                    keywords: ["switch", "workspace", workspace.name, workspace.path]
                ) {
                    viewModel.dismissCommandPalette()
                    Task { await viewModel.selectWorkspace(workspace) }
                }
            )
        }

        return items
    }

    private var filteredPaletteItems: [CommandPaletteItem] {
        paletteItems.filter { $0.matches(query) }
    }

    private var filteredSections: [CommandPaletteSection] {
        let baseItems = filteredPaletteItems
        let recentIDs = viewModel.recentCommandActionIDs
        let recentItems = recentIDs.compactMap { id in
            baseItems.first(where: { $0.id == id })
        }

        var usedIDs = Set(recentItems.map(\.id))
        var sections: [CommandPaletteSection] = []

        if !recentItems.isEmpty {
            sections.append(
                CommandPaletteSection(
                    id: .recent,
                    title: CommandPaletteItem.Group.recent.rawValue,
                    items: recentItems
                )
            )
        }

        let orderedGroups: [CommandPaletteItem.Group] = [.workspace, .runtime, .conversation, .projects, .system]
        for group in orderedGroups {
            let items = baseItems.filter { item in
                item.group == group && !usedIDs.contains(item.id)
            }
            guard !items.isEmpty else { continue }
            sections.append(
                CommandPaletteSection(
                    id: group,
                    title: group.rawValue,
                    items: items
                )
            )
            usedIDs.formUnion(items.map(\.id))
        }

        return sections
    }

    private func executeFirstMatch(_ items: [CommandPaletteItem]) {
        guard let match = items.first(where: \.enabled) else { return }
        execute(match)
    }

    private func executeHighlightedItem(in items: [CommandPaletteItem]) {
        guard !items.isEmpty else { return }
        if let highlightedItemID,
           let item = items.first(where: { $0.id == highlightedItemID && $0.enabled }) {
            execute(item)
            return
        }
        executeFirstMatch(items)
    }

    private func execute(_ item: CommandPaletteItem) {
        guard item.enabled else { return }
        item.perform()
        Task {
            await viewModel.recordCommandAction(item.id)
        }
    }

    private func syncHighlightedItem(with items: [CommandPaletteItem]) {
        let enabledItems = items.filter(\.enabled)
        guard !enabledItems.isEmpty else {
            highlightedItemID = nil
            return
        }

        if let highlightedItemID,
           enabledItems.contains(where: { $0.id == highlightedItemID }) {
            return
        }

        highlightedItemID = enabledItems[0].id
    }

    private func moveSelection(in items: [CommandPaletteItem], direction: Int) {
        let enabledItems = items.filter(\.enabled)
        guard !enabledItems.isEmpty else {
            highlightedItemID = nil
            return
        }

        guard let currentHighlightedID = highlightedItemID,
              let currentIndex = enabledItems.firstIndex(where: { $0.id == currentHighlightedID }) else {
            self.highlightedItemID = enabledItems[0].id
            return
        }

        let count = enabledItems.count
        let nextIndex = (currentIndex + direction + count) % count
        highlightedItemID = enabledItems[nextIndex].id
    }

    private func handleKeyEvent(_ event: NSEvent, items: [CommandPaletteItem]) -> NSEvent? {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard modifiers.isEmpty else { return event }

        switch event.keyCode {
        case 125:
            moveSelection(in: items, direction: 1)
            return nil
        case 126:
            moveSelection(in: items, direction: -1)
            return nil
        case 36, 76:
            executeHighlightedItem(in: items)
            return nil
        case 53:
            viewModel.dismissCommandPalette()
            return nil
        default:
            return event
        }
    }
}

private struct CommandPaletteRow: View {
    let item: CommandPaletteItem
    let isHighlighted: Bool
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 12) {
            Image(systemName: item.systemImage)
                .frame(width: 18, height: 18)
                .foregroundStyle(tokens.accent)
                .padding(.top, 2)

            VStack(alignment: .leading, spacing: 4) {
                Text(item.title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(item.subtitle)
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer(minLength: 12)

            if let shortcutLabel = item.shortcutLabel {
                Text(shortcutLabel)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(tokens.tertiaryText)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(tokens.panelBackground, in: Capsule())
            }
        }
        .padding(14)
        .background(
            (isHighlighted ? tokens.elevatedBackground : tokens.panelBackground),
            in: RoundedRectangle(cornerRadius: 18, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(
                    isHighlighted ? tokens.accent.opacity(0.55) : tokens.border.opacity(0.55),
                    lineWidth: 1
                )
        )
    }
}

private struct CommandKeycap: View {
    let label: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Text(label)
            .font(.caption.weight(.semibold))
            .foregroundStyle(tokens.tertiaryText)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(tokens.panelBackground, in: Capsule())
    }
}

private struct CommandPaletteKeyMonitor: NSViewRepresentable {
    let onKeyDown: (NSEvent) -> NSEvent?

    func makeCoordinator() -> Coordinator {
        Coordinator(onKeyDown: onKeyDown)
    }

    func makeNSView(context: Context) -> NSView {
        context.coordinator.start()
        return NSView(frame: .zero)
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.onKeyDown = onKeyDown
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.stop()
    }

    final class Coordinator {
        var onKeyDown: (NSEvent) -> NSEvent?
        private var monitor: Any?

        init(onKeyDown: @escaping (NSEvent) -> NSEvent?) {
            self.onKeyDown = onKeyDown
        }

        func start() {
            guard monitor == nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                guard let self else { return event }
                return self.onKeyDown(event)
            }
        }

        func stop() {
            guard let monitor else { return }
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }

        deinit {
            stop()
        }
    }
}

public struct WorkbenchView: View {
    @Bindable private var appViewModel: AppViewModel
    @Bindable private var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(appViewModel: AppViewModel, viewModel: WorkbenchViewModel) {
        self.appViewModel = appViewModel
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        NavigationSplitView {
            SidebarContent(appViewModel: appViewModel, viewModel: viewModel)
                .navigationSplitViewColumnWidth(min: 280, ideal: 320)
        } content: {
            ConversationContent(appViewModel: appViewModel, viewModel: viewModel)
                .navigationSplitViewColumnWidth(min: 560, ideal: 820)
        } detail: {
            if viewModel.isInspectorVisible {
                InspectorContent(appViewModel: appViewModel, viewModel: viewModel)
                    .navigationSplitViewColumnWidth(min: 280, ideal: 320)
            } else {
                Color.clear
            }
        }
        .background(
            LinearGradient(
                colors: [
                    tokens.windowBackground,
                    tokens.windowBackground,
                    tokens.accent.opacity(colorScheme == .dark ? 0.08 : 0.04),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
        .toolbar {
            ToolbarItem(placement: .principal) {
                WorkbenchToolbarPrincipal(appViewModel: appViewModel, viewModel: viewModel)
            }

            ToolbarItemGroup {
                Button {
                    appViewModel.presentCommandPalette()
                } label: {
                    Image(systemName: "magnifyingglass")
                }
                .help("Command palette")

                Button {
                    viewModel.newThread()
                } label: {
                    Image(systemName: "square.and.pencil")
                }
                .help("New thread")

                Button {
                    appViewModel.showSetupHub()
                } label: {
                    Image(systemName: "folder")
                }
                .help("Switch workspace")

                Button {
                    Task { await viewModel.refresh() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Refresh workspace")

                Button {
                    Task { await viewModel.clearSelectedThread() }
                } label: {
                    Image(systemName: "trash")
                }
                .disabled(!viewModel.canClearSelectedThread)
                .help("Clear selected thread")

                Button {
                    viewModel.toggleInspector()
                } label: {
                    Image(systemName: viewModel.isInspectorVisible ? "sidebar.right" : "sidebar.left")
                }
                .help("Toggle inspector")
            }
        }
    }
}

private struct WorkbenchToolbarPrincipal: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text(viewModel.selectedThreadSummary?.title ?? viewModel.state.conversation.threadTitle)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)

                HStack(spacing: 6) {
                    Text(viewModel.state.sidebar.currentWorkspace.name)
                    Text("•")
                    Text(appViewModel.currentModelLabel)
                    Text("•")
                    Text(appViewModel.currentHealthLabel)
                }
                .font(.caption)
                .foregroundStyle(tokens.tertiaryText)
                .lineLimit(1)
            }

            HStack(spacing: 8) {
                ToolbarPill(
                    title: "Threads",
                    value: "\(viewModel.threadCount)",
                    accent: tokens.accent
                )
                ToolbarPill(
                    title: "Turns",
                    value: "\(viewModel.messageCount)",
                    accent: tokens.success
                )
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(tokens.panelBackground.opacity(0.76), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(tokens.border.opacity(0.65), lineWidth: 1)
        )
    }
}

private struct ToolbarPill: View {
    let title: String
    let value: String
    let accent: Color
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Circle()
                .fill(accent.opacity(0.9))
                .frame(width: 6, height: 6)
            Text(title)
                .font(.caption2)
                .foregroundStyle(tokens.tertiaryText)
            Text(value)
                .font(.caption.weight(.semibold))
                .foregroundStyle(tokens.primaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(tokens.elevatedBackground.opacity(0.86), in: Capsule())
    }
}

struct SidebarContent: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Current Workspace")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    VStack(alignment: .leading, spacing: 8) {
                        Text(viewModel.state.sidebar.currentWorkspace.name)
                            .font(.headline)
                        Text(viewModel.state.sidebar.currentWorkspace.path)
                            .font(.caption)
                            .foregroundStyle(tokens.secondaryText)
                            .lineLimit(2)
                        HStack(spacing: 10) {
                            Button("Switch") {
                                appViewModel.showSetupHub()
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(tokens.accent)

                            Button("Reveal") {
                                appViewModel.revealSelectedWorkspaceInFinder()
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(tokens.secondaryText)
                        }
                    }
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(tokens.panelBackground.opacity(0.88), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .stroke(tokens.border.opacity(0.7), lineWidth: 1)
                    )
                }

                if !viewModel.state.sidebar.recentWorkspaces.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Recent Workspaces")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                        ForEach(viewModel.state.sidebar.recentWorkspaces) { workspace in
                            Button {
                                Task { await appViewModel.selectWorkspace(workspace) }
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(workspace.name)
                                        .foregroundStyle(tokens.primaryText)
                                    Text(workspace.path)
                                        .font(.caption)
                                        .foregroundStyle(tokens.tertiaryText)
                                        .lineLimit(1)
                                }
                                .padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(tokens.panelBackground.opacity(0.78), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }

                VStack(alignment: .leading, spacing: 10) {
                    Text("Threads")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)

                    TextField("Filter threads", text: $viewModel.threadFilter)
                        .textFieldStyle(.roundedBorder)

                    Button("New Thread") {
                        viewModel.newThread()
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(tokens.accent)

                    if viewModel.filteredThreads.isEmpty {
                        Text(viewModel.threadFilter.isEmpty ? "No sessions yet." : "No matching threads.")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                    } else {
                        ForEach(viewModel.filteredThreads) { thread in
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
                                        .fill(viewModel.state.conversation.sessionID == thread.id ? tokens.elevatedBackground : tokens.panelBackground.opacity(0.68))
                                )
                                .overlay(
                                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                                        .stroke(
                                            viewModel.state.conversation.sessionID == thread.id ? tokens.accent.opacity(0.4) : tokens.border.opacity(0.5),
                                            lineWidth: 1
                                        )
                                )
                            }
                            .buttonStyle(.plain)
                            .contextMenu {
                                Button("Open Thread") {
                                    Task { await viewModel.selectThread(thread.id) }
                                }
                                Button("Clear Thread", role: .destructive) {
                                    Task { await viewModel.clearThread(thread.id) }
                                }
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
        case "switch-workspace":
            Button {
                appViewModel.showSetupHub()
            } label: {
                Label(action.title, systemImage: action.systemImage)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .foregroundStyle(tokens.secondaryText)
        case "reveal-workspace":
            Button {
                appViewModel.revealSelectedWorkspaceInFinder()
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
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 0) {
            ConversationHeaderCard(appViewModel: appViewModel, viewModel: viewModel)
                .padding(.horizontal, 20)
                .padding(.top, 18)
                .padding(.bottom, 8)

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 16) {
                        if viewModel.state.conversation.messages.isEmpty {
                            EmptyConversationView(appViewModel: appViewModel, viewModel: viewModel)
                                .frame(maxWidth: .infinity, minHeight: 360)
                        } else {
                            ForEach(viewModel.state.conversation.messages) { message in
                                ConversationTimelineRow(message: message)
                            }
                        }
                    }
                    .padding(.horizontal, 24)
                    .padding(.top, 8)
                    .padding(.bottom, 22)
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

            ComposerDock(appViewModel: appViewModel, viewModel: viewModel)
                .padding(.horizontal, 20)
                .padding(.top, 6)
                .padding(.bottom, 20)
        }
        .background(tokens.windowBackground)
    }
}

private struct ConversationHeaderCard: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 12) {
                Text(viewModel.state.conversation.threadTitle)
                    .font(.system(size: 26, weight: .bold, design: .rounded))
                    .foregroundStyle(tokens.primaryText)
                    .fixedSize(horizontal: false, vertical: true)

                Text(viewModel.selectedThreadSummary?.subtitle ?? "Project-first conversation workspace")
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .lineLimit(2)

                StatusStripView(status: viewModel.state.conversation.status)
            }

            Spacer(minLength: 18)

            VStack(alignment: .trailing, spacing: 10) {
                HStack(spacing: 8) {
                    ConversationMetricBadge(
                        title: "Session",
                        value: viewModel.state.conversation.sessionID.map { String($0.prefix(8)) } ?? "New",
                        accent: tokens.accent
                    )
                    ConversationMetricBadge(
                        title: "Turns",
                        value: "\(viewModel.messageCount)",
                        accent: tokens.success
                    )
                    ConversationMetricBadge(
                        title: "Threads",
                        value: "\(viewModel.threadCount)",
                        accent: tokens.warning
                    )
                }

                HStack(spacing: 8) {
                    MiniActionButton(title: "Palette", systemImage: "magnifyingglass") {
                        appViewModel.presentCommandPalette()
                    }
                    MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                        Task { await viewModel.refresh() }
                    }
                    MiniActionButton(title: "New", systemImage: "square.and.pencil") {
                        viewModel.newThread()
                    }
                }
            }
        }
        .padding(22)
        .background(
            LinearGradient(
                colors: [
                    tokens.panelBackground.opacity(0.96),
                    tokens.elevatedBackground.opacity(0.92),
                    tokens.accent.opacity(0.10),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            ),
            in: RoundedRectangle(cornerRadius: 24, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
    }
}

private struct ConversationMetricBadge: View {
    let title: String
    let value: String
    let accent: Color
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 3) {
            Text(title)
                .font(.caption2)
                .foregroundStyle(tokens.tertiaryText)
            HStack(spacing: 6) {
                Circle()
                    .fill(accent.opacity(0.92))
                    .frame(width: 6, height: 6)
                Text(value)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(tokens.primaryText)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(tokens.windowBackground.opacity(0.4), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

private struct MiniActionButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            Label(title, systemImage: systemImage)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
                .padding(.horizontal, 12)
                .padding(.vertical, 9)
                .background(tokens.windowBackground.opacity(0.38), in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct ComposerDock: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 14) {
            if !viewModel.state.conversation.suggestedPrompts.isEmpty {
                SuggestionPromptStrip(prompts: viewModel.state.conversation.suggestedPrompts) { prompt in
                    viewModel.applySuggestedPrompt(prompt)
                }
            }

            TextEditor(text: $viewModel.composerText)
                .font(.system(size: 14))
                .frame(minHeight: 108)
                .scrollContentBackground(.hidden)
                .padding(14)
                .background(tokens.windowBackground.opacity(0.42), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .stroke(tokens.border.opacity(0.72), lineWidth: 1)
                )

            HStack(alignment: .center, spacing: 10) {
                HStack(spacing: 8) {
                    ConversationMetricBadge(
                        title: "Profile",
                        value: appViewModel.currentProfileLabel,
                        accent: tokens.accent
                    )
                    ConversationMetricBadge(
                        title: "Model",
                        value: appViewModel.currentModelLabel,
                        accent: tokens.success
                    )
                }

                Spacer()

                Text("Cmd+Enter to send")
                    .font(.caption)
                    .foregroundStyle(tokens.tertiaryText)

                Button("Clear Thread", role: .destructive) {
                    Task { await viewModel.clearSelectedThread() }
                }
                .buttonStyle(.bordered)
                .disabled(!viewModel.canClearSelectedThread)

                Button(viewModel.state.conversation.isSending ? "Sending…" : "Send") {
                    Task { await viewModel.sendCurrentPrompt() }
                }
                .buttonStyle(.borderedProminent)
                .disabled(viewModel.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || viewModel.state.conversation.isSending)
            }
        }
        .padding(18)
        .background(tokens.panelBackground.opacity(0.96), in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(tokens.border.opacity(0.74), lineWidth: 1)
        )
        .shadow(color: Color.black.opacity(colorScheme == .dark ? 0.18 : 0.08), radius: 18, y: 8)
    }
}

private struct SuggestionPromptStrip: View {
    let prompts: [String]
    let onSelect: (String) -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 10) {
                ForEach(prompts, id: \.self) { prompt in
                    Button {
                        onSelect(prompt)
                    } label: {
                        Text(prompt)
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(tokens.primaryText)
                            .lineLimit(1)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 9)
                            .background(tokens.windowBackground.opacity(0.4), in: Capsule())
                            .overlay(
                                Capsule()
                                    .stroke(tokens.border.opacity(0.62), lineWidth: 1)
                            )
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

private struct EmptyConversationView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 18) {
                Text("Start from the workspace, not from a blank chat box.")
                    .font(.system(size: 28, weight: .bold, design: .rounded))
                    .foregroundStyle(tokens.primaryText)
                    .fixedSize(horizontal: false, vertical: true)

                Text("Ask Mosaic to inspect the project, audit runtime health, or turn the next change into an execution plan.")
                    .font(.system(size: 14))
                    .foregroundStyle(tokens.secondaryText)

                HStack(spacing: 10) {
                    ConversationMetricBadge(
                        title: "Workspace",
                        value: viewModel.state.sidebar.currentWorkspace.name,
                        accent: tokens.accent
                    )
                    ConversationMetricBadge(
                        title: "Threads",
                        value: "\(viewModel.threadCount)",
                        accent: tokens.warning
                    )
                    ConversationMetricBadge(
                        title: "Health",
                        value: appViewModel.currentHealthLabel,
                        accent: tokens.success
                    )
                }

                HStack(spacing: 10) {
                    Button("Switch Workspace") {
                        appViewModel.showSetupHub()
                    }
                    .buttonStyle(.bordered)

                    Button("Reveal in Finder") {
                        appViewModel.revealSelectedWorkspaceInFinder()
                    }
                    .buttonStyle(.bordered)

                    Button("Refresh Workspace") {
                        Task { await viewModel.refresh() }
                    }
                    .buttonStyle(.bordered)
                }
            }
            .frame(maxWidth: 460, alignment: .leading)

            VStack(alignment: .leading, spacing: 12) {
                Text("Suggested prompts")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(tokens.primaryText)
                FlowPromptGrid(prompts: viewModel.state.conversation.suggestedPrompts) { prompt in
                    viewModel.applySuggestedPrompt(prompt)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(26)
        .background(
            LinearGradient(
                colors: [
                    tokens.panelBackground.opacity(0.94),
                    tokens.elevatedBackground.opacity(0.90),
                    tokens.accent.opacity(0.08),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            ),
            in: RoundedRectangle(cornerRadius: 24, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct FlowPromptGrid: View {
    let prompts: [String]
    let onSelect: (String) -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), spacing: 10)], spacing: 10) {
            ForEach(prompts, id: \.self) { prompt in
                Button {
                    onSelect(prompt)
                } label: {
                    Text(prompt)
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.primaryText)
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(tokens.border.opacity(0.6), lineWidth: 1)
                        )
                }
                .buttonStyle(.plain)
            }
        }
    }
}

private struct ModelChoiceStrip: View {
    let selectedModel: String
    let options: [String]
    let onSelect: (String) -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(options, id: \.self) { option in
                    Button(option) {
                        onSelect(option)
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(option == selectedModel ? tokens.primaryText : tokens.secondaryText)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(
                        (option == selectedModel ? tokens.elevatedBackground : tokens.panelBackground),
                        in: Capsule()
                    )
                }
            }
        }
    }
}

struct InspectorContent: View {
    private enum InspectorPane: String, CaseIterable, Identifiable {
        case overview
        case runtime
        case session

        var id: String { rawValue }

        var title: String {
            switch self {
            case .overview: "Overview"
            case .runtime: "Runtime"
            case .session: "Session"
            }
        }

        var systemImage: String {
            switch self {
            case .overview: "square.grid.2x2"
            case .runtime: "slider.horizontal.3"
            case .session: "text.bubble"
            }
        }
    }

    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var selectedPane: InspectorPane = .overview

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                InspectorHeroCard(appViewModel: appViewModel, viewModel: viewModel)

                HStack(spacing: 8) {
                    ForEach(InspectorPane.allCases) { pane in
                        Button {
                            selectedPane = pane
                        } label: {
                            Label(pane.title, systemImage: pane.systemImage)
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(selectedPane == pane ? tokens.primaryText : tokens.secondaryText)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 9)
                                .background(
                                    (selectedPane == pane ? tokens.elevatedBackground : tokens.panelBackground.opacity(0.65)),
                                    in: Capsule()
                                )
                                .overlay(
                                    Capsule()
                                        .stroke(selectedPane == pane ? tokens.accent.opacity(0.4) : tokens.border.opacity(0.45), lineWidth: 1)
                                )
                        }
                        .buttonStyle(.plain)
                    }
                }

                switch selectedPane {
                case .overview:
                    InspectorWorkspaceActions(appViewModel: appViewModel, viewModel: viewModel)
                    inspectorSections(for: ["context", "runtime"])
                    InspectorHealthChecksCard(appViewModel: appViewModel)
                case .runtime:
                    RuntimeControlsCard(viewModel: appViewModel, title: "Runtime controls")
                    inspectorSections(for: ["runtime", "context"])
                    InspectorHealthChecksCard(appViewModel: appViewModel)
                case .session:
                    InspectorSessionActions(viewModel: viewModel)
                    inspectorSections(for: ["session", "context"])
                }
            }
            .padding(16)
        }
    }

    @ViewBuilder
    private func inspectorSections(for ids: [String]) -> some View {
        let filtered = viewModel.state.inspector.sections.filter { ids.contains($0.id) }
        ForEach(filtered) { section in
            InspectorSectionCard(section: section)
        }
    }
}

private struct InspectorHeroCard: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Inspector")
                        .font(.system(size: 18, weight: .bold, design: .rounded))
                        .foregroundStyle(tokens.primaryText)
                    Text("Runtime, workspace, and session detail for the active thread.")
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
            }

            HStack(spacing: 10) {
                ConversationMetricBadge(
                    title: "Provider",
                    value: appViewModel.currentProviderLabel,
                    accent: tokens.accent
                )
                ConversationMetricBadge(
                    title: "Model",
                    value: appViewModel.currentModelLabel,
                    accent: tokens.success
                )
                ConversationMetricBadge(
                    title: "Health",
                    value: appViewModel.currentHealthLabel,
                    accent: tokens.warning
                )
            }
        }
        .padding(18)
        .background(
            LinearGradient(
                colors: [
                    tokens.panelBackground.opacity(0.94),
                    tokens.elevatedBackground.opacity(0.90),
                    tokens.accent.opacity(0.08),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            ),
            in: RoundedRectangle(cornerRadius: 22, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
    }
}

private struct InspectorWorkspaceActions: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            Text("Workspace Actions")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            HStack(spacing: 10) {
                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await viewModel.refresh() }
                }
                MiniActionButton(title: "Reveal", systemImage: "folder.badge.gearshape") {
                    appViewModel.revealSelectedWorkspaceInFinder()
                }
                MiniActionButton(title: "Setup", systemImage: "slider.horizontal.3") {
                    appViewModel.showSetupHub()
                }
            }
        }
        .padding(14)
        .background(tokens.panelBackground.opacity(0.86), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
    }
}

private struct InspectorSessionActions: View {
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            Text("Session Actions")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            HStack(spacing: 10) {
                MiniActionButton(title: "New", systemImage: "square.and.pencil") {
                    viewModel.newThread()
                }
                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await viewModel.refresh() }
                }
                Button("Clear Thread", role: .destructive) {
                    Task { await viewModel.clearSelectedThread() }
                }
                .buttonStyle(.bordered)
                .disabled(!viewModel.canClearSelectedThread)
            }
        }
        .padding(14)
        .background(tokens.panelBackground.opacity(0.86), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
    }
}

private struct InspectorHealthChecksCard: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            Text("Health Checks")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            if let checks = appViewModel.selectedWorkspaceHealth?.checks, !checks.isEmpty {
                ForEach(Array(checks.enumerated()), id: \.offset) { index, check in
                    HStack(alignment: .top, spacing: 10) {
                        Circle()
                            .fill(color(for: check.status, tokens: tokens))
                            .frame(width: 8, height: 8)
                            .padding(.top, 5)

                        VStack(alignment: .leading, spacing: 3) {
                            Text(check.name.replacingOccurrences(of: "_", with: " ").capitalized)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                            Text(check.detail)
                                .font(.system(size: 12))
                                .foregroundStyle(tokens.secondaryText)
                                .fixedSize(horizontal: false, vertical: true)
                        }

                        Spacer()

                        Text(check.status.capitalized)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(color(for: check.status, tokens: tokens))
                    }

                    if index != checks.count - 1 {
                        Divider()
                            .overlay(tokens.border.opacity(0.45))
                    }
                }
            } else {
                Text("No health checks loaded yet.")
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
            }
        }
        .padding(14)
        .background(tokens.panelBackground.opacity(0.86), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
    }

    private func color(for status: String, tokens: ThemeTokens) -> Color {
        switch status.lowercased() {
        case "fail":
            tokens.failure
        case "warn":
            tokens.warning
        default:
            tokens.success
        }
    }
}

private struct InspectorSectionCard: View {
    let section: InspectorSectionState
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

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
        .background(tokens.panelBackground.opacity(0.86), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
    }
}

private struct RuntimeControlsCard: View {
    @Bindable var viewModel: AppViewModel
    let title: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            Text("Keep Azure or local provider values editable from the app so you can recover quickly without dropping back to the CLI.")
                .font(.system(size: 12))
                .foregroundStyle(tokens.secondaryText)
                .fixedSize(horizontal: false, vertical: true)

            SetupValueRow(label: "Provider", value: viewModel.runtimeDraftProviderLabel)

            ProviderPresetPicker(
                azureSelected: viewModel.runtimeDraftUsesAzurePreset,
                azureAction: viewModel.applyAzureRuntimePreset,
                localAction: viewModel.applyLocalRuntimePreset
            )

            if viewModel.runtimeDraftUsesAzurePreset {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Azure resource")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    TextField(
                        "my-resource",
                        text: Binding(
                            get: { viewModel.runtimeDraftAzureResourceName },
                            set: { viewModel.runtimeDraftAzureResourceName = $0 }
                        )
                    )
                    .textFieldStyle(.roundedBorder)

                    SetupValueRow(label: "Resolved URL", value: viewModel.runtimeDraftBaseURL)
                }
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Base URL")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    TextField("Base URL", text: $viewModel.runtimeDraftBaseURL)
                        .textFieldStyle(.roundedBorder)
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                Text(viewModel.runtimeDraftUsesAzurePreset ? "Model / deployment" : "Model")
                    .font(.caption)
                    .foregroundStyle(tokens.tertiaryText)
                ModelChoiceStrip(
                    selectedModel: viewModel.runtimeDraftModel,
                    options: viewModel.setupModelChoices,
                    onSelect: viewModel.selectRuntimeModel
                )
                TextField("Model", text: $viewModel.runtimeDraftModel)
                    .textFieldStyle(.roundedBorder)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("API Key Env")
                    .font(.caption)
                    .foregroundStyle(tokens.tertiaryText)
                TextField("API Key Env", text: $viewModel.runtimeDraftAPIKeyEnv)
                    .textFieldStyle(.roundedBorder)
            }

            if viewModel.runtimeDraftRequiresAzureResourceHost {
                SetupHintRow(
                    systemImage: "exclamationmark.triangle.fill",
                    text: "Replace `YOUR_RESOURCE_NAME` before saving an Azure runtime preset."
                )
            }

            HStack(spacing: 10) {
                Button(viewModel.isSavingRuntimeSettings ? "Saving…" : "Save Runtime") {
                    Task { await viewModel.saveRuntimeSettings() }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!viewModel.canSaveRuntimeSettings)

                Button("Reset") {
                    viewModel.resetRuntimeDraft()
                }
                .buttonStyle(.bordered)
                .disabled(!viewModel.runtimeDraftHasChanges || viewModel.isSavingRuntimeSettings)
            }
        }
        .padding(14)
        .background(tokens.panelBackground.opacity(0.86), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border.opacity(0.7), lineWidth: 1)
        )
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

        HStack(spacing: 10) {
            Circle()
                .fill(toneColor)
                .frame(width: 8, height: 8)
            Text(status.title)
                .font(.system(size: 12, weight: .semibold))
            Text(status.detail)
                .font(.system(size: 12))
                .foregroundStyle(tokens.secondaryText)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(tokens.windowBackground.opacity(0.38), in: Capsule())
        .overlay(
            Capsule()
                .stroke(tokens.border.opacity(0.6), lineWidth: 1)
        )
    }
}

struct ConversationTimelineRow: View {
    let message: ConversationMessage
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let isUser = message.role == .user
        let isSystem = message.role == .system
        let bubbleBackground: Color = switch message.role {
        case .assistant:
            tokens.panelBackground.opacity(0.88)
        case .user:
            tokens.elevatedBackground.opacity(0.96)
        case .system:
            tokens.windowBackground.opacity(0.82)
        }

        HStack(alignment: .top, spacing: 14) {
            if !isUser {
                ConversationRoleGlyph(role: message.role)
            } else {
                Spacer(minLength: 80)
            }

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Text(message.role.title)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(message.timestampLabel)
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                }

                Text(message.body)
                    .font(.system(size: isSystem ? 14 : 15))
                    .foregroundStyle(tokens.primaryText)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(18)
            .background(
                bubbleBackground,
                in: RoundedRectangle(cornerRadius: 22, style: .continuous)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .stroke(
                        isUser ? tokens.accent.opacity(0.30) : tokens.border.opacity(0.60),
                        lineWidth: 1
                    )
            )
            .frame(maxWidth: isUser ? 700 : 860, alignment: .leading)

            if isUser {
                ConversationRoleGlyph(role: message.role)
            } else {
                Spacer(minLength: 80)
            }
        }
        .frame(maxWidth: .infinity, alignment: isUser ? .trailing : .leading)
        .id(message.id)
    }
}

private struct ConversationRoleGlyph: View {
    let role: ConversationMessage.Role
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let accent: Color = switch role {
        case .assistant: tokens.accent
        case .user: tokens.success
        case .system: tokens.warning
        }
        let symbol: String = switch role {
        case .assistant: "sparkles"
        case .user: "person.fill"
        case .system: "waveform.path.ecg"
        }

        ZStack {
            Circle()
                .fill(accent.opacity(0.16))
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(accent)
        }
        .frame(width: 34, height: 34)
    }
}
