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
        let viewModel = WorkbenchViewModel(
            workspace: PreviewFixtures.workspace,
            recentWorkspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            runtimeClient: client
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

        let light = NSHostingView(rootView: WorkbenchView(viewModel: viewModel).environment(\.colorScheme, .light))
        let dark = NSHostingView(rootView: WorkbenchView(viewModel: viewModel).environment(\.colorScheme, .dark))

        XCTAssertNotNil(light)
        XCTAssertNotNil(dark)
    }

    func testOnboardingAndWorkspacePickerInstantiate() {
        let client = MockRuntimeClient()
        let store = InMemoryWorkspaceStore(workspaces: [PreviewFixtures.workspace], selectedID: PreviewFixtures.workspace.id)
        let appViewModel = AppViewModel(runtimeClient: client, workspaceStore: store)

        let picker = NSHostingView(rootView: WorkspacePickerView(viewModel: appViewModel))
        let onboarding = NSHostingView(rootView: OnboardingView(viewModel: appViewModel, workspace: PreviewFixtures.workspace))

        XCTAssertNotNil(picker)
        XCTAssertNotNil(onboarding)
    }
}
