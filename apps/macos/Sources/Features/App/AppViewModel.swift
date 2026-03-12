import AppKit
import Domain
import Foundation
import Infrastructure
import Observation

@MainActor
@Observable
public final class AppViewModel {
    public var screen: AppScreen = .loading
    public private(set) var workbench: WorkbenchViewModel?
    public private(set) var projects: [Project] = []
    public var settings: AppSettings = .init()
    public var destination: WorkbenchDestination = .thread
    public var settingsSection: SettingsSection = .general
    public var isConsoleDrawerVisible = false
    public var isCommandPalettePresented = false
    public var globalError: String?

    private var archive = DesktopArchive()
    private let runtime: AgentWorkbenchRuntime
    private let persistenceStore: DesktopPersistenceStoring
    private let workspaceStore: WorkspaceStoring
    private let pinnedSessionsStore: PinnedSessionsStoring

    public init(
        runtimeClient: AgentWorkbenchRuntime,
        persistenceStore: DesktopPersistenceStoring = DesktopArchiveStore(),
        workspaceStore: WorkspaceStoring = WorkspaceStore(),
        pinnedSessionsStore: PinnedSessionsStoring = PinnedSessionStore()
    ) {
        self.runtime = runtimeClient
        self.persistenceStore = persistenceStore
        self.workspaceStore = workspaceStore
        self.pinnedSessionsStore = pinnedSessionsStore
    }

    public var selectedProject: Project? {
        guard let selectedProjectID = archive.selectedProjectID else { return nil }
        return projects.first(where: { $0.id == selectedProjectID })
    }

    public var recentProjects: [Project] {
        projects.sorted { $0.lastOpenedAt > $1.lastOpenedAt }
    }

    public var canSendPrompt: Bool {
        workbench?.canSend == true
    }

    public func bootstrap() async {
        screen = .loading
        archive = await persistenceStore.loadArchive()
        archive = await importLegacyWorkspacesIfNeeded(into: archive)
        settings = archive.settings
        projects = archive.projects.map(\.project).sorted { $0.lastOpenedAt > $1.lastOpenedAt }

        guard let selectedProject = resolveSelectedProject() else {
            workbench = nil
            screen = .setupHub
            return
        }

        await openProject(selectedProject.id)
    }

    public func openProject(_ projectID: UUID) async {
        guard let projectArchive = archive.projects.first(where: { $0.project.id == projectID }) else { return }
        archive.selectedProjectID = projectID
        await persistArchive()

        let workbench = WorkbenchViewModel(
            project: projectArchive.project,
            archive: projectArchive,
            runtime: runtime,
            pinnedSessionsStore: pinnedSessionsStore
        ) { [weak self] projectArchive in
            await self?.persist(projectArchive: projectArchive)
        }
        self.workbench = workbench
        destination = .thread
        screen = .workbench
        await workbench.bootstrap()
    }

    public func registerWorkspace(url: URL) async {
        let workspace = WorkspaceReference(
            name: FileManager.default.displayName(atPath: url.path),
            path: url.path
        )
        await workspaceStore.save(workspace: workspace)

        let project = Project(
            workspace: workspace,
            preferredProfile: settings.defaultProfile
        )
        archive.projects.removeAll { $0.project.id == project.id || $0.project.workspacePath == project.workspacePath }
        archive.projects.insert(ProjectArchive(project: project), at: 0)
        projects = archive.projects.map(\.project).sorted { $0.lastOpenedAt > $1.lastOpenedAt }
        archive.selectedProjectID = project.id
        settings.defaultWorkspacePath = project.workspacePath
        await persistArchive()
        await openProject(project.id)
    }

    public func showSetupHub() {
        screen = .setupHub
    }

    public func navigate(to destination: WorkbenchDestination) {
        self.destination = destination
    }

    public func showSettings(section: SettingsSection = .general) {
        settingsSection = section
        destination = .settings
    }

    public func selectSettingsSection(_ section: SettingsSection) {
        settingsSection = section
        destination = .settings
    }

    public func toggleConsoleDrawer() {
        isConsoleDrawerVisible.toggle()
    }

    public func presentCommandPalette() {
        isCommandPalettePresented = true
    }

    public func dismissCommandPalette() {
        isCommandPalettePresented = false
    }

    public func refreshActiveProject() async {
        await workbench?.refresh()
    }

    public func createNewThread() {
        destination = .thread
        workbench?.newThread()
    }

    public func openSession(_ sessionID: String) {
        destination = .thread
        workbench?.selectSession(sessionID)
    }

    public func seedComposer(with prompt: String, startNewThread: Bool = false) {
        destination = .thread
        if startNewThread {
            workbench?.newThread()
        }
        workbench?.composerText = prompt
    }

    public func sendCurrentPrompt() async {
        await workbench?.sendCurrentPrompt(settings: settings)
    }

    public func cancelActiveTask() async {
        await workbench?.cancelActiveTask()
    }

    public func retrySelectedTask() async {
        await workbench?.retrySelectedTask(settings: settings)
    }

    public func toggleInspector() {
        workbench?.toggleInspector()
    }

    public func revealSelectedWorkspaceInFinder() {
        guard let selectedProject else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: selectedProject.workspacePath)])
    }

    public func persistSettings() async {
        archive.settings = settings
        if let selectedProjectID = archive.selectedProjectID,
           let index = archive.projects.firstIndex(where: { $0.project.id == selectedProjectID }) {
            archive.projects[index].project.preferredProfile = settings.defaultProfile
        }
        await persistArchive()
    }

    public func selectProfile(_ profile: String) async {
        workbench?.selectedProfile = profile
        settings.defaultProfile = profile
        await persistSettings()
    }

    private func persist(projectArchive: ProjectArchive) async {
        archive.projects.removeAll { $0.project.id == projectArchive.project.id }
        archive.projects.insert(projectArchive, at: 0)
        projects = archive.projects.map(\.project).sorted { $0.lastOpenedAt > $1.lastOpenedAt }
        if archive.selectedProjectID == nil {
            archive.selectedProjectID = projectArchive.project.id
        }
        await persistArchive()
    }

    private func persistArchive() async {
        do {
            try await persistenceStore.saveArchive(archive)
        } catch {
            globalError = error.localizedDescription
        }
    }

    private func resolveSelectedProject() -> Project? {
        if let selectedProjectID = archive.selectedProjectID,
           let project = archive.projects.first(where: { $0.project.id == selectedProjectID })?.project {
            return project
        }
        return archive.projects.first?.project
    }

    private func importLegacyWorkspacesIfNeeded(into archive: DesktopArchive) async -> DesktopArchive {
        guard archive.projects.isEmpty else { return archive }
        let workspaces = await workspaceStore.recentWorkspaces()
        guard !workspaces.isEmpty else { return archive }

        var updated = archive
        updated.projects = workspaces.map {
            ProjectArchive(
                project: Project(workspace: $0, preferredProfile: archive.settings.defaultProfile)
            )
        }
        updated.selectedProjectID = updated.projects.first?.project.id
        return updated
    }
}
