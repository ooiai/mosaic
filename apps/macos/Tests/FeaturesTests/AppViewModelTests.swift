import Domain
import Features
import Infrastructure
import XCTest

@MainActor
final class AppViewModelTests: XCTestCase {
    func testBootstrapShowsWorkspacePickerWhenNoWorkspaceSaved() async {
        let client = MockRuntimeClient()
        let store = InMemoryWorkspaceStore()
        let viewModel = AppViewModel(runtimeClient: client, workspaceStore: store)

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.screen, .workspacePicker)
    }

    func testBootstrapRoutesConfiguredWorkspaceToWorkbench() async {
        let client = MockRuntimeClient()
        let store = InMemoryWorkspaceStore(
            workspaces: [PreviewFixtures.workspace],
            selectedID: PreviewFixtures.workspace.id
        )
        let viewModel = AppViewModel(runtimeClient: client, workspaceStore: store)

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.screen, .workbench)
        XCTAssertNotNil(viewModel.workbench)
    }

    func testBootstrapRoutesUnconfiguredWorkspaceToOnboarding() async {
        let client = MockRuntimeClient()
        client.statusHandler = { _ in
            RuntimeStatusSummary(
                configured: false,
                configPath: "/tmp/.mosaic/config.toml",
                profile: nil,
                latestSession: nil,
                defaultAgentID: nil,
                agentsCount: 0,
                stateMode: "project",
                provider: nil
            )
        }
        let store = InMemoryWorkspaceStore(
            workspaces: [PreviewFixtures.workspace],
            selectedID: PreviewFixtures.workspace.id
        )
        let viewModel = AppViewModel(runtimeClient: client, workspaceStore: store)

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.screen, .onboarding(PreviewFixtures.workspace))
    }
}
