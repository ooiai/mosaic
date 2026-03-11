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

        HStack(spacing: 24) {
            EmptyStateCard(
                eyebrow: "AI Agent Desktop",
                title: "Start with a workspace, not a blank chat.",
                detail: "Attach a local project folder, then Mosaic will manage sessions, tasks, logs, diffs, and runtime metadata around that workspace.",
                actionTitle: "Choose Workspace…"
            ) {
                let panel = NSOpenPanel()
                panel.canChooseDirectories = true
                panel.canChooseFiles = false
                panel.allowsMultipleSelection = false
                if panel.runModal() == .OK, let url = panel.url {
                    Task { await viewModel.registerWorkspace(url: url) }
                }
            }
            .frame(maxWidth: 520, alignment: .leading)

            PanelCard {
                VStack(alignment: .leading, spacing: 14) {
                    SectionHeader("Recent Projects", trailing: "\(viewModel.recentProjects.count)")
                    if viewModel.recentProjects.isEmpty {
                        Text("No recent projects yet.")
                            .font(.system(size: 13))
                            .foregroundStyle(tokens.secondaryText)
                    } else {
                        ForEach(viewModel.recentProjects.prefix(10)) { project in
                            Button {
                                Task { await viewModel.openProject(project.id) }
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(project.name)
                                        .font(.system(size: 13, weight: .semibold))
                                        .foregroundStyle(tokens.primaryText)
                                    Text(project.workspacePath)
                                        .font(.system(size: 11))
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
            }
            .frame(maxWidth: 380)
        }
        .padding(36)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(tokens.windowBackground)
    }
}
