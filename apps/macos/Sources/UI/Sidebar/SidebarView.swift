import AppKit
import Features
import SwiftUI

struct SidebarView: View {
    @Bindable var appViewModel: AppViewModel
    @Bindable var viewModel: WorkbenchViewModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                PanelCard {
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("mosaic")
                                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                                    .foregroundStyle(tokens.accent)
                                Text(viewModel.project.name)
                                    .font(.system(size: 17, weight: .semibold))
                                    .foregroundStyle(tokens.primaryText)
                            }
                            Spacer()
                            Button {
                                let panel = NSOpenPanel()
                                panel.canChooseDirectories = true
                                panel.canChooseFiles = false
                                panel.allowsMultipleSelection = false
                                if panel.runModal() == .OK, let url = panel.url {
                                    Task { await appViewModel.registerWorkspace(url: url) }
                                }
                            } label: {
                                Image(systemName: "plus")
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(tokens.tertiaryText)
                        }

                        Text(viewModel.project.workspacePath)
                            .font(.system(size: 11))
                            .foregroundStyle(tokens.secondaryText)
                            .textSelection(.enabled)
                            .lineLimit(3)

                        HStack(spacing: 8) {
                            MetricChip(title: "Profile", value: viewModel.selectedProfile, accent: tokens.accent)
                            MetricChip(title: "Health", value: viewModel.currentHealthLabel, accent: tokens.success)
                        }
                    }
                }

                PanelCard {
                    VStack(alignment: .leading, spacing: 10) {
                        SectionHeader("Projects", trailing: "\(appViewModel.recentProjects.count)")
                        ForEach(appViewModel.recentProjects.prefix(8)) { project in
                            Button {
                                Task { await appViewModel.openProject(project.id) }
                            } label: {
                                HStack {
                                    VStack(alignment: .leading, spacing: 3) {
                                        Text(project.name)
                                            .font(.system(size: 12, weight: .semibold))
                                        Text(project.workspacePath)
                                            .font(.system(size: 10))
                                            .lineLimit(1)
                                    }
                                    Spacer()
                                }
                                .foregroundStyle(project.id == viewModel.project.id ? tokens.primaryText : tokens.secondaryText)
                                .padding(.vertical, 5)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }

                PanelCard {
                    VStack(alignment: .leading, spacing: 10) {
                        SectionHeader("Sessions", trailing: "\(viewModel.filteredSessions.count)")

                        TextField("Search sessions", text: $viewModel.sidebarQuery)
                            .textFieldStyle(.roundedBorder)

                        Button("New Thread") {
                            appViewModel.createNewThread()
                        }
                        .buttonStyle(.borderless)
                        .foregroundStyle(tokens.accent)

                        ForEach(viewModel.filteredSessions) { session in
                            Button {
                                viewModel.selectSession(session.id)
                            } label: {
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack {
                                        Text(session.title)
                                            .font(.system(size: 12, weight: .semibold))
                                            .lineLimit(1)
                                        Spacer()
                                        if session.isPinned {
                                            Image(systemName: "pin.fill")
                                                .font(.system(size: 10))
                                                .foregroundStyle(tokens.warning)
                                        }
                                    }
                                    Text(session.summary)
                                        .font(.system(size: 10))
                                        .lineLimit(2)
                                }
                                .foregroundStyle(session.id == viewModel.selectedSessionID ? tokens.primaryText : tokens.secondaryText)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(10)
                                .background(
                                    (session.id == viewModel.selectedSessionID ? tokens.selection : tokens.elevatedBackground.opacity(0.6)),
                                    in: RoundedRectangle(cornerRadius: 12, style: .continuous)
                                )
                            }
                            .buttonStyle(.plain)
                            .contextMenu {
                                Button(session.isPinned ? "Unpin" : "Pin") {
                                    Task { await viewModel.togglePinned(sessionID: session.id) }
                                }
                            }
                        }
                    }
                }

                if !viewModel.recentTasks.isEmpty {
                    PanelCard {
                        VStack(alignment: .leading, spacing: 10) {
                            SectionHeader("Recent Tasks", trailing: "\(min(viewModel.recentTasks.count, 6))")
                            ForEach(viewModel.recentTasks.prefix(6)) { task in
                                Button {
                                    viewModel.selectTask(task.id)
                                    viewModel.inspectorPanel = .overview
                                } label: {
                                    VStack(alignment: .leading, spacing: 4) {
                                        HStack {
                                            Text(task.title)
                                                .font(.system(size: 12, weight: .semibold))
                                                .lineLimit(1)
                                            Spacer()
                                            StatusChip(title: task.status.rawValue.uppercased(), state: task.status)
                                        }
                                        Text(task.summary.isEmpty ? task.prompt : task.summary)
                                            .font(.system(size: 10))
                                            .lineLimit(2)
                                            .foregroundStyle(tokens.secondaryText)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }
            }
            .padding(14)
        }
    }
}
