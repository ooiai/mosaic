import AppKit
import Domain
import Features
import SwiftUI

struct SidebarView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

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
                VStack(alignment: .leading, spacing: 18) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("mosaic")
                            .font(.system(size: 11, weight: .bold, design: .monospaced))
                            .foregroundStyle(tokens.secondaryText)
                        Text(viewModel.project.name)
                            .font(.system(size: 18, weight: .semibold))
                            .foregroundStyle(tokens.primaryText)
                        Text(viewModel.project.workspacePath)
                            .font(.system(size: 12))
                            .foregroundStyle(tokens.tertiaryText)
                            .lineLimit(2)
                            .textSelection(.enabled)
                    }
                    .padding(.horizontal, 8)
                    .padding(.top, 4)

                    VStack(spacing: 4) {
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

                    sidebarSectionHeader("Projects", tokens: tokens) {
                        Button(action: chooseWorkspace) {
                            Image(systemName: "folder.badge.plus")
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(tokens.tertiaryText)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(appViewModel.recentProjects.prefix(8)) { project in
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
                            Button {
                                appViewModel.createNewThread()
                            } label: {
                                Image(systemName: "plus")
                            }
                            .buttonStyle(.plain)

                            Button {
                                appViewModel.presentCommandPalette()
                            } label: {
                                Image(systemName: "line.3.horizontal.decrease")
                            }
                            .buttonStyle(.plain)
                        }
                        .foregroundStyle(tokens.tertiaryText)
                    }

                    TextField("Search threads", text: $viewModel.sidebarQuery)
                        .textFieldStyle(.roundedBorder)

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
                .padding(12)
                .padding(.top, 6)
            }

            Divider()

            VStack(spacing: 8) {
                SidebarFooterCard(
                    title: viewModel.selectedProfile,
                    subtitle: viewModel.currentModelLabel,
                    symbolName: "person.crop.circle"
                )

                Button {
                    appViewModel.showSettings()
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: "gearshape")
                        Text("Settings")
                            .font(.system(size: 14, weight: .medium))
                        Spacer()
                    }
                    .foregroundStyle(tokens.primaryText)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 12)
                }
                .buttonStyle(.plain)
            }
            .padding(12)
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
                .font(.system(size: 13, weight: .medium))
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
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(tokens.secondaryText)
                Text(project.name)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(background, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
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

        Button(action: onSelect) {
            VStack(alignment: .leading, spacing: 7) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(session.title)
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                        .lineLimit(1)
                    if session.isPinned {
                        Image(systemName: "pin.fill")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(tokens.warning)
                    }
                    Spacer()
                    Text(relativeDate(session.updatedAt))
                        .font(.system(size: 11))
                        .foregroundStyle(tokens.tertiaryText)
                }

                HStack(spacing: 6) {
                    Circle()
                        .fill(accentColor)
                        .frame(width: 6, height: 6)
                    Text(session.summary.isEmpty ? "No summary yet" : session.summary)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(2)
                }

                HStack(spacing: 10) {
                    Text("\(session.messageCount) msg")
                    Text("\(session.taskCount) task")
                    if let latestTask {
                        let additions = latestTask.fileChanges.reduce(0) { $0 + $1.additions }
                        let deletions = latestTask.fileChanges.reduce(0) { $0 + $1.deletions }
                        if additions > 0 {
                            Text("+\(additions)")
                                .foregroundStyle(tokens.success)
                        }
                        if deletions > 0 {
                            Text("-\(deletions)")
                                .foregroundStyle(tokens.failure)
                        }
                    }
                }
                .font(.system(size: 10, weight: .medium, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 10)
            .background(background(tokens: tokens), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
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
            .padding(.vertical, 8)
            .background(
                (isHovered ? tokens.panelBackground : tokens.panelBackground.opacity(0.6)),
                in: RoundedRectangle(cornerRadius: 12, style: .continuous)
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

private struct SidebarFooterCard: View {
    let title: String
    let subtitle: String
    let symbolName: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 10) {
            Image(systemName: symbolName)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(tokens.secondaryText)
                .frame(width: 24, height: 24)
                .background(tokens.elevatedBackground, in: Circle())

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                    .lineLimit(1)
                Text(subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(tokens.secondaryText)
                    .lineLimit(1)
            }

            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}
