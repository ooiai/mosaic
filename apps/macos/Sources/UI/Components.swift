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
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            HStack(alignment: .top, spacing: 0) {
                SetupSidebarColumn(viewModel: viewModel)
                    .frame(width: 280)

                Rectangle()
                    .fill(tokens.border.opacity(0.75))
                    .frame(width: 1)
                    .padding(.vertical, 10)

                VStack(alignment: .leading, spacing: 20) {
                    SetupHeroDeck(viewModel: viewModel)
                    SetupWorkspaceColumn(viewModel: viewModel)
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)
                .padding(.horizontal, 24)

                Rectangle()
                    .fill(tokens.border.opacity(0.75))
                    .frame(width: 1)
                    .padding(.vertical, 10)

                SetupActionColumn(viewModel: viewModel)
                    .frame(width: 360)
            }
            .padding(20)
            .frame(maxWidth: 1180, alignment: .leading)
        }
        .scrollIndicators(.hidden)
    }
}

private struct SetupSidebarColumn: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                Text("mosaic")
                    .font(.system(size: 11, weight: .bold, design: .monospaced))
                    .foregroundStyle(tokens.accent)
                Text("Setup")
                    .font(.system(size: 24, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text("Project-first runtime bootstrap for the native workbench.")
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }

            VStack(spacing: 8) {
                setupSidebarAction(
                    title: "Choose workspace",
                    subtitle: "Pick the local project root.",
                    systemImage: "folder"
                ) {
                    let panel = NSOpenPanel()
                    panel.canChooseDirectories = true
                    panel.canChooseFiles = false
                    panel.allowsMultipleSelection = false
                    panel.prompt = "Use Workspace"
                    if panel.runModal() == .OK, let url = panel.url {
                        Task { await viewModel.registerWorkspace(url: url) }
                    }
                }

                setupSidebarAction(
                    title: "Refresh setup",
                    subtitle: "Reload workspace and runtime status.",
                    systemImage: "arrow.clockwise"
                ) {
                    Task { await viewModel.refreshSelectedWorkspace() }
                }

                setupSidebarAction(
                    title: "Open settings",
                    subtitle: "Tune desktop and runtime defaults.",
                    systemImage: "gearshape"
                ) {
                    Task { await viewModel.recordCommandAction("settings") }
                    openSettings()
                }
            }

            Rectangle()
                .fill(tokens.border.opacity(0.78))
                .frame(height: 1)

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Recent Projects")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Text("\(viewModel.recentWorkspaces.count)")
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                }

                if viewModel.recentWorkspaces.isEmpty {
                    Text("No recent workspaces yet.")
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                } else {
                    ForEach(viewModel.recentWorkspaces.prefix(8)) { workspace in
                        Button {
                            Task { await viewModel.previewWorkspace(workspace) }
                        } label: {
                            HStack(spacing: 10) {
                                Image(systemName: "folder")
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(tokens.warning)
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(workspace.name)
                                        .font(.system(size: 12, weight: .semibold))
                                        .foregroundStyle(tokens.primaryText)
                                        .lineLimit(1)
                                    Text(workspace.path)
                                        .font(.system(size: 11))
                                        .foregroundStyle(tokens.tertiaryText)
                                        .lineLimit(1)
                                }
                                Spacer()
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 9)
                            .background(tokens.panelBackground.opacity(0.34), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Spacer(minLength: 0)

            StatusHeroView(
                title: viewModel.setupStatusTitle,
                detail: viewModel.setupStatusDetail,
                tone: viewModel.setupStatusTone
            )
        }
        .padding(.trailing, 18)
    }

    private func setupSidebarAction(
        title: String,
        subtitle: String,
        systemImage: String,
        action: @escaping () -> Void
    ) -> some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        return Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: systemImage)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(tokens.accent)
                    .frame(width: 18)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                    Text(subtitle)
                        .font(.system(size: 11))
                        .foregroundStyle(tokens.tertiaryText)
                }
                Spacer()
                Image(systemName: "arrow.up.right")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(tokens.tertiaryText)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 9)
            .background(tokens.panelBackground.opacity(0.22), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
        .buttonStyle(.plain)
    }
}

private struct SetupBackdrop: View {
    let tokens: ThemeTokens

    var body: some View {
        ZStack {
            tokens.windowBackground

            LinearGradient(
                colors: [
                    tokens.windowBackground,
                    tokens.elevatedBackground.opacity(0.4),
                    tokens.windowBackground,
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
    }
}

private struct SetupHeroDeck: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 20) {
                VStack(alignment: .leading, spacing: 12) {
                    Text("WORKSPACE SETUP")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.accent)

                    Text("Connect the project and bring the runtime online.")
                        .font(.system(size: 30, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                        .fixedSize(horizontal: false, vertical: true)

                    Text("Start from the repo, confirm the runtime, then open the workbench. Azure OpenAI stays the default path, but the screen now behaves more like a Codex project launcher than a raw configuration form.")
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
                    Text("Current bootstrap")
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
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
                .background(tokens.panelBackground.opacity(0.44), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            }
        }
        .padding(.horizontal, 4)
        .padding(.vertical, 6)
        .overlay(
            Rectangle()
                .fill(tokens.border.opacity(0.75))
                .frame(height: 1),
            alignment: .bottom
        )
    }
}

private struct SetupWorkspaceColumn: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            Text("Workspace")
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)

            Text("Select the project root and confirm the local context before runtime initialization.")
                .font(.system(size: 13))
                .foregroundStyle(tokens.secondaryText)

            VStack(alignment: .leading, spacing: 12) {
                if let workspace = viewModel.selectedWorkspace {
                    HStack(alignment: .top, spacing: 12) {
                        Image(systemName: "folder.fill")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(tokens.warning)
                            .frame(width: 28, height: 28)
                            .background(tokens.panelBackground.opacity(0.4), in: RoundedRectangle(cornerRadius: 10, style: .continuous))

                        VStack(alignment: .leading, spacing: 6) {
                            Text(workspace.name)
                                .font(.system(size: 22, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                            Text(workspace.path)
                                .font(.system(size: 12))
                                .foregroundStyle(tokens.secondaryText)
                                .textSelection(.enabled)
                        }
                    }
                } else {
                    Text("No workspace selected")
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text("Choose a local project folder first. Mosaic keeps runtime state, sessions, and configuration anchored to that directory.")
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

            Rectangle()
                .fill(tokens.border.opacity(0.75))
                .frame(height: 1)

            VStack(alignment: .leading, spacing: 10) {
                Text("Anchored to this project")
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)

                SetupChecklistItem(text: "The existing project directory as your workspace root")
                SetupChecklistItem(text: "A project-local Mosaic config once you initialize")
                SetupChecklistItem(text: "Session history and runtime health scoped to this workspace")
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tokens.panelBackground.opacity(0.56), in: RoundedRectangle(cornerRadius: 20, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .stroke(tokens.border.opacity(0.66), lineWidth: 1)
        )
    }
}

private struct SetupActionColumn: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            Text("Runtime")
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)

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
                    Text("Azure OpenAI is the default initialization path. Pick a workspace, replace the Azure resource host, then initialize the project-local Mosaic config.")
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

            Rectangle()
                .fill(tokens.border.opacity(0.75))
                .frame(height: 1)

            VStack(alignment: .leading, spacing: 8) {
                Text("Why this flow")
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Text("The app leads with project context, but the runtime defaults now bias toward Azure OpenAI or a local OpenAI-compatible server instead of a generic cloud preset.")
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.leading, 24)
    }
}

private struct RecentWorkspaceSection: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 18) {
            Text("Workspace")
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.primaryText)
        }
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
                .frame(width: 8, height: 8)
                .padding(.top, 5)
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(detail)
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
        }
        .padding(14)
        .background(tokens.panelBackground.opacity(0.36), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(tokens.border.opacity(0.58), lineWidth: 1)
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
                VStack(alignment: .leading, spacing: 14) {
                    HStack(alignment: .top) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("COMMAND PALETTE")
                                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                .foregroundStyle(tokens.accent)
                            Text("Search commands, sessions, workspaces, and runtime actions.")
                                .font(.system(size: 13))
                                .foregroundStyle(tokens.secondaryText)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 6) {
                            Text("\(filteredItems.count) RESULTS")
                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                .foregroundStyle(tokens.tertiaryText)
                            HStack(spacing: 6) {
                                CommandKeycap(label: "↑↓")
                                CommandKeycap(label: "↩")
                                CommandKeycap(label: "Esc")
                            }
                        }
                    }

                    HStack(spacing: 12) {
                        Image(systemName: "magnifyingglass")
                            .foregroundStyle(tokens.tertiaryText)

                        TextField("Search commands, sessions, and workspaces", text: $query)
                            .textFieldStyle(.plain)
                            .font(.system(size: 16))
                            .focused($isSearchFocused)
                            .onSubmit {
                                executeHighlightedItem(in: filteredItems)
                            }

                        if !query.isEmpty {
                            Button {
                                query = ""
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .foregroundStyle(tokens.tertiaryText)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                    .background(tokens.panelBackground.opacity(0.62), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .stroke(tokens.border.opacity(0.6), lineWidth: 1)
                    )
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
                                        HStack {
                                            Text(section.title.uppercased())
                                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                                .foregroundStyle(tokens.tertiaryText)
                                            Spacer()
                                            Text("\(section.items.count)")
                                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                                .foregroundStyle(tokens.tertiaryText)
                                        }
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

                for thread in workbench.state.sidebar.threads {
                    let isPinned = workbench.isPinnedThread(thread.id)
                    items.append(
                        CommandPaletteItem(
                            id: "session-open-\(thread.id)",
                            title: thread.title,
                            subtitle: "\(thread.eventCount) events · \(thread.updatedLabel)\(isPinned ? " · pinned" : "")",
                            systemImage: isPinned ? "pin.fill" : "text.bubble",
                            shortcutLabel: nil,
                            enabled: true,
                            group: .conversation,
                            keywords: ["session", "thread", thread.id, thread.title, thread.subtitle, isPinned ? "pinned" : "recent"]
                        ) {
                            viewModel.dismissCommandPalette()
                            Task { await viewModel.openSession(thread.id) }
                        }
                    )

                    items.append(
                        CommandPaletteItem(
                            id: "session-pin-\(thread.id)",
                            title: isPinned ? "Unpin \(thread.title)" : "Pin \(thread.title)",
                            subtitle: isPinned ? "Remove this session from the pinned group." : "Keep this session at the top of the sidebar.",
                            systemImage: isPinned ? "pin.slash" : "pin",
                            shortcutLabel: nil,
                            enabled: true,
                            group: .conversation,
                            keywords: ["pin", "session", "thread", thread.id, thread.title]
                        ) {
                            viewModel.dismissCommandPalette()
                            Task { await viewModel.togglePinnedSession(thread.id) }
                        }
                    )

                    items.append(
                        CommandPaletteItem(
                            id: "session-clear-\(thread.id)",
                            title: "Clear \(thread.title)",
                            subtitle: "Delete this session from the current workspace.",
                            systemImage: "trash",
                            shortcutLabel: nil,
                            enabled: !workbench.state.conversation.isSending,
                            group: .conversation,
                            keywords: ["clear", "delete", "session", "thread", thread.id, thread.title]
                        ) {
                            viewModel.dismissCommandPalette()
                            Task { await viewModel.clearSession(thread.id) }
                        }
                    )
                }
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
                    Task { await viewModel.activateWorkspace(workspace) }
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
            RoundedRectangle(cornerRadius: 999, style: .continuous)
                .fill(isHighlighted ? tokens.accent : tokens.border.opacity(0.42))
                .frame(width: 3)

            HStack(alignment: .top, spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .fill(tokens.elevatedBackground.opacity(0.86))
                    Image(systemName: item.systemImage)
                        .frame(width: 18, height: 18)
                        .foregroundStyle(tokens.accent)
                }
                .frame(width: 28, height: 28)
                .padding(.top, 2)

                VStack(alignment: .leading, spacing: 5) {
                    HStack(spacing: 8) {
                        Text(item.title)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                        Text(item.group.rawValue.uppercased())
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                    }

                    Text(item.subtitle)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }

                Spacer(minLength: 12)

                VStack(alignment: .trailing, spacing: 8) {
                    if let shortcutLabel = item.shortcutLabel {
                        Text(shortcutLabel)
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(tokens.panelBackground, in: Capsule())
                    }

                    Image(systemName: "arrow.up.right")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(tokens.tertiaryText)
                }
            }
        }
        .padding(14)
        .background(
            (isHighlighted ? tokens.elevatedBackground : tokens.panelBackground.opacity(0.72)),
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
                    tokens.windowBackground.opacity(0.985),
                    tokens.elevatedBackground.opacity(colorScheme == .dark ? 0.58 : 0.22),
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        )
        .toolbar {
            ToolbarItem(placement: .principal) {
                WorkbenchToolbarPrincipal(appViewModel: appViewModel, viewModel: viewModel)
            }

            ToolbarItemGroup {
                ToolbarModelMenu(appViewModel: appViewModel)

                Button {
                    appViewModel.presentCommandPalette()
                } label: {
                    ToolbarIconButton(systemImage: "magnifyingglass")
                }
                .help("Command palette")

                Button {
                    appViewModel.createNewThread()
                } label: {
                    ToolbarIconButton(systemImage: "square.and.pencil", accentStyle: .accent)
                }
                .help("New thread")

                Button {
                    appViewModel.showSetupHub()
                } label: {
                    ToolbarIconButton(systemImage: "folder")
                }
                .help("Switch workspace")

                Button {
                    Task { await appViewModel.refreshActiveWorkspace() }
                } label: {
                    ToolbarIconButton(systemImage: "arrow.clockwise")
                }
                .help("Refresh workspace")

                Button {
                    Task { await appViewModel.clearCurrentThread() }
                } label: {
                    ToolbarIconButton(systemImage: "trash", accentStyle: .destructive)
                }
                .disabled(!viewModel.canClearSelectedThread)
                .help("Clear selected thread")

                Button {
                    appViewModel.toggleInspector()
                } label: {
                    ToolbarIconButton(
                        systemImage: viewModel.isInspectorVisible ? "sidebar.right" : "sidebar.left",
                        accentStyle: viewModel.isInspectorVisible ? .accent : .neutral
                    )
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

    private var healthAccent: Color {
        switch appViewModel.currentHealthLabel {
        case "Degraded":
            ThemeTokens.current(for: colorScheme).failure
        case "Needs attention":
            ThemeTokens.current(for: colorScheme).warning
        default:
            ThemeTokens.current(for: colorScheme).success
        }
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(viewModel.state.sidebar.currentWorkspace.name.uppercased())
                    Text("•")
                    Text(appViewModel.currentProviderLabel.uppercased())
                    Text("•")
                    Text(appViewModel.currentModelLabel.uppercased())
                    if viewModel.isLoading {
                        Text("•")
                        Text("SYNCING")
                    }
                }
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
                .lineLimit(1)

                Text(viewModel.selectedThreadSummary?.title ?? viewModel.state.conversation.threadTitle)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)
            }

            Spacer(minLength: 10)

            ToolbarWorkspaceMenu(appViewModel: appViewModel, viewModel: viewModel)
            ToolbarSessionMenu(appViewModel: appViewModel, viewModel: viewModel)

            HStack(spacing: 8) {
                ToolbarStatusPill(
                    title: "Health",
                    value: appViewModel.currentHealthLabel,
                    systemImage: "waveform.path.ecg",
                    accent: healthAccent
                )
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
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .background(tokens.panelBackground.opacity(0.72), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(tokens.border.opacity(0.58), lineWidth: 1)
        )
    }
}

private struct ToolbarSelectorLabel: View {
    let title: String
    let value: String
    let systemImage: String
    let accent: Color
    var trailingBadge: String? = nil
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 8) {
            ZStack {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(tokens.elevatedBackground.opacity(0.86))
                Image(systemName: systemImage)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(accent)
            }
            .frame(width: 24, height: 24)

            VStack(alignment: .leading, spacing: 2) {
                Text(title.uppercased())
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Text(value)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)
            }

            if let trailingBadge, !trailingBadge.isEmpty {
                Text(trailingBadge.uppercased())
                    .font(.system(size: 9, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 4)
                    .background(tokens.elevatedBackground.opacity(0.76), in: Capsule())
            }

            Image(systemName: "chevron.down")
                .font(.caption2)
                .foregroundStyle(tokens.tertiaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(tokens.panelBackground.opacity(0.74), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(tokens.border.opacity(0.55), lineWidth: 1)
        )
    }
}

private struct ToolbarIconButton: View {
    enum AccentStyle {
        case neutral
        case accent
        case destructive
    }

    let systemImage: String
    var accentStyle: AccentStyle = .neutral
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let foreground: Color = switch accentStyle {
        case .neutral: tokens.primaryText
        case .accent: tokens.accent
        case .destructive: tokens.failure
        }

        Image(systemName: systemImage)
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(foreground)
            .frame(width: 28, height: 28)
            .background(tokens.panelBackground.opacity(0.78), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .stroke(tokens.border.opacity(0.52), lineWidth: 1)
            )
    }
}

private struct ToolbarWorkspaceMenu: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Menu {
            Button("Reveal in Finder") {
                appViewModel.revealSelectedWorkspaceInFinder()
            }

            Button("Choose Workspace") {
                appViewModel.showSetupHub()
            }

            if !viewModel.state.sidebar.recentWorkspaces.isEmpty {
                Divider()

                ForEach(viewModel.state.sidebar.recentWorkspaces) { workspace in
                    Button {
                        Task { await appViewModel.activateWorkspace(workspace, recordHistory: true) }
                    } label: {
                        Label(workspace.name, systemImage: "folder")
                    }
                }
            }
        } label: {
            ToolbarSelectorLabel(
                title: "Workspace",
                value: viewModel.state.sidebar.currentWorkspace.name,
                systemImage: "folder",
                accent: tokens.accent,
                trailingBadge: viewModel.isLoading ? "sync" : nil
            )
        }
        .menuStyle(.borderlessButton)
    }
}

private struct ToolbarSessionMenu: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let selectedThread = viewModel.selectedThreadSummary

        Menu {
            Button("New Thread") {
                appViewModel.createNewThread()
            }

            if !viewModel.state.sidebar.threads.isEmpty {
                Divider()

                ForEach(viewModel.state.sidebar.threads) { thread in
                    Button {
                        Task { await appViewModel.openSession(thread.id, recordHistory: true) }
                    } label: {
                        if viewModel.isPinnedThread(thread.id) {
                            Label(thread.title, systemImage: "pin.fill")
                        } else {
                            Label(thread.title, systemImage: "text.bubble")
                        }
                    }
                }
            }
        } label: {
            ToolbarSelectorLabel(
                title: "Session",
                value: selectedThread?.title ?? "New thread",
                systemImage: viewModel.state.conversation.sessionID == nil ? "plus.bubble" : "text.bubble",
                accent: tokens.success,
                trailingBadge: viewModel.state.conversation.sessionID.flatMap { viewModel.isPinnedThread($0) ? "pin" : nil }
            )
        }
        .menuStyle(.borderlessButton)
    }
}

private struct ToolbarStatusPill: View {
    let title: String
    let value: String
    let systemImage: String
    let accent: Color
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 7) {
            Image(systemName: systemImage)
                .font(.caption.weight(.semibold))
                .foregroundStyle(accent)
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Text(value)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(tokens.elevatedBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 11, style: .continuous))
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
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            Text(value)
                .font(.caption.weight(.semibold))
                .foregroundStyle(tokens.primaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(tokens.elevatedBackground.opacity(0.82), in: Capsule())
    }
}

private struct ToolbarModelMenu: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Menu {
            if appViewModel.setupModelChoices.isEmpty {
                Text("No models available")
            } else {
                ForEach(appViewModel.setupModelChoices, id: \.self) { modelID in
                    Button {
                        Task { await appViewModel.quickSwitchModel(modelID) }
                    } label: {
                        if modelID == appViewModel.currentModelLabel {
                            Label(modelID, systemImage: "checkmark")
                        } else {
                            Text(modelID)
                        }
                    }
                }
            }
        } label: {
            ToolbarSelectorLabel(
                title: "Model",
                value: appViewModel.isApplyingQuickModel ? "Switching…" : appViewModel.currentModelLabel,
                systemImage: "cpu",
                accent: tokens.success,
                trailingBadge: "\(appViewModel.setupModelChoices.count)"
            )
        }
        .menuStyle(.borderlessButton)
        .disabled(appViewModel.setupModelChoices.isEmpty || !appViewModel.canQuickSwitchModels)
    }
}

struct SidebarContent: View {
    private struct RecentTaskDescriptor: Identifiable {
        let id: String
        let title: String
        let subtitle: String
        let systemImage: String
        let perform: () -> Void
    }

    private struct RecentTaskCard: View {
        let task: RecentTaskDescriptor
        @Environment(\.colorScheme) private var colorScheme

        var body: some View {
            let tokens = ThemeTokens.current(for: colorScheme)

            Button(action: task.perform) {
                HStack(alignment: .top, spacing: 12) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(tokens.elevatedBackground.opacity(0.92))
                        Image(systemName: task.systemImage)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(tokens.accent)
                    }
                    .frame(width: 34, height: 34)

                    VStack(alignment: .leading, spacing: 4) {
                        Text(task.title)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                            .lineLimit(1)
                        Text(task.subtitle)
                            .font(.caption)
                            .foregroundStyle(tokens.secondaryText)
                            .lineLimit(2)
                    }

                    Spacer(minLength: 0)

                    Image(systemName: "arrow.up.right")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(tokens.tertiaryText)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(tokens.panelBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .stroke(tokens.border.opacity(0.62), lineWidth: 1)
                )
            }
            .buttonStyle(.plain)
        }
    }

    private struct SidebarPrimaryActionRow: View {
        let title: String
        let subtitle: String
        let systemImage: String
        let accent: Color
        let action: () -> Void
        @Environment(\.colorScheme) private var colorScheme

        var body: some View {
            let tokens = ThemeTokens.current(for: colorScheme)

            Button(action: action) {
                HStack(spacing: 12) {
                    Image(systemName: systemImage)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(accent)
                        .frame(width: 20, height: 20)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(title)
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(tokens.primaryText)
                        Text(subtitle)
                            .font(.system(size: 11))
                            .foregroundStyle(tokens.tertiaryText)
                            .lineLimit(1)
                    }

                    Spacer(minLength: 0)

                    Image(systemName: "arrow.up.right")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(tokens.tertiaryText)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(tokens.panelBackground.opacity(0.22), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            }
            .buttonStyle(.plain)
        }
    }

    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("mosaic")
                                .font(.system(size: 11, weight: .bold, design: .monospaced))
                                .foregroundStyle(tokens.accent)
                            Text(viewModel.state.sidebar.currentWorkspace.name)
                                .font(.system(size: 17, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                                .lineLimit(1)
                            HStack(spacing: 6) {
                                Text(appViewModel.currentProviderLabel.uppercased())
                                Text("•")
                                Text(appViewModel.currentModelLabel.uppercased())
                            }
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                        }
                        Spacer()
                        Button {
                            Task { await appViewModel.recordCommandAction("settings") }
                            openSettings()
                        } label: {
                            Image(systemName: "gearshape")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(tokens.tertiaryText)
                                .frame(width: 28, height: 28)
                                .background(tokens.panelBackground.opacity(0.3), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        }
                        .buttonStyle(.plain)
                    }

                    Text(viewModel.state.sidebar.currentWorkspace.path)
                        .font(.caption)
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(2)
                        .textSelection(.enabled)

                    Rectangle()
                        .fill(tokens.border.opacity(0.8))
                        .frame(height: 1)

                    VStack(spacing: 6) {
                        SidebarPrimaryActionRow(
                            title: "New thread",
                            subtitle: "Start a new conversation in this workspace.",
                            systemImage: "square.and.pencil",
                            accent: tokens.accent
                        ) {
                            appViewModel.createNewThread()
                        }

                        SidebarPrimaryActionRow(
                            title: "Command palette",
                            subtitle: "Search commands, sessions, and workspaces.",
                            systemImage: "magnifyingglass",
                            accent: tokens.success
                        ) {
                            appViewModel.presentCommandPalette()
                        }

                        SidebarPrimaryActionRow(
                            title: "Switch workspace",
                            subtitle: "Return to setup and switch projects.",
                            systemImage: "folder",
                            accent: tokens.warning
                        ) {
                            appViewModel.showSetupHub()
                        }

                        SidebarPrimaryActionRow(
                            title: "Refresh runtime",
                            subtitle: "Reload sessions, health, and status.",
                            systemImage: "arrow.clockwise",
                            accent: tokens.success
                        ) {
                            Task { await appViewModel.refreshActiveWorkspace() }
                        }

                        SidebarPrimaryActionRow(
                            title: "Reveal in Finder",
                            subtitle: "Open the current project folder.",
                            systemImage: "folder.badge.gearshape",
                            accent: tokens.secondaryText
                        ) {
                            appViewModel.revealSelectedWorkspaceInFinder()
                        }
                    }
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(tokens.panelBackground.opacity(0.68), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .stroke(tokens.border.opacity(0.52), lineWidth: 1)
                )

                if !viewModel.state.sidebar.recentWorkspaces.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Workspaces")
                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                        ForEach(viewModel.state.sidebar.recentWorkspaces) { workspace in
                            Button {
                                Task { await appViewModel.activateWorkspace(workspace, recordHistory: true) }
                            } label: {
                                HStack(spacing: 10) {
                                    Image(systemName: "folder")
                                        .font(.caption.weight(.semibold))
                                        .foregroundStyle(tokens.warning)
                                    VStack(alignment: .leading, spacing: 3) {
                                        Text(workspace.name)
                                            .font(.system(size: 12, weight: .semibold))
                                            .foregroundStyle(tokens.primaryText)
                                        Text(workspace.path)
                                            .font(.caption)
                                            .foregroundStyle(tokens.tertiaryText)
                                            .lineLimit(1)
                                    }
                                    Spacer(minLength: 0)
                                }
                                .padding(.horizontal, 10)
                                .padding(.vertical, 9)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(tokens.panelBackground.opacity(0.66), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }

                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Text("Sessions")
                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                        Spacer()
                        Text("\(viewModel.state.sidebar.threads.count)")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(tokens.tertiaryText)
                    }

                    TextField("Search sessions", text: $viewModel.threadFilter)
                        .textFieldStyle(.plain)
                        .font(.system(size: 13))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 10)
                        .background(tokens.panelBackground.opacity(0.68), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                        .overlay(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .stroke(tokens.border.opacity(0.56), lineWidth: 1)
                        )

                    if let selected = viewModel.selectedThreadSummary {
                        ActiveSessionCard(
                            thread: selected,
                            isPinned: viewModel.isPinnedThread(selected.id),
                            onOpen: { Task { await viewModel.selectThread(selected.id) } },
                            onTogglePinned: { Task { await viewModel.togglePinnedThread(selected.id) } },
                            onClear: { Task { await viewModel.clearThread(selected.id) } }
                        )
                    }

                    HStack(spacing: 10) {
                        Button("New Thread") {
                            appViewModel.createNewThread()
                        }
                        .buttonStyle(.borderless)
                        .foregroundStyle(tokens.accent)

                        if viewModel.hasThreadSearchQuery {
                            Text("Showing \(viewModel.filteredThreads.count) results")
                                .font(.caption)
                                .foregroundStyle(tokens.tertiaryText)
                        }
                    }

                    if viewModel.threadSections.isEmpty {
                        Text(viewModel.threadFilter.isEmpty ? "No sessions yet." : "No matching sessions.")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                    } else {
                        ForEach(viewModel.threadSections) { section in
                            VStack(alignment: .leading, spacing: 8) {
                                HStack {
                                    Text(section.title)
                                        .font(.caption.weight(.semibold))
                                        .foregroundStyle(tokens.tertiaryText)
                                    Spacer()
                                    Text("\(section.threads.count)")
                                        .font(.caption2.weight(.semibold))
                                        .foregroundStyle(tokens.tertiaryText)
                                }

                        ForEach(section.threads) { thread in
                            SidebarThreadRow(
                                thread: thread,
                                        isSelected: viewModel.state.conversation.sessionID == thread.id,
                                        isPinned: viewModel.isPinnedThread(thread.id),
                                        onOpen: { Task { await viewModel.selectThread(thread.id) } },
                                        onTogglePinned: { Task { await viewModel.togglePinnedThread(thread.id) } },
                                        onClear: { Task { await viewModel.clearThread(thread.id) } }
                                    )
                                }
                            }
                        }
                    }
                }

                if !appViewModel.recentCommandActionIDs.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Recent Tasks")
                            .font(.system(size: 11, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                        ForEach(resolvedRecentTasks) { task in
                            RecentTaskCard(task: task)
                        }
                    }
                }
            }
            .padding(14)
        }
        .background(
            LinearGradient(
                colors: [
                    tokens.panelBackground.opacity(0.92),
                    tokens.windowBackground.opacity(0.98),
                    tokens.accent.opacity(0.05),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
    }

    private struct ActiveSessionCard: View {
        let thread: ThreadSummary
        let isPinned: Bool
        let onOpen: () -> Void
        let onTogglePinned: () -> Void
        let onClear: () -> Void
        @Environment(\.colorScheme) private var colorScheme

        var body: some View {
            let tokens = ThemeTokens.current(for: colorScheme)

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Active Session")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    if isPinned {
                        Image(systemName: "pin.fill")
                            .font(.caption)
                            .foregroundStyle(tokens.warning)
                    }
                }

                Text(thread.title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(2)

                HStack(spacing: 8) {
                    Text(thread.updatedLabel)
                    Text("•")
                    Text("\(thread.eventCount) events")
                }
                .font(.caption)
                .foregroundStyle(tokens.secondaryText)

                HStack(spacing: 8) {
                    MiniActionButton(title: "Open", systemImage: "arrow.up.right") {
                        onOpen()
                    }
                    MiniActionButton(title: isPinned ? "Unpin" : "Pin", systemImage: isPinned ? "pin.slash" : "pin") {
                        onTogglePinned()
                    }
                    Button("Clear", role: .destructive) {
                        onClear()
                    }
                    .buttonStyle(.borderless)
                }
            }
            .padding(14)
            .background(tokens.panelBackground.opacity(0.72), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(tokens.accent.opacity(0.26), lineWidth: 1)
            )
        }
    }

    private struct SidebarThreadRow: View {
        let thread: ThreadSummary
        let isSelected: Bool
        let isPinned: Bool
        let onOpen: () -> Void
        let onTogglePinned: () -> Void
        let onClear: () -> Void
        @Environment(\.colorScheme) private var colorScheme

        var body: some View {
            let tokens = ThemeTokens.current(for: colorScheme)

            Button {
                onOpen()
            } label: {
                HStack(spacing: 12) {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(isSelected ? tokens.accent.opacity(0.95) : tokens.border.opacity(0.45))
                        .frame(width: 3)

                    VStack(alignment: .leading, spacing: 5) {
                        HStack(spacing: 8) {
                            Text(thread.title)
                                .font(.system(size: 13, weight: isSelected ? .semibold : .medium))
                                .foregroundStyle(tokens.primaryText)
                                .lineLimit(1)
                            if isPinned {
                                Image(systemName: "pin.fill")
                                    .font(.caption2)
                                    .foregroundStyle(tokens.warning)
                            }
                            Spacer()
                            Text(thread.updatedLabel)
                                .font(.caption2)
                                .foregroundStyle(tokens.tertiaryText)
                        }

                        HStack(spacing: 6) {
                            Text("\(thread.eventCount) events")
                            Text("•")
                            Text(thread.subtitle)
                                .lineLimit(1)
                        }
                        .font(.caption)
                        .foregroundStyle(tokens.secondaryText)
                    }

                    HStack(spacing: 8) {
                        Button {
                            onTogglePinned()
                        } label: {
                            Image(systemName: isPinned ? "pin.fill" : "pin")
                        }
                        .buttonStyle(.borderless)
                        .foregroundStyle(isPinned ? tokens.warning : tokens.tertiaryText)

                        Button(role: .destructive) {
                            onClear()
                        } label: {
                            Image(systemName: "trash")
                        }
                        .buttonStyle(.borderless)
                        .foregroundStyle(tokens.tertiaryText)
                    }
                    .opacity(isSelected ? 1 : 0.7)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .fill(isSelected ? tokens.elevatedBackground.opacity(0.95) : tokens.panelBackground.opacity(0.56))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .stroke(
                            isSelected ? tokens.accent.opacity(0.24) : tokens.border.opacity(0.42),
                            lineWidth: 1
                        )
                )
            }
            .buttonStyle(.plain)
            .contextMenu {
                Button("Open Session") {
                    onOpen()
                }
                Button(isPinned ? "Unpin Session" : "Pin Session") {
                    onTogglePinned()
                }
                Button("Clear Session", role: .destructive) {
                    onClear()
                }
            }
        }
    }

    @ViewBuilder
    private func quickActionRow(for action: QuickAction, tokens: ThemeTokens) -> some View {
        switch action.id {
        case "new-thread":
            Button {
                appViewModel.createNewThread()
            } label: {
                quickActionLabel(for: action, tokens: tokens)
            }
            .buttonStyle(.plain)
        case "refresh":
            Button {
                Task { await appViewModel.refreshActiveWorkspace() }
            } label: {
                quickActionLabel(for: action, tokens: tokens)
            }
            .buttonStyle(.plain)
        case "switch-workspace":
            Button {
                appViewModel.showSetupHub()
            } label: {
                quickActionLabel(for: action, tokens: tokens)
            }
            .buttonStyle(.plain)
        case "reveal-workspace":
            Button {
                appViewModel.revealSelectedWorkspaceInFinder()
            } label: {
                quickActionLabel(for: action, tokens: tokens)
            }
            .buttonStyle(.plain)
        case "settings":
            Button {
                Task { await appViewModel.recordCommandAction("settings") }
                openSettings()
            } label: {
                quickActionLabel(for: action, tokens: tokens)
            }
            .buttonStyle(.plain)
        default:
            quickActionLabel(for: action, tokens: tokens)
        }
    }

    private func quickActionLabel(for action: QuickAction, tokens: ThemeTokens) -> some View {
        HStack(spacing: 10) {
            Image(systemName: action.systemImage)
                .font(.caption.weight(.semibold))
                .foregroundStyle(tokens.accent)
            Text(action.title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(tokens.primaryText)
            Spacer()
            Image(systemName: "arrow.up.right")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(tokens.tertiaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 9)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tokens.panelBackground.opacity(0.54), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(tokens.border.opacity(0.44), lineWidth: 1)
        )
    }

    private var resolvedRecentTasks: [RecentTaskDescriptor] {
        Array(appViewModel.recentCommandActionIDs.prefix(5)).compactMap { actionID in
            recentTaskDescriptor(for: actionID)
        }
    }

    private func recentTaskDescriptor(for actionID: String) -> RecentTaskDescriptor? {
        let workspaceTitle = viewModel.state.sidebar.currentWorkspace.name

        switch actionID {
        case "new-thread":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Start a new thread",
                subtitle: "Open a fresh conversation in \(workspaceTitle).",
                systemImage: "square.and.pencil"
            ) {
                appViewModel.createNewThread()
            }
        case "refresh-workspace":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Refresh workspace",
                subtitle: "Reload health, sessions, and runtime state for \(workspaceTitle).",
                systemImage: "arrow.clockwise"
            ) {
                Task { await appViewModel.refreshActiveWorkspace() }
            }
        case "reveal-workspace":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Reveal in Finder",
                subtitle: viewModel.state.sidebar.currentWorkspace.path,
                systemImage: "folder.badge.gearshape"
            ) {
                appViewModel.revealSelectedWorkspaceInFinder()
            }
        case "settings":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Open settings",
                subtitle: "Adjust runtime, actions, and desktop defaults.",
                systemImage: "gearshape"
            ) {
                Task { await appViewModel.recordCommandAction("settings") }
                openSettings()
            }
        case "choose-workspace":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Choose workspace",
                subtitle: "Return to the setup hub and switch projects.",
                systemImage: "folder"
            ) {
                appViewModel.showSetupHub()
            }
        case "open-workspace":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Open workbench",
                subtitle: "Resume the active project in the main workspace.",
                systemImage: "play.rectangle"
            ) {
                Task { await appViewModel.openSelectedWorkspace() }
            }
        case "initialize-workspace":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Initialize workspace",
                subtitle: "Create the local Mosaic runtime config with the current draft.",
                systemImage: "wand.and.stars"
            ) {
                Task { await appViewModel.completeOnboarding() }
            }
        case "save-runtime":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Save runtime settings",
                subtitle: "\(appViewModel.currentProviderLabel) · \(appViewModel.currentModelLabel)",
                systemImage: "internaldrive"
            ) {
                Task { await appViewModel.saveRuntimeSettings() }
            }
        case "clear-thread":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Clear selected thread",
                subtitle: viewModel.selectedThreadSummary?.title ?? "Remove the active session from this workspace.",
                systemImage: "trash"
            ) {
                Task { await appViewModel.clearCurrentThread() }
            }
        case "send-prompt":
            return RecentTaskDescriptor(
                id: actionID,
                title: "Send prompt",
                subtitle: "Dispatch the current composer text to \(appViewModel.currentModelLabel).",
                systemImage: "paperplane"
            ) {
                Task { await appViewModel.sendCurrentPrompt() }
            }
        case "toggle-inspector":
            return RecentTaskDescriptor(
                id: actionID,
                title: viewModel.isInspectorVisible ? "Hide inspector" : "Show inspector",
                subtitle: "Toggle the runtime and session side panel.",
                systemImage: viewModel.isInspectorVisible ? "sidebar.right" : "sidebar.left"
            ) {
                appViewModel.toggleInspector()
            }
        default:
            if let task = workspaceTaskDescriptor(for: actionID) {
                return task
            }
            if let task = sessionTaskDescriptor(for: actionID) {
                return task
            }
            return RecentTaskDescriptor(
                id: actionID,
                title: actionID,
                subtitle: "Replay this workspace action.",
                systemImage: "clock.arrow.circlepath"
            ) {
                Task { await appViewModel.recordCommandAction(actionID) }
            }
        }
    }

    private func workspaceTaskDescriptor(for actionID: String) -> RecentTaskDescriptor? {
        guard actionID.hasPrefix("workspace-") else { return nil }
        let rawID = String(actionID.dropFirst("workspace-".count))
        guard let workspaceID = UUID(uuidString: rawID) else { return nil }

        let candidates = [appViewModel.selectedWorkspace].compactMap { $0 }
            + appViewModel.recentWorkspaces
            + viewModel.state.sidebar.recentWorkspaces
        guard let workspace = candidates.first(where: { $0.id == workspaceID }) else { return nil }

        return RecentTaskDescriptor(
            id: actionID,
            title: "Switch to \(workspace.name)",
            subtitle: workspace.path,
            systemImage: "folder"
        ) {
            Task { await appViewModel.activateWorkspace(workspace, recordHistory: true) }
        }
    }

    private func sessionTaskDescriptor(for actionID: String) -> RecentTaskDescriptor? {
        if actionID.hasPrefix("session-open-") {
            let sessionID = String(actionID.dropFirst("session-open-".count))
            guard let thread = viewModel.state.sidebar.threads.first(where: { $0.id == sessionID }) else { return nil }
            return RecentTaskDescriptor(
                id: actionID,
                title: "Resume \(thread.title)",
                subtitle: "\(thread.eventCount) events · \(thread.updatedLabel)",
                systemImage: "text.bubble"
            ) {
                Task { await appViewModel.openSession(sessionID, recordHistory: true) }
            }
        }

        if actionID.hasPrefix("session-pin-") {
            let sessionID = String(actionID.dropFirst("session-pin-".count))
            guard let thread = viewModel.state.sidebar.threads.first(where: { $0.id == sessionID }) else { return nil }
            let isPinned = viewModel.isPinnedThread(sessionID)
            return RecentTaskDescriptor(
                id: actionID,
                title: isPinned ? "Unpin \(thread.title)" : "Pin \(thread.title)",
                subtitle: isPinned ? "Move it back into the recent sessions group." : "Keep this session at the top of the sidebar.",
                systemImage: isPinned ? "pin.slash" : "pin"
            ) {
                Task { await appViewModel.togglePinnedSession(sessionID, recordHistory: true) }
            }
        }

        if actionID.hasPrefix("session-clear-") {
            let sessionID = String(actionID.dropFirst("session-clear-".count))
            guard let thread = viewModel.state.sidebar.threads.first(where: { $0.id == sessionID }) else { return nil }
            return RecentTaskDescriptor(
                id: actionID,
                title: "Clear \(thread.title)",
                subtitle: "Delete this session from \(viewModel.state.sidebar.currentWorkspace.name).",
                systemImage: "trash"
            ) {
                Task { await appViewModel.clearSession(sessionID, recordHistory: true) }
            }
        }

        return nil
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
                .frame(maxWidth: 920)
                .padding(.horizontal, 24)
                .padding(.top, 18)
                .padding(.bottom, 6)

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
                    .frame(maxWidth: 920)
                    .padding(.horizontal, 24)
                    .padding(.top, 12)
                    .padding(.bottom, 22)
                    .frame(maxWidth: .infinity)
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
                .frame(maxWidth: 920)
                .padding(.horizontal, 24)
                .padding(.top, 8)
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

        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 8) {
                        Text("THREAD")
                        Text("•")
                        Text(viewModel.state.sidebar.currentWorkspace.name.uppercased())
                        if let sessionID = viewModel.state.conversation.sessionID {
                            Text("•")
                            Text(String(sessionID.prefix(8)).uppercased())
                        }
                    }
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)

                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(viewModel.state.conversation.threadTitle)
                            .font(.system(size: 22, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                            .fixedSize(horizontal: false, vertical: true)

                        if let selected = viewModel.selectedThreadSummary {
                            Text(selected.updatedLabel.uppercased())
                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                .foregroundStyle(tokens.tertiaryText)
                        }
                    }

                    Text(viewModel.selectedThreadSummary?.subtitle ?? "Workspace-first execution thread")
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(2)
                }

                Spacer(minLength: 18)

                VStack(alignment: .trailing, spacing: 8) {
                    HStack(spacing: 6) {
                        ToolbarPill(
                            title: "Provider",
                            value: appViewModel.currentProviderLabel,
                            accent: tokens.accent
                        )
                        ToolbarPill(
                            title: "Model",
                            value: appViewModel.currentModelLabel,
                            accent: tokens.success
                        )
                        ToolbarStatusPill(
                            title: "Health",
                            value: appViewModel.currentHealthLabel,
                            systemImage: "waveform.path.ecg",
                            accent: appViewModel.currentHealthLabel == "Healthy" ? tokens.success : tokens.warning
                        )
                    }

                    HStack(spacing: 8) {
                        ConversationMetricBadge(
                            title: "Turns",
                            value: "\(viewModel.messageCount)",
                            accent: tokens.success
                        )
                        ConversationMetricBadge(
                            title: "Sessions",
                            value: "\(viewModel.threadCount)",
                            accent: tokens.warning
                        )
                    }
                }
            }

            HStack(spacing: 8) {
                MiniActionButton(title: "Palette", systemImage: "magnifyingglass") {
                    appViewModel.presentCommandPalette()
                }
                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await appViewModel.refreshActiveWorkspace() }
                }
                MiniActionButton(title: "New", systemImage: "square.and.pencil") {
                    appViewModel.createNewThread()
                }
                Spacer()
                Text("CMD+ENTER TO SEND")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }

            StatusStripView(status: viewModel.state.conversation.status)
        }
        .padding(.horizontal, 2)
        .padding(.vertical, 6)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(tokens.border.opacity(0.8))
                .frame(height: 1)
                .padding(.top, 8)
        }
    }
}

private struct ConversationMetricBadge: View {
    let title: String
    let value: String
    let accent: Color
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Circle()
                .fill(accent.opacity(0.92))
                .frame(width: 5, height: 5)
            Text(title.uppercased())
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            Text(value)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(tokens.panelBackground.opacity(0.5), in: Capsule())
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
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .background(tokens.panelBackground.opacity(0.5), in: Capsule())
                .overlay(
                    Capsule()
                        .stroke(tokens.border.opacity(0.42), lineWidth: 1)
                )
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
            HStack(spacing: 8) {
                Button {
                    appViewModel.presentCommandPalette()
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                        .frame(width: 28, height: 28)
                        .background(tokens.windowBackground.opacity(0.55), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                }
                .buttonStyle(.plain)

                MiniActionButton(title: "Palette", systemImage: "magnifyingglass") {
                    appViewModel.presentCommandPalette()
                }

                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await appViewModel.refreshActiveWorkspace() }
                }

                Spacer()

                Text(viewModel.state.sidebar.currentWorkspace.name.uppercased())
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }

            if !viewModel.state.conversation.suggestedPrompts.isEmpty {
                SuggestionPromptStrip(prompts: viewModel.state.conversation.suggestedPrompts) { prompt in
                    viewModel.applySuggestedPrompt(prompt)
                }
            }

            ZStack(alignment: .topLeading) {
                if viewModel.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Text("Ask Mosaic to inspect the workspace, propose changes, or continue the current thread…")
                        .font(.system(size: 14))
                        .foregroundStyle(tokens.tertiaryText)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 16)
                }

                TextEditor(text: $viewModel.composerText)
                    .font(.system(size: 14))
                    .frame(minHeight: 114)
                    .scrollContentBackground(.hidden)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 2)
                    .background(Color.clear)
            }

            HStack(alignment: .center, spacing: 10) {
                HStack(spacing: 8) {
                    ToolbarPill(
                        title: "Profile",
                        value: appViewModel.currentProfileLabel,
                        accent: tokens.accent
                    )
                    ToolbarPill(
                        title: "Model",
                        value: appViewModel.currentModelLabel,
                        accent: tokens.success
                    )
                    ToolbarStatusPill(
                        title: "Runtime",
                        value: appViewModel.currentHealthLabel,
                        systemImage: "bolt.horizontal",
                        accent: appViewModel.currentHealthLabel == "Healthy" ? tokens.success : tokens.warning
                    )
                }

                Spacer()

                Text("CMD+ENTER")
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)

                Button("Clear Thread", role: .destructive) {
                    Task { await appViewModel.clearCurrentThread() }
                }
                .buttonStyle(.plain)
                .foregroundStyle(tokens.failure)
                .disabled(!viewModel.canClearSelectedThread)

                Button {
                    Task { await appViewModel.sendCurrentPrompt() }
                } label: {
                    Image(systemName: viewModel.state.conversation.isSending ? "clock" : "arrow.up")
                        .font(.system(size: 12, weight: .bold))
                        .foregroundStyle(Color.white)
                        .frame(width: 34, height: 34)
                        .background(tokens.accent, in: Circle())
                }
                .buttonStyle(.plain)
                .disabled(viewModel.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || viewModel.state.conversation.isSending)
            }
        }
        .padding(16)
        .background(tokens.panelBackground.opacity(0.76), in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(tokens.border.opacity(0.62), lineWidth: 1)
        )
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
                            .padding(.vertical, 8)
                            .background(tokens.windowBackground.opacity(0.26), in: Capsule())
                            .overlay(
                                Capsule()
                                    .stroke(tokens.border.opacity(0.52), lineWidth: 1)
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
                Text("Start from the workspace.")
                    .font(.system(size: 24, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .fixedSize(horizontal: false, vertical: true)

                Text("Use this thread to inspect the repo, audit runtime health, or turn the next change into an execution plan.")
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
                        Task { await appViewModel.refreshActiveWorkspace() }
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
        .background(tokens.panelBackground.opacity(0.72), in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(tokens.border.opacity(0.56), lineWidth: 1)
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
                    InspectorRuntimeSnapshotCard(appViewModel: appViewModel)
                    InspectorSessionDetailCard(appViewModel: appViewModel, viewModel: viewModel)
                    InspectorModelSwitcherCard(appViewModel: appViewModel)
                    inspectorSections(for: ["context", "runtime"])
                    InspectorHealthChecksCard(appViewModel: appViewModel)
                case .runtime:
                    InspectorRuntimeSnapshotCard(appViewModel: appViewModel)
                    InspectorModelSwitcherCard(appViewModel: appViewModel)
                    RuntimeControlsCard(viewModel: appViewModel, title: "Runtime controls")
                    inspectorSections(for: ["runtime", "context"])
                    InspectorHealthChecksCard(appViewModel: appViewModel)
                case .session:
                    InspectorSessionDetailCard(appViewModel: appViewModel, viewModel: viewModel)
                    InspectorSessionActions(appViewModel: appViewModel, viewModel: viewModel)
                    InspectorSessionListCard(appViewModel: appViewModel, viewModel: viewModel)
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
                    Text("CONTROL SURFACE")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.accent)
                    Text(viewModel.state.sidebar.currentWorkspace.name)
                        .font(.system(size: 20, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(viewModel.selectedThreadSummary?.title ?? "No active session selected")
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()

                VStack(alignment: .trailing, spacing: 8) {
                    if viewModel.isLoading {
                        Text("SYNCING")
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                    }
                    HStack(spacing: 8) {
                        ConversationMetricBadge(
                            title: "Sessions",
                            value: "\(viewModel.threadCount)",
                            accent: tokens.accent
                        )
                        ConversationMetricBadge(
                            title: "Turns",
                            value: "\(viewModel.messageCount)",
                            accent: tokens.success
                        )
                    }
                }
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

            HStack(spacing: 8) {
                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await appViewModel.refreshActiveWorkspace() }
                }
                MiniActionButton(title: "Setup", systemImage: "slider.horizontal.3") {
                    appViewModel.showSetupHub()
                }
                MiniActionButton(title: "Palette", systemImage: "magnifyingglass") {
                    appViewModel.presentCommandPalette()
                }
            }
        }
        .padding(16)
        .background(tokens.panelBackground.opacity(0.74), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border.opacity(0.64), lineWidth: 1)
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
                    Task { await appViewModel.refreshActiveWorkspace() }
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

private struct InspectorRuntimeSnapshotCard: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    private var aliasesCount: Int {
        appViewModel.selectedModelsStatus?.aliases.count ?? 0
    }

    private var fallbackCount: Int {
        appViewModel.selectedModelsStatus?.fallbacks.count ?? 0
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Runtime Snapshot")
                        .font(.headline)
                        .foregroundStyle(tokens.primaryText)
                    Text("Provider routing, profile, and config shape for the active workspace.")
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Text(appViewModel.currentProviderLabel)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(tokens.accent)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(tokens.elevatedBackground.opacity(0.88), in: Capsule())
            }

            HStack(spacing: 8) {
                ConversationMetricBadge(
                    title: "Profile",
                    value: appViewModel.currentProfileLabel,
                    accent: tokens.accent
                )
                ConversationMetricBadge(
                    title: "Aliases",
                    value: "\(aliasesCount)",
                    accent: tokens.success
                )
                ConversationMetricBadge(
                    title: "Fallbacks",
                    value: "\(fallbackCount)",
                    accent: tokens.warning
                )
            }

            SetupValueRow(label: "Endpoint", value: appViewModel.currentBaseURLLabel)
            SetupValueRow(label: "API Key Env", value: appViewModel.currentAPIKeyEnvLabel)
            SetupValueRow(label: "State Mode", value: appViewModel.selectedWorkspaceStatus?.stateMode ?? "project")
            SetupValueRow(label: "Agents", value: "\(appViewModel.selectedWorkspaceStatus?.agentsCount ?? 0)")
            SetupValueRow(label: "Config Path", value: appViewModel.selectedWorkspaceStatus?.configPath ?? "Unavailable")

            if let fallbacks = appViewModel.selectedModelsStatus?.fallbacks, !fallbacks.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Fallback order")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)

                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 120), spacing: 8)], spacing: 8) {
                        ForEach(fallbacks, id: \.self) { fallback in
                            Text(fallback)
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                                .lineLimit(1)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(tokens.windowBackground.opacity(0.36), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                                        .stroke(tokens.border.opacity(0.5), lineWidth: 1)
                                )
                        }
                    }
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

private struct InspectorSessionDetailCard: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    private var latestMessagePreview: String? {
        viewModel.state.conversation.messages.last?.body
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Current Session")
                    .font(.headline)
                    .foregroundStyle(tokens.primaryText)
                Spacer()
                if let sessionID = viewModel.state.conversation.sessionID, viewModel.isPinnedThread(sessionID) {
                    Label("Pinned", systemImage: "pin.fill")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(tokens.warning)
                }
            }

            if let selected = viewModel.selectedThreadSummary {
                Text(selected.title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .fixedSize(horizontal: false, vertical: true)

                HStack(spacing: 8) {
                    ConversationMetricBadge(
                        title: "Session",
                        value: String(selected.id.prefix(8)),
                        accent: tokens.accent
                    )
                    ConversationMetricBadge(
                        title: "Events",
                        value: "\(selected.eventCount)",
                        accent: tokens.success
                    )
                    ConversationMetricBadge(
                        title: "Updated",
                        value: selected.updatedLabel,
                        accent: tokens.warning
                    )
                }

                if let latestMessagePreview, !latestMessagePreview.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Latest message")
                            .font(.caption)
                            .foregroundStyle(tokens.tertiaryText)
                        Text(latestMessagePreview)
                            .font(.system(size: 12))
                            .foregroundStyle(tokens.secondaryText)
                            .lineLimit(4)
                            .padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(tokens.windowBackground.opacity(0.34), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                }

                HStack(spacing: 8) {
                    MiniActionButton(title: "Open", systemImage: "arrow.up.right") {
                        Task { await appViewModel.openSession(selected.id) }
                    }
                    MiniActionButton(
                        title: viewModel.isPinnedThread(selected.id) ? "Unpin" : "Pin",
                        systemImage: viewModel.isPinnedThread(selected.id) ? "pin.slash" : "pin"
                    ) {
                        Task { await appViewModel.togglePinnedSession(selected.id) }
                    }
                    Button("Clear Thread", role: .destructive) {
                        Task { await appViewModel.clearSession(selected.id) }
                    }
                    .buttonStyle(.bordered)
                    .disabled(viewModel.state.conversation.isSending)
                }
            } else {
                Text("No active session selected.")
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)

                MiniActionButton(title: "New Thread", systemImage: "square.and.pencil") {
                    appViewModel.createNewThread()
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

private struct InspectorModelSwitcherCard: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Available Models")
                        .font(.headline)
                        .foregroundStyle(tokens.primaryText)
                    Text("Quick-switch the active runtime model from the workbench.")
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                if appViewModel.isApplyingQuickModel {
                    ProgressView()
                        .controlSize(.small)
                }
            }

            LazyVGrid(columns: [GridItem(.adaptive(minimum: 120), spacing: 8)], spacing: 8) {
                ForEach(appViewModel.setupModelChoices, id: \.self) { modelID in
                    Button {
                        Task { await appViewModel.quickSwitchModel(modelID) }
                    } label: {
                        HStack(spacing: 8) {
                            Circle()
                                .fill(modelID == appViewModel.currentModelLabel ? tokens.success : tokens.accent.opacity(0.75))
                                .frame(width: 7, height: 7)
                            Text(modelID)
                                .font(.system(size: 12, weight: .semibold))
                                .lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .foregroundStyle(tokens.primaryText)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 9)
                        .background(
                            (modelID == appViewModel.currentModelLabel ? tokens.elevatedBackground : tokens.windowBackground.opacity(0.36)),
                            in: RoundedRectangle(cornerRadius: 14, style: .continuous)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .stroke(
                                    modelID == appViewModel.currentModelLabel ? tokens.success.opacity(0.45) : tokens.border.opacity(0.55),
                                    lineWidth: 1
                                )
                        )
                    }
                    .buttonStyle(.plain)
                    .disabled(!appViewModel.canQuickSwitchModels)
                }
            }

            if appViewModel.setupModelChoices.isEmpty {
                Text("No models available yet.")
                    .font(.system(size: 12))
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
}

private struct InspectorSessionActions: View {
    @Bindable var appViewModel: AppViewModel
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
                    appViewModel.createNewThread()
                }
                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await appViewModel.refreshActiveWorkspace() }
                }
                Button("Clear Thread", role: .destructive) {
                    Task { await appViewModel.clearCurrentThread() }
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

private struct InspectorSessionListCard: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            Text("Recent Sessions")
                .font(.headline)
                .foregroundStyle(tokens.primaryText)

            if viewModel.state.sidebar.threads.isEmpty {
                Text("No sessions in this workspace.")
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
            } else {
                ForEach(viewModel.state.sidebar.threads) { thread in
                    HStack(spacing: 10) {
                        Button {
                            Task { await appViewModel.openSession(thread.id) }
                        } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Text(thread.title)
                                        .font(.system(size: 13, weight: .semibold))
                                        .foregroundStyle(tokens.primaryText)
                                        .lineLimit(1)
                                    Spacer()
                                    Text(thread.updatedLabel)
                                        .font(.caption2)
                                        .foregroundStyle(tokens.tertiaryText)
                                }
                                Text("\(thread.eventCount) events")
                                    .font(.caption)
                                    .foregroundStyle(tokens.secondaryText)
                            }
                            .padding(10)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(
                                (viewModel.state.conversation.sessionID == thread.id ? tokens.elevatedBackground : tokens.windowBackground.opacity(0.32)),
                                in: RoundedRectangle(cornerRadius: 14, style: .continuous)
                            )
                            .overlay(
                                RoundedRectangle(cornerRadius: 14, style: .continuous)
                                    .stroke(
                                        viewModel.state.conversation.sessionID == thread.id ? tokens.accent.opacity(0.42) : tokens.border.opacity(0.45),
                                        lineWidth: 1
                                    )
                            )
                        }
                        .buttonStyle(.plain)

                        VStack(spacing: 8) {
                            Button {
                                Task { await appViewModel.togglePinnedSession(thread.id) }
                            } label: {
                                Image(systemName: viewModel.isPinnedThread(thread.id) ? "pin.fill" : "pin")
                                    .frame(width: 28, height: 28)
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(viewModel.isPinnedThread(thread.id) ? tokens.warning : tokens.tertiaryText)

                            Button(role: .destructive) {
                                Task { await appViewModel.clearSession(thread.id) }
                            } label: {
                                Image(systemName: "trash")
                                    .frame(width: 28, height: 28)
                            }
                            .buttonStyle(.borderless)
                            .disabled(viewModel.state.conversation.isSending)
                        }
                    }
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

private struct InspectorHealthChecksCard: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    private var checks: [HealthCheckSummary] {
        appViewModel.selectedWorkspaceHealth?.checks ?? []
    }

    private var okCount: Int {
        checks.filter { $0.status == "ok" }.count
    }

    private var warnCount: Int {
        checks.filter { $0.status == "warn" }.count
    }

    private var failCount: Int {
        checks.filter { $0.status == "fail" }.count
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Health Checks")
                    .font(.headline)
                    .foregroundStyle(tokens.primaryText)
                Spacer()
                MiniActionButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    Task { await appViewModel.refreshActiveWorkspace() }
                }
            }

            HStack(spacing: 8) {
                ConversationMetricBadge(
                    title: "Healthy",
                    value: "\(okCount)",
                    accent: tokens.success
                )
                ConversationMetricBadge(
                    title: "Warnings",
                    value: "\(warnCount)",
                    accent: tokens.warning
                )
                ConversationMetricBadge(
                    title: "Failures",
                    value: "\(failCount)",
                    accent: tokens.failure
                )
            }

            if !checks.isEmpty {
                if failCount > 0 || warnCount > 0 {
                    SetupHintRow(
                        systemImage: failCount > 0 ? "exclamationmark.triangle.fill" : "info.circle.fill",
                        text: failCount > 0
                            ? "Runtime health is degraded. Review the failing checks, then adjust the workspace runtime if needed."
                            : "Some checks need attention. Refresh after updating the runtime or workspace state."
                    )
                }

                ForEach(Array(checks.enumerated()), id: \.offset) { index, check in
                    VStack(alignment: .leading, spacing: 10) {
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
                    }
                    .padding(12)
                    .background(tokens.windowBackground.opacity(0.34), in: RoundedRectangle(cornerRadius: 14, style: .continuous))

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
        let leadingAccent: Color = switch message.role {
        case .assistant:
            tokens.accent
        case .user:
            tokens.success
        case .system:
            tokens.warning
        }
        let blockBackground: Color = switch message.role {
        case .assistant:
            .clear
        case .user:
            tokens.panelBackground.opacity(0.52)
        case .system:
            tokens.panelBackground.opacity(0.34)
        }
        let blockBorder: Color = switch message.role {
        case .assistant:
            .clear
        case .user:
            tokens.accent.opacity(0.18)
        case .system:
            tokens.border.opacity(0.34)
        }

        HStack(alignment: .top, spacing: 12) {
            ConversationRoleGlyph(role: message.role)

            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Text(message.role.title.uppercased())
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.primaryText)
                    Text(message.timestampLabel.uppercased())
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    if isUser {
                        Text("PROMPT")
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.tertiaryText)
                    }
                    if isSystem {
                        Text("RUNTIME")
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(tokens.warning)
                    }
                    Spacer()
                }

                VStack(alignment: .leading, spacing: 0) {
                    HStack(alignment: .top, spacing: 0) {
                        RoundedRectangle(cornerRadius: 999, style: .continuous)
                            .fill(leadingAccent.opacity(isSystem ? 0.74 : 0.95))
                            .frame(width: 2)

                        Text(message.body)
                            .font(.system(size: isSystem ? 13 : 15))
                            .foregroundStyle(tokens.primaryText)
                            .lineSpacing(4)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.leading, 14)
                    }
                    .padding(.vertical, isUser || isSystem ? 12 : 2)
                    .padding(.horizontal, isUser || isSystem ? 12 : 0)
                    .background(
                        blockBackground,
                        in: RoundedRectangle(cornerRadius: 16, style: .continuous)
                    )
                    .overlay {
                        if isUser || isSystem {
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(blockBorder, lineWidth: 1)
                        }
                    }
                }
            }
        }
        .frame(maxWidth: isUser ? 720 : 860, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
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
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .fill(tokens.panelBackground.opacity(0.72))
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .stroke(accent.opacity(0.28), lineWidth: 1)
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(accent)
        }
        .frame(width: 22, height: 22)
        .padding(.top, 2)
    }
}
