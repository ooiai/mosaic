import Domain
import Features
import Infrastructure
import XCTest

@MainActor
final class AppViewModelTests: XCTestCase {
    func testBootstrapImportsLegacyWorkspaceWhenArchiveIsEmpty() async {
        let runtime = MockWorkbenchRuntime()
        let persistence = InMemoryDesktopArchiveStore()
        let workspaceStore = InMemoryWorkspaceStore(
            workspaces: [PreviewFixtures.workspace],
            selectedID: PreviewFixtures.workspace.id
        )
        let viewModel = AppViewModel(
            runtimeClient: runtime,
            persistenceStore: persistence,
            workspaceStore: workspaceStore,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.screen, .workbench)
        XCTAssertEqual(viewModel.selectedProject?.workspacePath, PreviewFixtures.workspace.path)
        XCTAssertNotNil(viewModel.workbench)
    }

    func testRegisterWorkspaceCreatesProjectAndWorkbench() async {
        let runtime = MockWorkbenchRuntime()
        let persistence = InMemoryDesktopArchiveStore()
        let workspaceStore = InMemoryWorkspaceStore()
        let viewModel = AppViewModel(
            runtimeClient: runtime,
            persistenceStore: persistence,
            workspaceStore: workspaceStore,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )

        let url = URL(fileURLWithPath: "/tmp/demo-workspace", isDirectory: true)
        await viewModel.registerWorkspace(url: url)

        XCTAssertEqual(viewModel.selectedProject?.workspacePath, url.path)
        XCTAssertEqual(viewModel.screen, .workbench)
        XCTAssertNotNil(viewModel.workbench)
    }

    func testPersistSettingsWritesArchive() async {
        let runtime = MockWorkbenchRuntime()
        let persistence = InMemoryDesktopArchiveStore(
            archive: DesktopArchive(
                projects: [PreviewFixtures.projectArchive],
                selectedProjectID: PreviewFixtures.project.id,
                settings: .init()
            )
        )
        let viewModel = AppViewModel(
            runtimeClient: runtime,
            persistenceStore: persistence,
            workspaceStore: InMemoryWorkspaceStore(
                workspaces: [PreviewFixtures.workspace],
                selectedID: PreviewFixtures.workspace.id
            ),
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )

        await viewModel.bootstrap()
        viewModel.settings.defaultProfile = "review"
        viewModel.settings.themeMode = .dark
        await viewModel.persistSettings()

        let archive = await persistence.loadArchive()
        XCTAssertEqual(archive.settings.defaultProfile, "review")
        XCTAssertEqual(archive.settings.themeMode, .dark)
    }
}
