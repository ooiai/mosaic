import AppKit
import Features
import Infrastructure
import Observation
import SwiftUI
import UI

final class MosaicAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        NSApp.windows.first?.makeKeyAndOrderFront(nil)
    }
}

@main
struct MosaicMacApp: App {
    @NSApplicationDelegateAdaptor(MosaicAppDelegate.self) private var appDelegate
    @State private var appViewModel = AppViewModel(
        runtimeClient: MosaicCLIClient(),
        workspaceStore: WorkspaceStore()
    )

    var body: some Scene {
        WindowGroup("Mosaic") {
            RootContentView(viewModel: appViewModel)
                .frame(minWidth: 1180, minHeight: 760)
        }
        .commands {
            MosaicAppCommands(viewModel: appViewModel)
        }

        Settings {
            SettingsView(viewModel: appViewModel)
        }
    }
}

struct SettingsView: View {
    private enum SettingsSection: String, CaseIterable, Identifiable {
        case general
        case runtime
        case workspaces
        case actions
        case about

        var id: String { rawValue }

        var title: String {
            switch self {
            case .general: "General"
            case .runtime: "Runtime"
            case .workspaces: "Workspaces"
            case .actions: "Actions"
            case .about: "About"
            }
        }

        var subtitle: String {
            switch self {
            case .general: "Desktop behavior and current environment"
            case .runtime: "Provider, model, and runtime defaults"
            case .workspaces: "Current project and recent roots"
            case .actions: "Recent actions and keyboard flows"
            case .about: "Bundled CLI runtime and development overrides"
            }
        }

        var systemImage: String {
            switch self {
            case .general: "slider.horizontal.3"
            case .runtime: "bolt.horizontal.circle"
            case .workspaces: "folder"
            case .actions: "command"
            case .about: "info.circle"
            }
        }
    }

    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var selection: SettingsSection = .general

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 0) {
            settingsSidebar(tokens: tokens)

            Divider()
                .overlay(tokens.border.opacity(0.55))

            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(selection.title)
                            .font(.system(size: 30, weight: .bold, design: .rounded))
                            .foregroundStyle(tokens.primaryText)
                        Text(selection.subtitle)
                            .font(.system(size: 13))
                            .foregroundStyle(tokens.secondaryText)
                    }

                    settingsContent(tokens: tokens)
                }
                .padding(28)
            }
        }
        .frame(minWidth: 980, minHeight: 720)
        .background(
            LinearGradient(
                colors: [
                    tokens.windowBackground,
                    tokens.windowBackground,
                    tokens.accent.opacity(colorScheme == .dark ? 0.06 : 0.03),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
    }

    @ViewBuilder
    private func settingsContent(tokens: ThemeTokens) -> some View {
        switch selection {
        case .general:
            generalSettingsContent(tokens: tokens)
        case .runtime:
            runtimeSettingsContent(tokens: tokens)
        case .workspaces:
            workspaceSettingsContent(tokens: tokens)
        case .actions:
            actionSettingsContent(tokens: tokens)
        case .about:
            aboutSettingsContent(tokens: tokens)
        }
    }

    private func settingsSidebar(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Mosaic")
                    .font(.system(size: 24, weight: .bold, design: .rounded))
                    .foregroundStyle(tokens.primaryText)
                Text("Native desktop workbench settings")
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
            }

            VStack(alignment: .leading, spacing: 6) {
                ForEach(SettingsSection.allCases) { item in
                    Button {
                        selection = item
                    } label: {
                        HStack(spacing: 10) {
                            Image(systemName: item.systemImage)
                                .frame(width: 18)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(item.title)
                                    .font(.system(size: 13, weight: .semibold))
                                Text(item.subtitle)
                                    .font(.caption2)
                                    .lineLimit(1)
                            }
                            Spacer()
                        }
                        .foregroundStyle(selection == item ? tokens.primaryText : tokens.secondaryText)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 10)
                        .background(
                            (selection == item ? tokens.elevatedBackground.opacity(0.92) : Color.clear),
                            in: RoundedRectangle(cornerRadius: 14, style: .continuous)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .stroke(selection == item ? tokens.accent.opacity(0.4) : Color.clear, lineWidth: 1)
                        )
                    }
                    .buttonStyle(.plain)
                }
            }

            Spacer()

            settingsPanelCard(tokens: tokens, title: "Current Workspace", description: "The desktop app keeps runtime state anchored to the selected project.") {
                if let workspace = viewModel.selectedWorkspace {
                    Text(workspace.name)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(workspace.path)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(3)
                } else {
                    Text("No workspace selected yet.")
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                }
            }
        }
        .padding(22)
        .frame(width: 290, alignment: .topLeading)
        .background(tokens.panelBackground.opacity(0.80))
    }

    private func generalSettingsContent(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 10) {
                settingsMetricPill(title: "Provider", value: viewModel.currentProviderLabel, tokens: tokens)
                settingsMetricPill(title: "Model", value: viewModel.currentModelLabel, tokens: tokens)
                settingsMetricPill(title: "Health", value: viewModel.currentHealthLabel, tokens: tokens)
            }

            settingsPanelCard(tokens: tokens, title: "Desktop Behavior", description: "High-level desktop defaults and command surface.") {
                settingsValueRow(tokens: tokens, label: "Command palette", value: "Cmd+Shift+P")
                settingsValueRow(tokens: tokens, label: "Send prompt", value: "Cmd+Enter")
                settingsValueRow(tokens: tokens, label: "New thread", value: "Cmd+N")
                settingsValueRow(tokens: tokens, label: "Toggle inspector", value: "Cmd+Option+I")
            }

            settingsPanelCard(tokens: tokens, title: "Current Environment", description: "The app reflects the active workspace and runtime state.") {
                settingsValueRow(tokens: tokens, label: "Workspace", value: viewModel.selectedWorkspace?.name ?? "None")
                settingsValueRow(tokens: tokens, label: "Profile", value: viewModel.currentProfileLabel)
                settingsValueRow(tokens: tokens, label: "Provider", value: viewModel.currentProviderLabel)
                settingsValueRow(tokens: tokens, label: "Base URL", value: viewModel.currentBaseURLLabel)
            }
        }
    }

    private func runtimeSettingsContent(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            settingsPanelCard(tokens: tokens, title: "Runtime Draft", description: "Edit runtime values without dropping back to the CLI.") {
                if viewModel.selectedWorkspaceConfigured {
                    HStack(spacing: 10) {
                        settingsMetricPill(title: "Profile", value: viewModel.currentProfileLabel, tokens: tokens)
                        settingsMetricPill(title: "Provider", value: viewModel.currentProviderLabel, tokens: tokens)
                        settingsMetricPill(title: "Model", value: viewModel.currentModelLabel, tokens: tokens)
                        settingsMetricPill(title: "Health", value: viewModel.currentHealthLabel, tokens: tokens)
                    }

                    HStack(spacing: 10) {
                        Button("Azure OpenAI") {
                            viewModel.applyAzureRuntimePreset()
                        }
                        .buttonStyle(.bordered)

                        Button("Local Server") {
                            viewModel.applyLocalRuntimePreset()
                        }
                        .buttonStyle(.bordered)
                    }

                    if viewModel.runtimeDraftUsesAzurePreset {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Azure Resource")
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

                            settingsMonospaceValue(tokens: tokens, value: viewModel.runtimeDraftBaseURL)
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
                        Label("Replace `YOUR_RESOURCE_NAME` before saving the Azure runtime.", systemImage: "exclamationmark.triangle.fill")
                            .font(.system(size: 12))
                            .foregroundStyle(tokens.warning)
                    }

                    HStack(spacing: 10) {
                        Spacer()

                        Button("Reset") {
                            viewModel.resetRuntimeDraft()
                        }
                        .buttonStyle(.bordered)
                        .disabled(!viewModel.runtimeDraftHasChanges)

                        Button(viewModel.isSavingRuntimeSettings ? "Saving…" : "Save Runtime") {
                            Task { await viewModel.saveRuntimeSettings() }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(!viewModel.canSaveRuntimeSettings)
                    }
                } else {
                    Text("Runtime editing becomes available after the workspace has been initialized.")
                        .foregroundStyle(tokens.secondaryText)
                }
            }
        }
    }

    private func workspaceSettingsContent(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            settingsPanelCard(tokens: tokens, title: "Current Workspace", description: "Project root and scoped desktop actions.") {
                if let workspace = viewModel.selectedWorkspace {
                    Text(workspace.name)
                        .font(.title3.weight(.semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(workspace.path)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .textSelection(.enabled)

                    HStack(spacing: 10) {
                        Button("Open Setup Hub") {
                            viewModel.showSetupHub()
                        }
                        .buttonStyle(.bordered)

                        Button("Refresh Workspace") {
                            Task { await viewModel.refreshActiveWorkspace() }
                        }
                        .buttonStyle(.bordered)

                        Button("Reveal in Finder") {
                            viewModel.revealSelectedWorkspaceInFinder()
                        }
                        .buttonStyle(.bordered)
                    }
                } else {
                    Text("No workspace selected.")
                        .foregroundStyle(tokens.secondaryText)
                }
            }

            if !viewModel.recentWorkspaces.isEmpty {
                settingsPanelCard(tokens: tokens, title: "Recent Workspaces", description: "Switch among recent project roots directly from settings.") {
                    ForEach(viewModel.recentWorkspaces) { workspace in
                        Button {
                            Task { await viewModel.selectWorkspace(workspace) }
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(workspace.name)
                                        .foregroundStyle(tokens.primaryText)
                                    Text(workspace.path)
                                        .font(.caption)
                                        .foregroundStyle(tokens.secondaryText)
                                        .lineLimit(1)
                                }
                                Spacer()
                                if workspace.id == viewModel.selectedWorkspace?.id {
                                    Text("Current")
                                        .font(.caption.weight(.semibold))
                                        .foregroundStyle(tokens.accent)
                                }
                            }
                            .padding(12)
                            .background(tokens.windowBackground.opacity(0.38), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    private func actionSettingsContent(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            settingsPanelCard(tokens: tokens, title: "Recent Actions", description: "Actions surfaced in the command palette history.") {
                HStack {
                    Text("Latest command palette history")
                        .font(.caption)
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Button("Clear") {
                        Task { await viewModel.clearRecentCommandActions() }
                    }
                    .buttonStyle(.bordered)
                }

                if viewModel.recentCommandActionIDs.isEmpty {
                    Text("No command history yet.")
                        .font(.system(size: 13))
                        .foregroundStyle(tokens.secondaryText)
                } else {
                    ForEach(viewModel.recentCommandActionIDs, id: \.self) { actionID in
                        HStack {
                            Image(systemName: "clock.arrow.circlepath")
                                .foregroundStyle(tokens.accent)
                            Text(displayName(for: actionID))
                                .font(.system(size: 13))
                                .foregroundStyle(tokens.primaryText)
                            Spacer()
                        }
                        .padding(10)
                        .background(tokens.windowBackground.opacity(0.38), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    }
                }
            }

            settingsPanelCard(tokens: tokens, title: "Keyboard Surface", description: "Primary shortcuts exposed by the native workbench.") {
                settingsValueRow(tokens: tokens, label: "Command palette", value: "Cmd+Shift+P")
                settingsValueRow(tokens: tokens, label: "Choose workspace", value: "Cmd+Shift+O")
                settingsValueRow(tokens: tokens, label: "Refresh workspace", value: "Cmd+Shift+R")
                settingsValueRow(tokens: tokens, label: "Send prompt", value: "Cmd+Enter")
                settingsValueRow(tokens: tokens, label: "Clear selected thread", value: "Cmd+Delete")
            }
        }
    }

    private func aboutSettingsContent(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            settingsPanelCard(tokens: tokens, title: "Desktop Runtime", description: "Native macOS shell backed by the bundled mosaic CLI JSON runtime.") {
                settingsValueRow(tokens: tokens, label: "CLI source", value: "Bundled `mosaic` sidecar")
                settingsValueRow(tokens: tokens, label: "Development override", value: "MOSAIC_CLI_PATH")
                settingsValueRow(tokens: tokens, label: "Preferred provider", value: "Azure OpenAI")
            }

            settingsPanelCard(tokens: tokens, title: "Packaging Notes", description: "Release builds prefer the bundled executable and local app resources.") {
                Text("Use `MOSAIC_CLI_PATH` in development to override the CLI executable. Release builds still prefer the bundled `mosaic` sidecar.")
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func displayName(for actionID: String) -> String {
        switch actionID {
        case "choose-workspace":
            return "Choose Workspace"
        case "settings":
            return "Open Settings"
        case "refresh-workspace":
            return "Refresh Workspace"
        case "reveal-workspace":
            return "Reveal in Finder"
        case "open-workspace":
            return "Open Workspace"
        case "initialize-workspace":
            return "Initialize Workspace"
        case "save-runtime":
            return "Save Runtime Settings"
        case "new-thread":
            return "New Thread"
        case "send-prompt":
            return "Send Prompt"
        case "clear-thread":
            return "Clear Selected Thread"
        case "toggle-inspector":
            return "Toggle Inspector"
        default:
            if actionID.hasPrefix("workspace-") {
                let rawID = String(actionID.dropFirst("workspace-".count))
                if
                    let workspaceID = UUID(uuidString: rawID),
                    let workspace = ([viewModel.selectedWorkspace].compactMap { $0 } + viewModel.recentWorkspaces)
                        .first(where: { $0.id == workspaceID })
                {
                    return "Switch to \(workspace.name)"
                }
                return "Switch Workspace"
            }
            if actionID.hasPrefix("session-open-") {
                let sessionID = String(actionID.dropFirst("session-open-".count))
                if let thread = viewModel.workbench?.state.sidebar.threads.first(where: { $0.id == sessionID }) {
                    return "Resume \(thread.title)"
                }
                return "Open Session"
            }
            if actionID.hasPrefix("session-pin-") {
                let sessionID = String(actionID.dropFirst("session-pin-".count))
                if let thread = viewModel.workbench?.state.sidebar.threads.first(where: { $0.id == sessionID }) {
                    let isPinned = viewModel.workbench?.isPinnedThread(sessionID) == true
                    return isPinned ? "Unpin \(thread.title)" : "Pin \(thread.title)"
                }
                return "Pin Session"
            }
            if actionID.hasPrefix("session-clear-") {
                let sessionID = String(actionID.dropFirst("session-clear-".count))
                if let thread = viewModel.workbench?.state.sidebar.threads.first(where: { $0.id == sessionID }) {
                    return "Clear \(thread.title)"
                }
                return "Clear Session"
            }
            return actionID
        }
    }

    private func settingsPanelCard<Content: View>(
        tokens: ThemeTokens,
        title: String,
        description: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.headline)
                    .foregroundStyle(tokens.primaryText)
                Text(description)
                    .font(.system(size: 12))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }

            content()
        }
        .padding(18)
        .background(tokens.panelBackground.opacity(0.88), in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(tokens.border.opacity(0.72), lineWidth: 1)
        )
    }

    private func settingsMetricPill(title: String, value: String, tokens: ThemeTokens) -> some View {
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
        .background(tokens.elevatedBackground.opacity(0.86), in: Capsule())
    }

    private func settingsValueRow(tokens: ThemeTokens, label: String, value: String) -> some View {
        HStack(alignment: .top) {
            Text(label)
                .font(.system(size: 13))
                .foregroundStyle(tokens.secondaryText)
            Spacer()
            Text(value)
                .font(.system(size: 13))
                .foregroundStyle(tokens.primaryText)
                .multilineTextAlignment(.trailing)
        }
    }

    private func settingsMonospaceValue(tokens: ThemeTokens, value: String) -> some View {
        Text(value)
            .font(.system(size: 12, design: .monospaced))
            .foregroundStyle(tokens.secondaryText)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
            .background(tokens.windowBackground.opacity(0.38), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}
