import AppKit
import Features
import SwiftUI

struct ComposerDock: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    private var showsSuggestions: Bool {
        viewModel.selectedMessages.isEmpty
    }

    private var contextChips: [String] {
        var items = [viewModel.project.name]
        if let session = viewModel.selectedSession?.title, !session.isEmpty {
            items.append(session)
        }
        if let task = viewModel.selectedTask?.title, !task.isEmpty {
            items.append(task)
        }
        return Array(items.prefix(3))
    }

    private var fileContextChips: [String] {
        guard let task = viewModel.selectedTask else { return [] }
        return Array(task.fileChanges.map(\.path).prefix(4))
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 12) {
            if showsSuggestions {
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
                .frame(maxWidth: 760)
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
                .frame(maxWidth: 760)
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

                    TextEditor(text: $viewModel.composerText)
                        .font(.system(size: appViewModel.settings.interfaceFontSize))
                        .foregroundStyle(tokens.primaryText)
                        .frame(minHeight: 112, maxHeight: 180)
                        .scrollContentBackground(.hidden)
                }

                Divider()
                    .padding(.top, 10)
                    .padding(.bottom, 10)

                HStack(spacing: 10) {
                    Menu {
                        Button("Choose Workspace…", action: chooseWorkspace)
                        Button("New thread") {
                            appViewModel.createNewThread()
                        }
                    } label: {
                        Image(systemName: "plus")
                            .font(.system(size: 15, weight: .medium))
                            .foregroundStyle(tokens.secondaryText)
                            .frame(width: 20, height: 20)
                    }
                    .menuStyle(.borderlessButton)
                    .fixedSize()

                    Menu {
                        ForEach(viewModel.profileChoices, id: \.self) { profile in
                            Button(profile) {
                                Task { await appViewModel.selectProfile(profile) }
                            }
                        }
                    } label: {
                        ComposerMenuLabel(title: viewModel.selectedProfile, systemImage: "person.crop.circle")
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

                    Menu {
                        Button(viewModel.currentModelLabel) {}
                    } label: {
                        ComposerMenuLabel(title: viewModel.currentModelLabel, systemImage: "sparkles")
                    }

                    Spacer()

                    Text("⌘↩ send")
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)

                    if viewModel.canCancelTask {
                        Button {
                            Task { await appViewModel.cancelActiveTask() }
                        } label: {
                            Image(systemName: "stop.fill")
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(tokens.failure)
                                .frame(width: 34, height: 34)
                                .background(tokens.elevatedBackground, in: Circle())
                        }
                        .buttonStyle(.plain)
                    }

                    Button {
                        Task { await appViewModel.sendCurrentPrompt() }
                    } label: {
                        Image(systemName: viewModel.canCancelTask ? "ellipsis" : "arrow.up")
                            .font(.system(size: 14, weight: .bold))
                            .foregroundStyle(Color.white)
                            .frame(width: 34, height: 34)
                            .background(appViewModel.canSendPrompt ? tokens.primaryText : tokens.tertiaryText, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .disabled(!appViewModel.canSendPrompt)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .frame(maxWidth: 760)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )

            HStack(spacing: 10) {
                ForEach(contextChips, id: \.self) { chip in
                    FooterPill(title: chip, systemImage: chip == viewModel.project.name ? "laptopcomputer" : "text.bubble")
                }
                FooterPill(title: viewModel.currentProviderLabel, systemImage: "externaldrive.badge.wifi")
                FooterPill(title: viewModel.selectedProfile, systemImage: "shield.lefthalf.filled")
                Spacer()
                FooterPill(title: viewModel.selectedSession?.state.rawValue.capitalized ?? "Idle", systemImage: "point.bottomleft.forward.to.point.topright.scurvepath")
            }
            .frame(maxWidth: 760)
        }
        .frame(maxWidth: .infinity)
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

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Image(systemName: systemImage)
            Text(title)
            Image(systemName: "chevron.down")
                .font(.system(size: 10, weight: .semibold))
        }
        .font(.system(size: 13, weight: .medium))
        .foregroundStyle(tokens.secondaryText)
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
        .background(tokens.panelBackground, in: Capsule())
        .overlay(
            Capsule()
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}
