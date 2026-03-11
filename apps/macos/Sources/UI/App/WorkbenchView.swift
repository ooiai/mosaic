import Features
import SwiftUI

public struct WorkbenchView: View {
    @Bindable private var appViewModel: AppViewModel
    @Bindable private var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openSettings) private var openSettings

    public init(appViewModel: AppViewModel, viewModel: WorkbenchViewModel) {
        self.appViewModel = appViewModel
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        NavigationSplitView {
            SidebarView(appViewModel: appViewModel, viewModel: viewModel)
                .background(tokens.sidebarBackground)
                .navigationSplitViewColumnWidth(min: 260, ideal: 300)
        } content: {
            ConversationView(appViewModel: appViewModel, viewModel: viewModel)
                .navigationSplitViewColumnWidth(min: 620, ideal: 860)
        } detail: {
            if viewModel.isInspectorVisible {
                InspectorView(viewModel: viewModel)
                    .navigationSplitViewColumnWidth(min: 300, ideal: 340)
            } else {
                ThemeTokens.current(for: colorScheme).windowBackground
            }
        }
        .toolbar {
            ToolbarItem(placement: .principal) {
                HStack(spacing: 14) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(viewModel.project.name)
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                        HStack(spacing: 8) {
                            Text(viewModel.currentProviderLabel.uppercased())
                            Text("•")
                            Text(viewModel.currentModelLabel.uppercased())
                            Text("•")
                            Text(viewModel.currentHealthLabel.uppercased())
                        }
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    }
                    Spacer(minLength: 16)
                    MetricChip(title: "Sessions", value: "\(viewModel.sessions.count)", accent: tokens.accent)
                    MetricChip(title: "Tasks", value: "\(viewModel.recentTasks.count)", accent: tokens.success)
                }
                .frame(minWidth: 420)
            }

            ToolbarItemGroup {
                Button {
                    appViewModel.presentCommandPalette()
                } label: {
                    ToolbarActionButton(systemImage: "magnifyingglass")
                }
                .help("Command palette")

                Button {
                    appViewModel.createNewThread()
                } label: {
                    ToolbarActionButton(systemImage: "square.and.pencil", accent: tokens.accent)
                }
                .help("New thread")

                Button {
                    Task { await appViewModel.refreshActiveProject() }
                } label: {
                    ToolbarActionButton(systemImage: "arrow.clockwise")
                }
                .help("Refresh project")

                Button {
                    if viewModel.canCancelTask {
                        Task { await appViewModel.cancelActiveTask() }
                    } else {
                        Task { await appViewModel.retrySelectedTask() }
                    }
                } label: {
                    ToolbarActionButton(
                        systemImage: viewModel.canCancelTask ? "stop.fill" : "arrow.clockwise.circle",
                        accent: viewModel.canCancelTask ? tokens.failure : tokens.warning
                    )
                }
                .help(viewModel.canCancelTask ? "Stop active task" : "Retry selected task")

                Button {
                    appViewModel.toggleInspector()
                } label: {
                    ToolbarActionButton(systemImage: viewModel.isInspectorVisible ? "sidebar.right" : "sidebar.left")
                }
                .help("Toggle inspector")

                Button {
                    openSettings()
                } label: {
                    ToolbarActionButton(systemImage: "gearshape")
                }
                .help("Settings")
            }
        }
    }
}
