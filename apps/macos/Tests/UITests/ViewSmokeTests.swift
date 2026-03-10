import Domain
import Features
import Infrastructure
import SwiftUI
import UI
import XCTest

@MainActor
final class ViewSmokeTests: XCTestCase {
    func testWorkbenchViewInstantiatesInDarkAndLightModes() {
        let client = MockRuntimeClient()
        let appViewModel = AppViewModel(
            runtimeClient: client,
            workspaceStore: InMemoryWorkspaceStore(
                workspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
                selectedID: PreviewFixtures.workspace.id
            )
        )
        appViewModel.selectedWorkspace = PreviewFixtures.workspace
        appViewModel.selectedWorkspaceStatus = PreviewFixtures.statusSummary
        appViewModel.selectedModelsStatus = PreviewFixtures.modelsStatusSummary
        appViewModel.selectedWorkspaceHealth = PreviewFixtures.healthSummary
        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )
        viewModel.replaceStateForPreview(WorkbenchStateMapper.map(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            status: PreviewFixtures.statusSummary,
            health: PreviewFixtures.healthSummary,
            models: PreviewFixtures.modelsStatusSummary,
            sessions: PreviewFixtures.sessions,
            transcript: PreviewFixtures.transcript,
            composerText: "",
            isSending: false,
            inlineError: nil
        ))

        let light = NSHostingView(rootView: WorkbenchView(appViewModel: appViewModel, viewModel: viewModel).environment(\.colorScheme, .light))
        let dark = NSHostingView(rootView: WorkbenchView(appViewModel: appViewModel, viewModel: viewModel).environment(\.colorScheme, .dark))

        XCTAssertNotNil(light)
        XCTAssertNotNil(dark)
    }

    func testOnboardingAndWorkspacePickerInstantiate() {
        let client = MockRuntimeClient()
        let store = InMemoryWorkspaceStore(workspaces: [PreviewFixtures.workspace], selectedID: PreviewFixtures.workspace.id)
        let appViewModel = AppViewModel(runtimeClient: client, workspaceStore: store)
        appViewModel.screen = .setupHub
        appViewModel.selectedWorkspace = PreviewFixtures.workspace
        appViewModel.selectedWorkspaceStatus = PreviewFixtures.statusSummary
        appViewModel.selectedModelsStatus = PreviewFixtures.modelsStatusSummary
        appViewModel.selectedWorkspaceHealth = PreviewFixtures.healthSummary

        let setupHub = NSHostingView(rootView: SetupHubView(viewModel: appViewModel))

        XCTAssertNotNil(setupHub)
    }

    func testRootContentViewInstantiatesWithCommandPaletteVisible() {
        let client = MockRuntimeClient()
        let appViewModel = AppViewModel(
            runtimeClient: client,
            workspaceStore: InMemoryWorkspaceStore(
                workspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
                selectedID: PreviewFixtures.workspace.id
            )
        )
        appViewModel.selectedWorkspace = PreviewFixtures.workspace
        appViewModel.selectedWorkspaceStatus = PreviewFixtures.statusSummary
        appViewModel.selectedModelsStatus = PreviewFixtures.modelsStatusSummary
        appViewModel.selectedWorkspaceHealth = PreviewFixtures.healthSummary
        appViewModel.selectedConfigurationSummary = PreviewFixtures.configurationSummary
        appViewModel.screen = .workbench
        appViewModel.isCommandPalettePresented = true

        let workbench = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            runtimeClient: client,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )
        workbench.replaceStateForPreview(WorkbenchStateMapper.map(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            status: PreviewFixtures.statusSummary,
            health: PreviewFixtures.healthSummary,
            models: PreviewFixtures.modelsStatusSummary,
            sessions: PreviewFixtures.sessions,
            transcript: PreviewFixtures.transcript,
            composerText: "Summarize the command palette work.",
            isSending: false,
            inlineError: nil
        ))
        appViewModel.workbench = workbench

        let root = NSHostingView(rootView: RootContentView(viewModel: appViewModel).environment(\.colorScheme, .dark))

        XCTAssertNotNil(root)
    }
}
