import Domain
import Foundation
import Infrastructure
import Observation

@MainActor
@Observable
public final class AppViewModel {
    public var screen: AppScreen = .loading
    public var workbench: WorkbenchViewModel?
    public var recentWorkspaces: [WorkspaceReference] = []
    public var selectedWorkspace: WorkspaceReference?
    public var onboardingBaseURL = "https://api.openai.com"
    public var onboardingModel = "gpt-4o-mini"
    public var onboardingAPIKeyEnv = "OPENAI_API_KEY"
    public var globalError: String?

    private let runtimeClient: MosaicRuntimeClient
    private let workspaceStore: WorkspaceStoring

    public init(
        runtimeClient: MosaicRuntimeClient,
        workspaceStore: WorkspaceStoring
    ) {
        self.runtimeClient = runtimeClient
        self.workspaceStore = workspaceStore
    }

    public func bootstrap() async {
        screen = .loading
        recentWorkspaces = await workspaceStore.recentWorkspaces()

        guard let workspace = await workspaceStore.selectedWorkspace() ?? recentWorkspaces.first else {
            screen = .workspacePicker
            return
        }

        await loadWorkspace(workspace)
    }

    public func selectWorkspace(_ workspace: WorkspaceReference) async {
        await workspaceStore.select(workspaceID: workspace.id)
        await loadWorkspace(workspace)
    }

    public func registerWorkspace(url: URL) async {
        let workspace = WorkspaceReference(
            name: FileManager.default.displayName(atPath: url.path),
            path: url.path
        )
        await workspaceStore.save(workspace: workspace)
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        await loadWorkspace(workspace)
    }

    public func completeOnboarding() async {
        guard let workspace = selectedWorkspace else { return }
        do {
            _ = try await runtimeClient.setup(
                workspace: workspace,
                baseURL: onboardingBaseURL,
                model: onboardingModel,
                apiKeyEnv: onboardingAPIKeyEnv
            )
            await loadWorkspace(workspace)
        } catch {
            globalError = error.localizedDescription
        }
    }

    public func dismissError() {
        globalError = nil
    }

    private func loadWorkspace(_ workspace: WorkspaceReference) async {
        selectedWorkspace = workspace
        do {
            let status = try await runtimeClient.status(workspace: workspace)
            if !status.configured {
                screen = .onboarding(workspace)
                return
            }

            let viewModel = WorkbenchViewModel(
                workspace: workspace,
                recentWorkspaces: await workspaceStore.recentWorkspaces(),
                runtimeClient: runtimeClient
            )
            workbench = viewModel
            screen = .workbench
            await viewModel.refresh()
        } catch {
            screen = .error(error.localizedDescription)
        }
    }
}
