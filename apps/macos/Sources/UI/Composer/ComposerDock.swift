import Features
import SwiftUI

struct ComposerDock: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        PanelCard {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 10) {
                    Picker("Mode", selection: $viewModel.composerMode) {
                        ForEach(ComposerMode.allCases) { mode in
                            Text(mode.title).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .frame(maxWidth: 260)

                    Menu {
                        ForEach(appViewModel.recentProjects) { project in
                            Button(project.name) {
                                Task { await appViewModel.openProject(project.id) }
                            }
                        }
                    } label: {
                        MetricChip(title: "Workspace", value: viewModel.project.name, accent: tokens.accent)
                    }

                    Menu {
                        ForEach(viewModel.profileChoices, id: \.self) { profile in
                            Button(profile) {
                                Task { await appViewModel.selectProfile(profile) }
                            }
                        }
                    } label: {
                        MetricChip(title: "Profile", value: viewModel.selectedProfile, accent: tokens.success)
                    }

                    Spacer()

                    if viewModel.canCancelTask {
                        Button {
                            Task { await appViewModel.cancelActiveTask() }
                        } label: {
                            Label("Stop", systemImage: "stop.fill")
                        }
                        .buttonStyle(.bordered)
                    }
                }

                ZStack(alignment: .topLeading) {
                    if viewModel.composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("Describe the next concrete task, review, or implementation step for this workspace…")
                            .foregroundStyle(tokens.tertiaryText)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 8)
                    }

                    TextEditor(text: $viewModel.composerText)
                        .font(.system(size: appViewModel.settings.interfaceFontSize))
                        .frame(minHeight: 120)
                        .scrollContentBackground(.hidden)
                }

                HStack(spacing: 8) {
                    Text("CMD+ENTER")
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Button("New Thread") {
                        appViewModel.createNewThread()
                    }
                    .buttonStyle(.borderless)

                    Button {
                        Task { await appViewModel.sendCurrentPrompt() }
                    } label: {
                        Label(viewModel.canCancelTask ? "Running…" : "Send", systemImage: "arrow.up")
                            .font(.system(size: 12, weight: .semibold))
                            .padding(.horizontal, 14)
                            .padding(.vertical, 8)
                            .background(tokens.accent, in: Capsule())
                            .foregroundStyle(Color.white)
                    }
                    .buttonStyle(.plain)
                    .disabled(!appViewModel.canSendPrompt)
                }
            }
        }
    }
}
