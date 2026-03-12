import AppKit
import Features
import SwiftUI

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

        HStack(spacing: 0) {
            SidebarView(appViewModel: appViewModel, viewModel: viewModel)
                .frame(width: appViewModel.destination == .settings ? 286 : 308)
                .background(tokens.sidebarBackground)

            Divider()

            centerPane(tokens: tokens)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(tokens.windowBackground)

            if showsInspector {
                Divider()

                InspectorView(viewModel: viewModel)
                    .frame(width: 340)
                    .background(tokens.sidebarBackground)
            }
        }
        .background(tokens.windowBackground)
        .toolbar {
            ToolbarItem(placement: .principal) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(toolbarTitle)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(toolbarSubtitle)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                }
                .frame(minWidth: 320, alignment: .leading)
            }

            ToolbarItemGroup {
                Button {
                    appViewModel.presentCommandPalette()
                } label: {
                    ToolbarActionButton(systemImage: "magnifyingglass")
                }
                .buttonStyle(.plain)
                .help("Command palette")

                Menu {
                    Button("Choose Workspace…", action: chooseWorkspace)
                    Button("Reveal in Finder") {
                        appViewModel.revealSelectedWorkspaceInFinder()
                    }
                    Divider()
                    ForEach(appViewModel.recentProjects) { project in
                        Button(project.name) {
                            Task { await appViewModel.openProject(project.id) }
                        }
                    }
                } label: {
                    ToolbarCapsuleLabel(title: "Open", systemImage: "folder")
                }

                Button {
                    Task { await appViewModel.refreshActiveProject() }
                } label: {
                    ToolbarActionButton(systemImage: "arrow.clockwise")
                }
                .buttonStyle(.plain)
                .help("Refresh workspace")

                if appViewModel.destination == .thread {
                    Button {
                        appViewModel.createNewThread()
                    } label: {
                        ToolbarCapsuleLabel(title: "New thread", systemImage: "square.and.pencil")
                    }
                    .buttonStyle(.plain)

                    Button {
                        if viewModel.canCancelTask {
                            Task { await appViewModel.cancelActiveTask() }
                        } else {
                            Task { await appViewModel.retrySelectedTask() }
                        }
                    } label: {
                        ToolbarCapsuleLabel(
                            title: viewModel.canCancelTask ? "Stop" : "Retry",
                            systemImage: viewModel.canCancelTask ? "stop.fill" : "arrow.clockwise",
                            accent: viewModel.canCancelTask ? tokens.failure : nil
                        )
                    }
                    .buttonStyle(.plain)

                    Button {
                        appViewModel.toggleConsoleDrawer()
                    } label: {
                        ToolbarActionButton(
                            systemImage: appViewModel.isConsoleDrawerVisible ? "rectangle.bottomthird.inset.filled" : "rectangle.bottomthird.inset",
                            accent: appViewModel.isConsoleDrawerVisible ? tokens.accent : nil
                        )
                    }
                    .buttonStyle(.plain)
                    .help("Toggle console")

                    Button {
                        appViewModel.toggleInspector()
                    } label: {
                        ToolbarActionButton(
                            systemImage: viewModel.isInspectorVisible ? "sidebar.right" : "sidebar.left",
                            accent: viewModel.isInspectorVisible ? tokens.accent : nil
                        )
                    }
                    .buttonStyle(.plain)
                    .help("Toggle inspector")
                }

                Button {
                    appViewModel.showSettings()
                } label: {
                    ToolbarActionButton(
                        systemImage: "gearshape",
                        accent: appViewModel.destination == .settings ? tokens.accent : nil
                    )
                }
                .buttonStyle(.plain)
                .help("Settings")
            }
        }
    }

    @ViewBuilder
    private func centerPane(tokens: ThemeTokens) -> some View {
        switch appViewModel.destination {
        case .thread:
            ConversationView(appViewModel: appViewModel, viewModel: viewModel)
        case .automations:
            AutomationTemplatesView(appViewModel: appViewModel)
        case .skills:
            SkillsCatalogView(appViewModel: appViewModel)
        case .settings:
            SettingsView(viewModel: appViewModel, embedded: true)
        }
    }

    private var showsInspector: Bool {
        appViewModel.destination == .thread && viewModel.isInspectorVisible
    }

    private var toolbarTitle: String {
        switch appViewModel.destination {
        case .thread:
            return viewModel.selectedSession?.title ?? "New thread"
        default:
            return appViewModel.destination.title
        }
    }

    private var toolbarSubtitle: String {
        switch appViewModel.destination {
        case .thread:
            return viewModel.project.name
        case .automations:
            return "Reusable scheduled workflows for \(viewModel.project.name)"
        case .skills:
            return "Installed and recommended capabilities"
        case .settings:
            return viewModel.project.name
        }
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

private struct ToolbarCapsuleLabel: View {
    let title: String
    let systemImage: String
    let accent: Color?
    @Environment(\.colorScheme) private var colorScheme

    init(title: String, systemImage: String, accent: Color? = nil) {
        self.title = title
        self.systemImage = systemImage
        self.accent = accent
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 8) {
            Image(systemName: systemImage)
                .font(.system(size: 12, weight: .semibold))
            Text(title)
                .font(.system(size: 13, weight: .medium))
        }
        .foregroundStyle(accent ?? tokens.primaryText)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(tokens.elevatedBackground, in: Capsule())
        .overlay(
            Capsule()
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}
