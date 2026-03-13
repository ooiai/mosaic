import AppKit
import Features
import SwiftUI

struct ComposerDock: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var suggestionsDismissed = false

    private var showsSuggestions: Bool {
        viewModel.selectedMessages.isEmpty && !suggestionsDismissed
    }

    private var fileContextChips: [String] {
        guard let task = viewModel.selectedTask else { return [] }
        return Array(task.fileChanges.map(\.path).prefix(4))
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 12) {
            if showsSuggestions {
                VStack(spacing: 10) {
                    HStack {
                        Spacer()
                        Text("Explore more")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(tokens.secondaryText)
                        Button {
                            suggestionsDismissed = true
                        } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(tokens.tertiaryText)
                        }
                        .buttonStyle(.plain)
                    }

                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 12) {
                            ForEach(WorkbenchReferenceContent.composerSuggestions) { suggestion in
                                SuggestionPromptCard(
                                    title: suggestion.title,
                                    symbolName: suggestion.symbolName,
                                    tint: suggestion.tint
                                ) {
                                    appViewModel.seedComposer(with: suggestion.prompt)
                                }
                            }
                        }
                        .padding(.horizontal, 2)
                    }
                    .frame(maxWidth: WorkbenchChromeMetrics.threadContentWidth)
                }
                .frame(maxWidth: WorkbenchChromeMetrics.threadContentWidth)
            }

            if !fileContextChips.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(fileContextChips, id: \.self) { chip in
                            ContextChip(title: chip, systemImage: "doc")
                        }
                    }
                    .padding(.horizontal, 2)
                }
                .frame(maxWidth: WorkbenchChromeMetrics.threadContentWidth)
            }

            VStack(alignment: .leading, spacing: 0) {
                ZStack(alignment: .topLeading) {
                    if viewModel.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("Ask Mosaic anything, @ to add files, / for commands")
                            .font(.system(size: 15))
                            .foregroundStyle(tokens.tertiaryText)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 8)
                    }

                    ComposerInputTextView(
                        text: $viewModel.composerText,
                        fontSize: appViewModel.settings.interfaceFontSize
                    ) {
                        Task { await appViewModel.sendCurrentPrompt() }
                    }
                    .frame(minHeight: 96, maxHeight: 168)
                }

                Divider()
                    .padding(.top, 7)
                    .padding(.bottom, 7)

                HStack(spacing: 10) {
                    Menu {
                        Button("Choose Workspace…", action: chooseWorkspace)
                        Button("New thread") {
                            appViewModel.createNewThread()
                        }
                    } label: {
                        ComposerIconButton(systemImage: "plus")
                    }
                    .menuStyle(.borderlessButton)
                    .fixedSize()

                    Menu {
                        ForEach(viewModel.availableModelChoices, id: \.self) { model in
                            Button(model) {
                                Task { await viewModel.selectModel(model) }
                            }
                        }
                    } label: {
                        ComposerMenuLabel(title: viewModel.currentModelLabel, systemImage: "sparkles")
                    }

                    Menu {
                        ForEach(ComposerMode.allCases) { mode in
                            Button(mode.title) {
                                viewModel.composerMode = mode
                            }
                        }
                    } label: {
                        ComposerMenuLabel(title: viewModel.composerMode.title, systemImage: "bolt")
                    }

                    Spacer()

                    Text("⌘↩ send")
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)

                    if viewModel.canCancelTask {
                        ComposerRoundButton(accent: tokens.failure) {
                            Task { await appViewModel.cancelActiveTask() }
                        } label: {
                            Image(systemName: "stop.fill")
                        }
                    }

                    ComposerRoundButton(
                        foreground: .white,
                        background: appViewModel.canSendPrompt ? tokens.primaryText : tokens.tertiaryText
                    ) {
                        Task { await appViewModel.sendCurrentPrompt() }
                    } label: {
                        Image(systemName: viewModel.canCancelTask ? "ellipsis" : "arrow.up")
                    }
                    .disabled(!appViewModel.canSendPrompt)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .frame(maxWidth: WorkbenchChromeMetrics.composerWidth)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
            .shadow(color: colorScheme == .light ? Color.black.opacity(0.04) : .clear, radius: 18, y: 10)

            HStack(spacing: 12) {
                Menu {
                    Button("Choose Workspace…", action: chooseWorkspace)
                    Button("Reveal in Finder") {
                        appViewModel.revealSelectedWorkspaceInFinder()
                    }
                } label: {
                    FooterBarMenuLabel(
                        title: viewModel.currentProviderLabel,
                        systemImage: viewModel.currentProviderLabel == "Local" ? "laptopcomputer" : "network"
                    )
                }
                .menuStyle(.borderlessButton)

                Menu {
                    ForEach(viewModel.profileChoices, id: \.self) { profile in
                        Button(profile) {
                            Task { await appViewModel.selectProfile(profile) }
                        }
                    }
                    Divider()
                    Button("Open Settings") {
                        appViewModel.showSettings(section: .general)
                    }
                } label: {
                    FooterBarMenuLabel(
                        title: profileFooterTitle,
                        systemImage: "checkmark.shield"
                    )
                }
                .menuStyle(.borderlessButton)

                Spacer()

                Menu {
                    Button("Refresh Workspace") {
                        Task { await appViewModel.refreshActiveProject() }
                    }
                    Button("Reveal in Finder") {
                        appViewModel.revealSelectedWorkspaceInFinder()
                    }
                } label: {
                    FooterBarMenuLabel(
                        title: viewModel.currentBranchLabel,
                        systemImage: "point.bottomleft.forward.to.point.topright.scurvepath"
                    )
                }
                .menuStyle(.borderlessButton)
            }
            .frame(maxWidth: WorkbenchChromeMetrics.composerWidth)
        }
        .frame(maxWidth: .infinity)
        .onChange(of: viewModel.selectedSessionID) {
            suggestionsDismissed = false
        }
    }

    private var profileFooterTitle: String {
        let profile = viewModel.selectedProfile
        return profile == "default" ? "Default profile" : profile
    }

    private func chooseWorkspace() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            Task { await appViewModel.registerWorkspace(url: url) }
        }
    }
}

private struct ComposerMenuLabel: View {
    let title: String
    let systemImage: String
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Image(systemName: systemImage)
            Text(title)
            Image(systemName: "chevron.down")
                .font(.system(size: 10, weight: .semibold))
        }
        .font(.system(size: 12, weight: .medium))
        .foregroundStyle(tokens.secondaryText)
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background((isHovered ? tokens.elevatedBackground : Color.clear), in: Capsule())
        .onHover { isHovered = $0 }
    }
}

private struct FooterBarMenuLabel: View {
    let title: String
    let systemImage: String
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Image(systemName: systemImage)
            Text(title)
                .lineLimit(1)
            Image(systemName: "chevron.down")
                .font(.system(size: 9, weight: .semibold))
        }
        .font(.system(size: 11, weight: .medium))
        .foregroundStyle(tokens.secondaryText)
        .padding(.horizontal, 2)
        .padding(.vertical, 2)
        .background((isHovered ? tokens.elevatedBackground.opacity(0.72) : Color.clear), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(isHovered ? tokens.border : .clear, lineWidth: 1)
        )
        .onHover { isHovered = $0 }
    }
}

private struct ContextChip: View {
    let title: String
    let systemImage: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Image(systemName: systemImage)
            Text(title)
                .lineLimit(1)
        }
        .font(.system(size: 11, weight: .medium))
        .foregroundStyle(tokens.secondaryText)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(tokens.elevatedBackground, in: Capsule())
        .overlay(
            Capsule()
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}

private struct ComposerIconButton: View {
    let systemImage: String
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Image(systemName: systemImage)
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(tokens.secondaryText)
            .frame(width: 22, height: 22)
            .background((isHovered ? tokens.elevatedBackground : Color.clear), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .onHover { isHovered = $0 }
    }
}

private struct ComposerRoundButton<Label: View>: View {
    let foreground: Color
    let background: Color
    let usesFilledBackground: Bool
    let action: () -> Void
    let label: Label
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    init(
        foreground: Color? = nil,
        background: Color? = nil,
        accent: Color? = nil,
        action: @escaping () -> Void,
        @ViewBuilder label: () -> Label
    ) {
        self.foreground = foreground ?? accent ?? .primary
        self.background = background ?? Color.clear
        self.usesFilledBackground = background != nil
        self.action = action
        self.label = label()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            label
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(foreground)
                .frame(width: 32, height: 32)
                .background((isHovered ? hoverBackground(tokens: tokens) : background), in: Circle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }

    private func hoverBackground(tokens: ThemeTokens) -> Color {
        if !usesFilledBackground {
            return tokens.elevatedBackground
        }
        return background.opacity(0.92)
    }
}
