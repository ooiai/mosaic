import AppKit
import Features
import SwiftUI

public struct WorkbenchView: View {
    @Bindable private var appViewModel: AppViewModel
    @Bindable private var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var sidebarDragStartWidth: CGFloat?
    @State private var inspectorDragStartWidth: CGFloat?

    public init(appViewModel: AppViewModel, viewModel: WorkbenchViewModel) {
        self.appViewModel = appViewModel
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 0) {
            SidebarView(appViewModel: appViewModel, viewModel: viewModel)
                .frame(width: appViewModel.sidebarWidth)
                .background(tokens.sidebarBackground)

            SplitterHandle()
                .gesture(sidebarResizeGesture)

            centerPane(tokens: tokens)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(tokens.windowBackground)

            if showsInspector {
                SplitterHandle()
                    .gesture(inspectorResizeGesture)

                InspectorView(viewModel: viewModel)
                    .frame(width: appViewModel.inspectorWidth)
                    .background(tokens.sidebarBackground)
            }
        }
        .background(tokens.windowBackground)
        .toolbar {
            ToolbarItem(placement: .principal) {
                toolbarPrincipal(tokens: tokens)
            }

            ToolbarItem(placement: .automatic) {
                HStack(spacing: 8) {
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
                    .buttonStyle(.plain)

                    if appViewModel.destination == .thread, viewModel.canCancelTask {
                        Button {
                            Task { await appViewModel.cancelActiveTask() }
                        } label: {
                            ToolbarCapsuleLabel(
                                title: "Stop",
                                systemImage: "stop.fill",
                                accent: tokens.failure,
                                isEnabled: true
                            )
                        }
                        .buttonStyle(.plain)
                    }

                    if appViewModel.destination == .thread {
                        Button {} label: {
                            ToolbarCapsuleLabel(
                                title: "Handoff",
                                systemImage: "arrow.left.arrow.right",
                                isEnabled: false
                            )
                        }
                        .buttonStyle(.plain)
                        .disabled(true)

                        Button {} label: {
                            ToolbarCapsuleLabel(
                                title: "Commit",
                                systemImage: "circle.dashed",
                                isEnabled: false
                            )
                        }
                        .buttonStyle(.plain)
                        .disabled(true)
                    }

                    ToolbarSeparatorView()

                    Button {
                        appViewModel.presentCommandPalette()
                    } label: {
                        ToolbarActionButton(systemImage: "magnifyingglass")
                    }
                    .buttonStyle(.plain)
                    .help("Command palette")

                    if appViewModel.destination == .thread {
                        Button {
                            Task { await appViewModel.refreshActiveProject() }
                        } label: {
                            ToolbarActionButton(
                                systemImage: "arrow.clockwise",
                                isEnabled: !viewModel.isLoadingSnapshot
                            )
                        }
                        .buttonStyle(.plain)
                        .disabled(viewModel.isLoadingSnapshot)
                        .help("Refresh workspace")

                        ToolbarSeparatorView()

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

                    ToolbarSeparatorView()

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

    @ViewBuilder
    private func toolbarPrincipal(tokens: ThemeTokens) -> some View {
        if appViewModel.destination == .thread {
            HStack(spacing: 7) {
                Text(toolbarTitle)
                    .font(.system(size: 13.5, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)

                Text(toolbarSubtitle)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(tokens.secondaryText)
                    .lineLimit(1)

                Image(systemName: "ellipsis")
                    .font(.system(size: 9.5, weight: .bold))
                    .foregroundStyle(tokens.tertiaryText.opacity(0.9))
            }
            .frame(minWidth: 292, alignment: .leading)
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Text(toolbarTitle)
                    .font(.system(size: 13.5, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                Text(toolbarSubtitle)
                    .font(.system(size: 10.5))
                    .foregroundStyle(tokens.secondaryText)
            }
            .frame(minWidth: 264, alignment: .leading)
        }
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

    private var sidebarResizeGesture: some Gesture {
        DragGesture(minimumDistance: 2)
            .onChanged { value in
                let startWidth = sidebarDragStartWidth ?? appViewModel.sidebarWidth
                if sidebarDragStartWidth == nil {
                    sidebarDragStartWidth = startWidth
                }
                appViewModel.setSidebarWidth(startWidth + value.translation.width)
            }
            .onEnded { value in
                let startWidth = sidebarDragStartWidth ?? appViewModel.sidebarWidth
                appViewModel.setSidebarWidth(startWidth + value.translation.width, persist: true)
                sidebarDragStartWidth = nil
            }
    }

    private var inspectorResizeGesture: some Gesture {
        DragGesture(minimumDistance: 2)
            .onChanged { value in
                let startWidth = inspectorDragStartWidth ?? appViewModel.inspectorWidth
                if inspectorDragStartWidth == nil {
                    inspectorDragStartWidth = startWidth
                }
                appViewModel.setInspectorWidth(startWidth - value.translation.width)
            }
            .onEnded { value in
                let startWidth = inspectorDragStartWidth ?? appViewModel.inspectorWidth
                appViewModel.setInspectorWidth(startWidth - value.translation.width, persist: true)
                inspectorDragStartWidth = nil
            }
    }
}

private struct ToolbarCapsuleLabel: View {
    let title: String
    let systemImage: String
    let accent: Color?
    let isEnabled: Bool
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    init(title: String, systemImage: String, accent: Color? = nil, isEnabled: Bool = true) {
        self.title = title
        self.systemImage = systemImage
        self.accent = accent
        self.isEnabled = isEnabled
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 7) {
            Image(systemName: systemImage)
                .font(.system(size: 10, weight: .semibold))
            Text(title)
                .font(.system(size: 11, weight: .medium))
        }
        .foregroundStyle(isEnabled ? (accent ?? tokens.primaryText) : tokens.tertiaryText)
        .padding(.horizontal, 8)
        .padding(.vertical, 3.5)
        .background((isHovered && isEnabled ? tokens.panelBackground.opacity(0.92) : tokens.elevatedBackground.opacity(0.96)), in: Capsule())
        .overlay(
            Capsule()
                .stroke(tokens.border, lineWidth: 1)
        )
        .opacity(isEnabled ? 1 : 0.62)
        .onHover { isHovered = $0 }
    }
}

private struct ToolbarSeparatorView: View {
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Rectangle()
            .fill(tokens.border)
            .frame(width: 1, height: 14)
            .opacity(0.72)
            .padding(.horizontal, 1)
    }
}

private struct SplitterHandle: View {
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovering = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Rectangle()
            .fill(isHovering ? tokens.accent.opacity(0.22) : Color.clear)
            .frame(width: 6)
            .overlay(
                Rectangle()
                    .fill(isHovering ? tokens.accent.opacity(0.55) : tokens.border)
                    .frame(width: isHovering ? 2 : 1)
            )
            .contentShape(Rectangle())
            .onHover { isHovering = $0 }
            .help("Drag to resize")
    }
}
