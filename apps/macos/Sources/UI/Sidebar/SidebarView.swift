import AppKit
import Domain
import Features
import SwiftUI

struct SidebarView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var isRuntimeMenuPresented = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Group {
            if appViewModel.destination == .settings {
                settingsSidebar(tokens: tokens)
            } else {
                globalSidebar(tokens: tokens)
            }
        }
        .background(tokens.sidebarBackground)
    }

    private func globalSidebar(tokens: ThemeTokens) -> some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    VStack(spacing: 3) {
                        SidebarNavButton(
                            title: "New thread",
                            systemImage: "square.and.pencil",
                            isSelected: appViewModel.destination == .thread
                        ) {
                            appViewModel.createNewThread()
                        }

                        SidebarNavButton(
                            title: "Automations",
                            systemImage: "clock.arrow.circlepath",
                            isSelected: appViewModel.destination == .automations
                        ) {
                            appViewModel.navigate(to: .automations)
                        }

                        SidebarNavButton(
                            title: "Skills",
                            systemImage: "square.grid.2x2",
                            isSelected: appViewModel.destination == .skills
                        ) {
                            appViewModel.navigate(to: .skills)
                        }
                    }

                    sidebarSectionHeader("Workspaces", tokens: tokens) {
                        SidebarHeaderIconButton(systemImage: "folder.badge.plus", tokens: tokens, action: chooseWorkspace)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(appViewModel.recentProjects.prefix(6)) { project in
                            ProjectRow(
                                project: project,
                                isSelected: project.id == viewModel.project.id,
                                tokens: tokens
                            ) {
                                Task { await appViewModel.openProject(project.id) }
                            }
                        }
                    }

                    sidebarSectionHeader("Threads", tokens: tokens) {
                        HStack(spacing: 10) {
                            SidebarHeaderIconButton(systemImage: "plus", tokens: tokens) {
                                appViewModel.createNewThread()
                            }

                            SidebarHeaderIconButton(systemImage: "line.3.horizontal.decrease", tokens: tokens) {
                                appViewModel.presentCommandPalette()
                            }
                        }
                    }

                    SidebarSearchField(text: $viewModel.sidebarQuery, tokens: tokens)

                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(viewModel.filteredSessions) { session in
                            ThreadRow(
                                session: session,
                                latestTask: viewModel.tasks.first(where: { $0.id == session.latestTaskID }),
                                isSelected: session.id == viewModel.selectedSessionID && appViewModel.destination == .thread,
                                accentColor: color(for: session.state, tokens: tokens),
                                onSelect: {
                                    appViewModel.openSession(session.id)
                                },
                                onTogglePinned: {
                                    Task { await viewModel.togglePinned(sessionID: session.id) }
                                }
                            )
                        }
                    }

                    if !viewModel.recentTasks.isEmpty {
                        sidebarSectionHeader("Recent tasks", tokens: tokens)

                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(viewModel.recentTasks.prefix(4)) { task in
                                RecentTaskRow(task: task, tokens: tokens) {
                                    appViewModel.navigate(to: .thread)
                                    viewModel.selectTask(task.id)
                                    viewModel.inspectorPanel = .overview
                                }
                            }
                        }
                    }
                }
                .padding(10)
                .padding(.top, 8)
            }

            Divider()

            Button {
                isRuntimeMenuPresented = true
            } label: {
                SidebarFooterSettingsButton()
            }
            .buttonStyle(.plain)
            .padding(10)
            .popover(isPresented: $isRuntimeMenuPresented, arrowEdge: .bottom) {
                RuntimeSettingsPopover(
                    appViewModel: appViewModel,
                    viewModel: viewModel,
                    isPresented: $isRuntimeMenuPresented
                )
            }
        }
    }

    private func settingsSidebar(tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                appViewModel.navigate(to: .thread)
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: "chevron.left")
                    Text("Back to app")
                        .font(.system(size: 14, weight: .medium))
                }
                .foregroundStyle(tokens.secondaryText)
                .padding(.horizontal, 12)
                .padding(.vertical, 12)
            }
            .buttonStyle(.plain)

            Divider()
                .padding(.bottom, 8)

            VStack(alignment: .leading, spacing: 4) {
                ForEach(SettingsSection.allCases) { section in
                    Button {
                        appViewModel.selectSettingsSection(section)
                    } label: {
                        HStack(spacing: 10) {
                            Image(systemName: section.symbolName)
                                .frame(width: 16)
                            Text(section.title)
                                .font(.system(size: 14, weight: .medium))
                            Spacer()
                        }
                        .foregroundStyle(appViewModel.settingsSection == section ? tokens.primaryText : tokens.secondaryText)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .background(
                            (appViewModel.settingsSection == section ? tokens.selection : Color.clear),
                            in: RoundedRectangle(cornerRadius: 12, style: .continuous)
                        )
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 8)

            Spacer(minLength: 0)
        }
        .padding(.top, 8)
    }

    private func sidebarSectionHeader<Accessory: View>(
        _ title: String,
        tokens: ThemeTokens,
        @ViewBuilder accessory: () -> Accessory
    ) -> some View {
        HStack {
            Text(title)
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            Spacer()
            accessory()
        }
    }

    private func sidebarSectionHeader(_ title: String, tokens: ThemeTokens) -> some View {
        sidebarSectionHeader(title, tokens: tokens) { EmptyView() }
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

    private func color(for state: SessionState, tokens: ThemeTokens) -> Color {
        switch state {
        case .idle: tokens.tertiaryText
        case .waiting: tokens.warning
        case .running: tokens.accent
        case .failed: tokens.failure
        case .cancelled: tokens.warning
        case .done: tokens.success
        }
    }

    private func relativeDate(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: .now)
    }
}

private struct ProjectRow: View {
    let project: Project
    let isSelected: Bool
    let tokens: ThemeTokens
    let action: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: "folder")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(tokens.secondaryText)
                Text(project.name)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(background, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(isSelected ? tokens.border : .clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }

    private var background: Color {
        if isSelected { return tokens.selection }
        if isHovered { return tokens.panelBackground.opacity(0.75) }
        return .clear
    }
}

private struct ThreadRow: View {
    let session: Session
    let latestTask: AgentTask?
    let isSelected: Bool
    let accentColor: Color
    let onSelect: () -> Void
    let onTogglePinned: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let additions = latestTask?.fileChanges.reduce(0) { $0 + $1.additions } ?? 0
        let deletions = latestTask?.fileChanges.reduce(0) { $0 + $1.deletions } ?? 0

        Button(action: onSelect) {
            VStack(alignment: .leading, spacing: 5) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(session.title)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                        .lineLimit(1)
                    if session.isPinned {
                        Image(systemName: "pin.fill")
                            .font(.system(size: 9, weight: .semibold))
                            .foregroundStyle(tokens.warning)
                    }
                    Spacer()
                    Text(relativeDate(session.updatedAt))
                        .font(.system(size: 10))
                        .foregroundStyle(tokens.tertiaryText)
                }

                HStack(alignment: .firstTextBaseline, spacing: 7) {
                    Circle()
                        .fill(accentColor)
                        .frame(width: 4, height: 4)
                    Text(session.summary.isEmpty ? "No summary yet" : session.summary)
                        .font(.system(size: 11))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(1)
                    Spacer(minLength: 8)
                    if additions > 0 {
                        Text("+\(additions)")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(tokens.success)
                    }
                    if deletions > 0 {
                        Text("-\(deletions)")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(tokens.failure)
                    }
                    if additions == 0 && deletions == 0 {
                        Text("\(session.messageCount) msgs")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(tokens.tertiaryText)
                    } else {
                        Text("\(session.taskCount)t")
                            .font(.system(size: 10, weight: .medium))
                            .foregroundStyle(tokens.tertiaryText)
                    }
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(background(tokens: tokens), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(isSelected ? tokens.border : .clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .contextMenu {
            Button(session.isPinned ? "Unpin" : "Pin") {
                onTogglePinned()
            }
        }
    }

    private func background(tokens: ThemeTokens) -> Color {
        if isSelected { return tokens.selection }
        if isHovered { return tokens.panelBackground.opacity(0.75) }
        return .clear
    }

    private func relativeDate(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: .now)
    }
}

private struct RecentTaskRow: View {
    let task: AgentTask
    let tokens: ThemeTokens
    let action: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 10) {
                Circle()
                    .fill(color)
                    .frame(width: 7, height: 7)
                    .padding(.top, 5)

                VStack(alignment: .leading, spacing: 3) {
                    Text(task.title)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                        .lineLimit(2)
                    Text(task.summary.isEmpty ? task.prompt : task.summary)
                        .font(.system(size: 11))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(2)
                }

                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                (isHovered ? tokens.panelBackground : tokens.panelBackground.opacity(0.6)),
                in: RoundedRectangle(cornerRadius: 10, style: .continuous)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }

    private var color: Color {
        switch task.status {
        case .idle: tokens.tertiaryText
        case .waiting: tokens.warning
        case .running: tokens.accent
        case .failed: tokens.failure
        case .cancelled: tokens.warning
        case .done: tokens.success
        }
    }
}

private struct RuntimeSettingsPopover: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Binding var isPresented: Bool
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(systemName: "person.crop.circle.fill")
                    .font(.system(size: 22))
                    .foregroundStyle(tokens.secondaryText)
                    .frame(width: 42, height: 42)
                    .background(tokens.elevatedBackground, in: Circle())

                VStack(alignment: .leading, spacing: 2) {
                    Text(viewModel.selectedProfile)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    Text(viewModel.currentModelLabel)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                }

                Spacer()
            }
            .padding(14)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )

            VStack(spacing: 0) {
                Menu {
                    ForEach(viewModel.profileChoices, id: \.self) { profile in
                        Button(profile) {
                            Task { await appViewModel.selectProfile(profile) }
                        }
                    }
                } label: {
                    RuntimePopoverRow(
                        title: "Profile",
                        systemImage: "switch.2",
                        trailing: viewModel.selectedProfile
                    )
                }
                .buttonStyle(.plain)

                Divider()
                    .padding(.leading, 38)

                RuntimePopoverStaticRow(
                    title: "Runtime health",
                    systemImage: "heart.text.square",
                    trailing: viewModel.currentHealthLabel
                )

                Divider()
                    .padding(.leading, 38)

                RuntimePopoverStaticRow(
                    title: "Branch",
                    systemImage: "point.bottomleft.forward.to.point.topright.scurvepath",
                    trailing: viewModel.currentBranchLabel
                )

                Divider()
                    .padding(.leading, 38)

                Button {
                    isPresented = false
                    appViewModel.showSettings()
                } label: {
                    RuntimePopoverRow(
                        title: "Settings",
                        systemImage: "gearshape"
                    )
                }
                .buttonStyle(.plain)
            }
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
        }
        .padding(14)
        .frame(width: 288)
        .background(tokens.windowBackground)
    }
}

private struct RuntimePopoverRow: View {
    let title: String
    let systemImage: String
    var trailing: String? = nil
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 10) {
            Image(systemName: systemImage)
                .frame(width: 16)
            Text(title)
            Spacer()
            if let trailing {
                Text(trailing)
                    .foregroundStyle(tokens.secondaryText)
                    .lineLimit(1)
            }
            Image(systemName: "chevron.right")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(tokens.tertiaryText)
        }
        .font(.system(size: 13, weight: .medium))
        .foregroundStyle(tokens.primaryText)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background((isHovered ? tokens.elevatedBackground : Color.clear), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(isHovered ? tokens.border : .clear, lineWidth: 1)
        )
        .contentShape(Rectangle())
        .onHover { isHovered = $0 }
    }
}

private struct RuntimePopoverStaticRow: View {
    let title: String
    let systemImage: String
    let trailing: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 10) {
            Image(systemName: systemImage)
                .frame(width: 16)
            Text(title)
            Spacer()
            Text(trailing)
                .foregroundStyle(tokens.secondaryText)
                .lineLimit(1)
        }
        .font(.system(size: 13, weight: .medium))
        .foregroundStyle(tokens.primaryText)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }
}

private struct SidebarFooterSettingsButton: View {
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 10) {
            Image(systemName: "gearshape")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(tokens.secondaryText)
                .frame(width: 24, height: 24)
                .background(tokens.elevatedBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 8, style: .continuous))

            Text("Settings")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(tokens.primaryText)

            Spacer()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 9)
        .background((isHovered ? tokens.panelBackground.opacity(0.82) : Color.clear), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onHover { isHovered = $0 }
    }
}

private struct SidebarHeaderIconButton: View {
    let systemImage: String
    let tokens: ThemeTokens
    let action: () -> Void
    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(tokens.tertiaryText)
                .frame(width: 22, height: 22)
                .background((isHovered ? tokens.panelBackground : Color.clear), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct SidebarSearchField: View {
    @Binding var text: String
    let tokens: ThemeTokens

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(tokens.tertiaryText)

            TextField("Search threads", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(tokens.primaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(tokens.panelBackground.opacity(0.82), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}
