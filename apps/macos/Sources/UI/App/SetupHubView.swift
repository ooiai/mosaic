import AppKit
import Features
import SwiftUI

public struct SetupHubView: View {
    @Bindable private var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 28) {
            VStack(spacing: 16) {
                Image(systemName: "sparkles")
                    .font(.system(size: 26, weight: .medium))
                    .foregroundStyle(tokens.primaryText)
                    .frame(width: 54, height: 54)
                    .background(tokens.panelBackground, in: Circle())
                    .overlay(
                        Circle()
                            .stroke(tokens.border, lineWidth: 1)
                    )

                VStack(spacing: 6) {
                    Text("Start with a workspace")
                        .font(.system(size: 30, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text("Attach a local project folder, then Mosaic will manage threads, tasks, logs, diffs, and runtime metadata around that workspace.")
                        .font(.system(size: 15))
                        .foregroundStyle(tokens.secondaryText)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 620)
                }
            }

            Button("Choose Workspace…", action: chooseWorkspace)
                .buttonStyle(.borderedProminent)

            if !viewModel.recentProjects.isEmpty {
                PanelCard {
                    VStack(alignment: .leading, spacing: 14) {
                        SectionHeader("Recent Projects", trailing: "\(viewModel.recentProjects.count)")
                        ForEach(viewModel.recentProjects.prefix(8)) { project in
                            Button {
                                Task { await viewModel.openProject(project.id) }
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(project.name)
                                        .font(.system(size: 14, weight: .semibold))
                                        .foregroundStyle(tokens.primaryText)
                                    Text(project.workspacePath)
                                        .font(.system(size: 12))
                                        .foregroundStyle(tokens.secondaryText)
                                        .lineLimit(2)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.vertical, 6)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .frame(maxWidth: 520)
            }
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(tokens.windowBackground)
    }

    private func chooseWorkspace() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            Task { await viewModel.registerWorkspace(url: url) }
        }
    }
}
