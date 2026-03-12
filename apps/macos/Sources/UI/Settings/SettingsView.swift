import AppKit
import Domain
import Features
import SwiftUI

public struct SettingsView: View {
    @Bindable private var viewModel: AppViewModel
    private let embedded: Bool
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel, embedded: Bool = false) {
        self.viewModel = viewModel
        self.embedded = embedded
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Group {
            if embedded {
                settingsContent(tokens: tokens)
            } else {
                HStack(spacing: 0) {
                    standaloneSidebar(tokens: tokens)
                        .frame(width: 260)
                        .background(tokens.sidebarBackground)
                    Divider()
                    settingsContent(tokens: tokens)
                }
            }
        }
        .background(tokens.windowBackground)
        .frame(minWidth: embedded ? 0 : 1040, minHeight: embedded ? 0 : 760)
        .task(id: viewModel.settings) {
            await viewModel.persistSettings()
        }
    }

    private func settingsContent(tokens: ThemeTokens) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(viewModel.settingsSection.title)
                        .font(.system(size: 24, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(description(for: viewModel.settingsSection))
                        .font(.system(size: 15))
                        .foregroundStyle(tokens.secondaryText)
                }

                switch viewModel.settingsSection {
                case .general:
                    generalSection(tokens: tokens)
                case .configuration:
                    configurationSection(tokens: tokens)
                case .personalization:
                    personalizationSection(tokens: tokens)
                case .markdown:
                    markdownSection(tokens: tokens)
                case .debug:
                    debugSection(tokens: tokens)
                }
            }
            .padding(.horizontal, 42)
            .padding(.vertical, 30)
            .frame(maxWidth: 920, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
    }

    private func standaloneSidebar(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Settings")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
                .padding(.horizontal, 16)
                .padding(.vertical, 18)

            Divider()
                .padding(.bottom, 8)

            VStack(alignment: .leading, spacing: 4) {
                ForEach(SettingsSection.allCases) { section in
                    Button {
                        viewModel.selectSettingsSection(section)
                    } label: {
                        HStack(spacing: 10) {
                            Image(systemName: section.symbolName)
                                .frame(width: 16)
                            Text(section.title)
                                .font(.system(size: 14, weight: .medium))
                            Spacer()
                        }
                        .foregroundStyle(viewModel.settingsSection == section ? tokens.primaryText : tokens.secondaryText)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .background(
                            (viewModel.settingsSection == section ? tokens.selection : Color.clear),
                            in: RoundedRectangle(cornerRadius: 12, style: .continuous)
                        )
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 8)

            Spacer(minLength: 0)
        }
    }

    private func generalSection(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            PanelCard {
                VStack(alignment: .leading, spacing: 0) {
                    SettingsRow(title: "Default profile", detail: "Profile used when opening new workspaces.") {
                        TextField("default", text: $viewModel.settings.defaultProfile)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 220)
                    }

                    SettingsDivider()

                    SettingsRow(title: "Selected workspace", detail: "Current project bound to the main workbench.") {
                        VStack(alignment: .trailing, spacing: 6) {
                            Text(viewModel.selectedProject?.name ?? "No workspace")
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                            Text(viewModel.selectedProject?.workspacePath ?? "Choose a workspace to begin.")
                                .font(.system(size: 12))
                                .foregroundStyle(tokens.secondaryText)
                                .lineLimit(2)
                            Button("Reveal in Finder") {
                                viewModel.revealSelectedWorkspaceInFinder()
                            }
                            .buttonStyle(.link)
                        }
                        .frame(width: 280, alignment: .trailing)
                    }

                    SettingsDivider()

                    SettingsRow(title: "Workspace library", detail: "Recent projects available in the sidebar.") {
                        HStack(spacing: 10) {
                            Text("\(viewModel.recentProjects.count)")
                                .font(.system(size: 18, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                            Text("projects")
                                .font(.system(size: 12))
                                .foregroundStyle(tokens.secondaryText)
                        }
                    }
                }
            }
        }
    }

    private func configurationSection(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            PanelCard {
                VStack(alignment: .leading, spacing: 0) {
                    SettingsRow(title: "CLI path", detail: "Override the `mosaic-cli` executable used by the workbench.") {
                        HStack(spacing: 10) {
                            TextField("/usr/local/bin/mosaic", text: Binding(
                                get: { viewModel.settings.cliPath ?? "" },
                                set: { viewModel.settings.cliPath = $0.isEmpty ? nil : $0 }
                            ))
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 280)

                            Button("Choose…", action: chooseCLI)
                                .buttonStyle(.bordered)
                        }
                    }

                    SettingsDivider()

                    SettingsRow(title: "Default workspace", detail: "Workspace path used as the initial project when available.") {
                        HStack(spacing: 10) {
                            TextField("/path/to/workspace", text: Binding(
                                get: { viewModel.settings.defaultWorkspacePath ?? "" },
                                set: { viewModel.settings.defaultWorkspacePath = $0.isEmpty ? nil : $0 }
                            ))
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 280)

                            Button("Choose…", action: chooseWorkspace)
                                .buttonStyle(.bordered)
                        }
                    }
                }
            }
        }
    }

    private func personalizationSection(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            PanelCard {
                VStack(alignment: .leading, spacing: 0) {
                    SettingsRow(title: "Theme", detail: "Use light, dark, or match your system appearance.") {
                        Picker("Theme", selection: $viewModel.settings.themeMode) {
                            ForEach(ThemeMode.allCases, id: \.self) { mode in
                                Text(mode.rawValue.capitalized).tag(mode)
                            }
                        }
                        .pickerStyle(.segmented)
                        .frame(width: 260)
                    }

                    SettingsDivider()

                    SettingsRow(title: "Interface font size", detail: "Controls the default UI font size across the workbench.") {
                        HStack(spacing: 10) {
                            Slider(value: $viewModel.settings.interfaceFontSize, in: 12...17, step: 1)
                                .frame(width: 220)
                            Text("\(Int(viewModel.settings.interfaceFontSize)) px")
                                .font(.system(size: 12))
                                .foregroundStyle(tokens.secondaryText)
                        }
                    }
                }
            }
        }
    }

    private func markdownSection(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            PanelCard {
                VStack(alignment: .leading, spacing: 0) {
                    toggleRow(
                        title: "Collapse long content",
                        detail: "Fold oversized code and text blocks by default.",
                        isOn: $viewModel.settings.markdown.collapseLongContent
                    )
                    SettingsDivider()
                    toggleRow(
                        title: "Show line numbers",
                        detail: "Render line numbers for code blocks when possible.",
                        isOn: $viewModel.settings.markdown.showLineNumbers
                    )
                    SettingsDivider()
                    toggleRow(
                        title: "Wrap code",
                        detail: "Soft-wrap long code lines inside message blocks.",
                        isOn: $viewModel.settings.markdown.wrapCode
                    )
                    SettingsDivider()
                    toggleRow(
                        title: "Render images",
                        detail: "Display remote images directly inside Markdown.",
                        isOn: $viewModel.settings.markdown.renderImages
                    )
                    SettingsDivider()
                    toggleRow(
                        title: "Highlight code",
                        detail: "Apply lightweight syntax highlighting to fenced blocks.",
                        isOn: $viewModel.settings.markdown.highlightCode
                    )
                }
            }
        }
    }

    private func debugSection(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            PanelCard {
                VStack(alignment: .leading, spacing: 0) {
                    toggleRow(
                        title: "Show raw CLI events",
                        detail: "Expose raw runtime events for debugging streaming issues.",
                        isOn: $viewModel.settings.debug.showRawCLIEvents
                    )
                    SettingsDivider()
                    toggleRow(
                        title: "Persist command logs",
                        detail: "Save command history and CLI output into local archive state.",
                        isOn: $viewModel.settings.debug.persistCommandLogs
                    )
                    SettingsDivider()
                    toggleRow(
                        title: "Echo stderr into chat",
                        detail: "Surface stderr output as system messages in the conversation.",
                        isOn: $viewModel.settings.debug.echoStdErrInChat
                    )
                }
            }

            if let error = viewModel.globalError {
                PanelCard {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Latest runtime error")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(tokens.failure)
                        Text(error)
                            .font(.system(size: 12))
                            .foregroundStyle(tokens.secondaryText)
                            .textSelection(.enabled)
                    }
                }
            }
        }
    }

    private func toggleRow(title: String, detail: String, isOn: Binding<Bool>) -> some View {
        SettingsRow(title: title, detail: detail) {
            Toggle("", isOn: isOn)
                .labelsHidden()
                .toggleStyle(.switch)
        }
    }

    private func description(for section: SettingsSection) -> String {
        switch section {
        case .general:
            "Core defaults for the workspace and thread experience."
        case .configuration:
            "Paths and runtime bindings used by Mosaic on this Mac."
        case .personalization:
            "Theme and interface density controls."
        case .markdown:
            "Rendering preferences for chat, code, logs, and previews."
        case .debug:
            "Low-level diagnostics for CLI integration and state persistence."
        }
    }

    private func chooseCLI() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK {
            viewModel.settings.cliPath = panel.url?.path
        }
    }

    private func chooseWorkspace() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK {
            viewModel.settings.defaultWorkspacePath = panel.url?.path
        }
    }
}

private struct SettingsRow<Accessory: View>: View {
    let title: String
    let detail: String
    let accessory: Accessory
    @Environment(\.colorScheme) private var colorScheme

    init(title: String, detail: String, @ViewBuilder accessory: () -> Accessory) {
        self.title = title
        self.detail = detail
        self.accessory = accessory()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(detail)
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer(minLength: 18)

            accessory
        }
        .padding(.vertical, 14)
    }
}

private struct SettingsDivider: View {
    var body: some View {
        Divider()
    }
}
