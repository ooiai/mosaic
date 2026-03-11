import Domain
import Features
import Infrastructure
import SwiftUI
import UI
import XCTest

@MainActor
final class ViewSmokeTests: XCTestCase {
    func testWorkbenchViewInstantiatesInLightAndDarkModes() async {
        let runtime = MockWorkbenchRuntime()
        let appViewModel = AppViewModel(
            runtimeClient: runtime,
            persistenceStore: InMemoryDesktopArchiveStore(
                archive: DesktopArchive(
                    projects: [PreviewFixtures.projectArchive],
                    selectedProjectID: PreviewFixtures.project.id,
                    settings: .init()
                )
            ),
            workspaceStore: InMemoryWorkspaceStore(
                workspaces: [PreviewFixtures.workspace],
                selectedID: PreviewFixtures.workspace.id
            ),
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )
        await appViewModel.bootstrap()
        guard let workbench = appViewModel.workbench else {
            XCTFail("Expected workbench")
            return
        }

        let light = NSHostingView(rootView: WorkbenchView(appViewModel: appViewModel, viewModel: workbench).environment(\.colorScheme, .light))
        let dark = NSHostingView(rootView: WorkbenchView(appViewModel: appViewModel, viewModel: workbench).environment(\.colorScheme, .dark))

        XCTAssertNotNil(light)
        XCTAssertNotNil(dark)
    }

    func testSetupHubInstantiates() async {
        let appViewModel = AppViewModel(
            runtimeClient: MockWorkbenchRuntime(),
            persistenceStore: InMemoryDesktopArchiveStore(),
            workspaceStore: InMemoryWorkspaceStore(),
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )
        await appViewModel.bootstrap()

        let setupHub = NSHostingView(rootView: SetupHubView(viewModel: appViewModel))
        XCTAssertNotNil(setupHub)
    }

    func testRootContentViewInstantiatesWithCommandPaletteVisible() async {
        let runtime = MockWorkbenchRuntime()
        let appViewModel = AppViewModel(
            runtimeClient: runtime,
            persistenceStore: InMemoryDesktopArchiveStore(
                archive: DesktopArchive(
                    projects: [PreviewFixtures.projectArchive],
                    selectedProjectID: PreviewFixtures.project.id,
                    settings: .init()
                )
            ),
            workspaceStore: InMemoryWorkspaceStore(
                workspaces: [PreviewFixtures.workspace],
                selectedID: PreviewFixtures.workspace.id
            ),
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )
        await appViewModel.bootstrap()
        appViewModel.isCommandPalettePresented = true

        let root = NSHostingView(rootView: RootContentView(viewModel: appViewModel).environment(\.colorScheme, .dark))
        XCTAssertNotNil(root)
    }
}
