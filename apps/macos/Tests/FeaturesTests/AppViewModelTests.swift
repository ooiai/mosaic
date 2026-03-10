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

        XCTAssertEqual(viewModel.screen, .setupHub)
    }

    func testDefaultOnboardingUsesAzureAndRequiresConcreteResourceHost() {
        let viewModel = AppViewModel(
            runtimeClient: MockRuntimeClient(),
            workspaceStore: InMemoryWorkspaceStore()
        )
        viewModel.selectedWorkspace = PreviewFixtures.workspace

        XCTAssertEqual(viewModel.onboardingBaseURL, "https://YOUR_RESOURCE_NAME.openai.azure.com/openai/v1")
        XCTAssertEqual(viewModel.onboardingModel, "gpt-5.2")
        XCTAssertEqual(viewModel.onboardingAPIKeyEnv, "AZURE_OPENAI_API_KEY")
        XCTAssertTrue(viewModel.onboardingRequiresAzureResourceHost)
        XCTAssertFalse(viewModel.canInitializeWorkspace)

        viewModel.onboardingBaseURL = "https://demo-resource.openai.azure.com/openai/v1"

        XCTAssertFalse(viewModel.onboardingRequiresAzureResourceHost)
        XCTAssertTrue(viewModel.canInitializeWorkspace)
    }

    func testAzureResourceFieldBuildsCanonicalBaseURL() {
        let viewModel = AppViewModel(
            runtimeClient: MockRuntimeClient(),
            workspaceStore: InMemoryWorkspaceStore()
        )

        viewModel.onboardingAzureResourceName = "demo-resource"
        XCTAssertEqual(viewModel.onboardingBaseURL, "https://demo-resource.openai.azure.com/openai/v1")
        XCTAssertEqual(viewModel.onboardingAzureResourceName, "demo-resource")

        viewModel.runtimeDraftAzureResourceName = "https://team-east.openai.azure.com/openai/v1"
        XCTAssertEqual(viewModel.runtimeDraftBaseURL, "https://team-east.openai.azure.com/openai/v1")
        XCTAssertEqual(viewModel.runtimeDraftAzureResourceName, "team-east")
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

        XCTAssertEqual(viewModel.screen, .setupHub)
        XCTAssertEqual(viewModel.selectedWorkspace, PreviewFixtures.workspace)
        XCTAssertFalse(viewModel.selectedWorkspaceConfigured)
    }

    func testSaveRuntimeSettingsPersistsDraftAndKeepsWorkbenchInstance() async {
        actor RuntimeStore {
            var baseURL = "https://demo-resource.openai.azure.com/openai/v1"
            var model = "gpt-5.2"
            var apiKeyEnv = "AZURE_OPENAI_API_KEY"

            func apply(key: RuntimeConfigKey, value: String) {
                switch key {
                case .providerBaseURL:
                    baseURL = value
                case .providerAPIKeyEnv:
                    apiKeyEnv = value
                }
            }

            func applyModel(_ value: String) {
                model = value
            }

            func configurationSummary() -> ConfigurationSummary {
                ConfigurationSummary(
                    profileName: "default",
                    agent: .init(maxTurns: 8, temperature: 0.2),
                    provider: .init(
                        apiKeyEnv: apiKeyEnv,
                        baseURL: baseURL,
                        kind: "openai_compatible",
                        model: model
                    ),
                    stateMode: "project",
                    projectDir: ".mosaic"
                )
            }

            func modelsStatus() -> ModelsStatusSummary {
                ModelsStatusSummary(
                    profile: "default",
                    currentModel: model,
                    effectiveModel: model,
                    baseURL: baseURL,
                    apiKeyEnv: apiKeyEnv,
                    aliases: [:],
                    fallbacks: []
                )
            }
        }

        let store = RuntimeStore()
        let client = MockRuntimeClient()
        client.configureShowHandler = { _ in
            await store.configurationSummary()
        }
        client.modelsStatusHandler = { _ in
            await store.modelsStatus()
        }
        client.configureSetHandler = { _, key, value in
            await store.apply(key: key, value: value)
        }
        client.setModelHandler = { _, model in
            let previousStatus = await store.modelsStatus()
            await store.applyModel(model)
            return ModelSelectionSummary(
                requestedModel: model,
                effectiveModel: model,
                previousModel: previousStatus.effectiveModel
            )
        }

        let workspaceStore = InMemoryWorkspaceStore(
            workspaces: [PreviewFixtures.workspace],
            selectedID: PreviewFixtures.workspace.id
        )
        let viewModel = AppViewModel(runtimeClient: client, workspaceStore: workspaceStore)

        await viewModel.bootstrap()

        let originalWorkbench = viewModel.workbench
        XCTAssertEqual(viewModel.screen, .workbench)

        viewModel.runtimeDraftBaseURL = "http://localhost:1234/v1"
        viewModel.runtimeDraftModel = "gpt-4.1-mini"
        viewModel.runtimeDraftAPIKeyEnv = "LOCAL_OPENAI_API_KEY"

        await viewModel.saveRuntimeSettings()

        XCTAssertEqual(viewModel.currentBaseURLLabel, "http://localhost:1234/v1")
        XCTAssertEqual(viewModel.currentModelLabel, "gpt-4.1-mini")
        XCTAssertEqual(viewModel.currentAPIKeyEnvLabel, "LOCAL_OPENAI_API_KEY")
        XCTAssertNil(viewModel.globalError)
        XCTAssertFalse(viewModel.runtimeDraftHasChanges)
        XCTAssertTrue(originalWorkbench === viewModel.workbench)
    }

    func testQuickSwitchModelPersistsAndKeepsWorkbenchInstance() async {
        actor RuntimeStore {
            var baseURL = "https://demo-resource.openai.azure.com/openai/v1"
            var model = "gpt-5.2"
            var apiKeyEnv = "AZURE_OPENAI_API_KEY"

            func applyModel(_ value: String) {
                model = value
            }

            func configurationSummary() -> ConfigurationSummary {
                ConfigurationSummary(
                    profileName: "default",
                    agent: .init(maxTurns: 8, temperature: 0.2),
                    provider: .init(
                        apiKeyEnv: apiKeyEnv,
                        baseURL: baseURL,
                        kind: "openai_compatible",
                        model: model
                    ),
                    stateMode: "project",
                    projectDir: ".mosaic"
                )
            }

            func modelsStatus() -> ModelsStatusSummary {
                ModelsStatusSummary(
                    profile: "default",
                    currentModel: model,
                    effectiveModel: model,
                    baseURL: baseURL,
                    apiKeyEnv: apiKeyEnv,
                    aliases: [:],
                    fallbacks: []
                )
            }

            func listModels() -> [ModelSummary] {
                [
                    ModelSummary(id: "gpt-5.2", ownedBy: "azure"),
                    ModelSummary(id: "gpt-5-mini", ownedBy: "azure"),
                ]
            }
        }

        let store = RuntimeStore()
        let client = MockRuntimeClient()
        client.configureShowHandler = { _ in
            await store.configurationSummary()
        }
        client.modelsStatusHandler = { _ in
            await store.modelsStatus()
        }
        client.modelsListHandler = { _ in
            await store.listModels()
        }
        client.setModelHandler = { _, model in
            let previousStatus = await store.modelsStatus()
            await store.applyModel(model)
            return ModelSelectionSummary(
                requestedModel: model,
                effectiveModel: model,
                previousModel: previousStatus.effectiveModel
            )
        }

        let workspaceStore = InMemoryWorkspaceStore(
            workspaces: [PreviewFixtures.workspace],
            selectedID: PreviewFixtures.workspace.id
        )
        let viewModel = AppViewModel(runtimeClient: client, workspaceStore: workspaceStore)

        await viewModel.bootstrap()

        let originalWorkbench = viewModel.workbench
        XCTAssertEqual(viewModel.currentModelLabel, "gpt-5.2")

        await viewModel.quickSwitchModel("gpt-5-mini")

        XCTAssertEqual(viewModel.currentModelLabel, "gpt-5-mini")
        XCTAssertEqual(viewModel.runtimeDraftModel, "gpt-5-mini")
        XCTAssertNil(viewModel.globalError)
        XCTAssertTrue(originalWorkbench === viewModel.workbench)
    }

    func testBootstrapLoadsRecentCommandHistory() async {
        let client = MockRuntimeClient()
        let workspaceStore = InMemoryWorkspaceStore()
        let historyStore = InMemoryCommandHistoryStore(entries: ["new-thread", "refresh-workspace"])
        let viewModel = AppViewModel(
            runtimeClient: client,
            workspaceStore: workspaceStore,
            commandHistoryStore: historyStore
        )

        await viewModel.bootstrap()

        XCTAssertEqual(viewModel.recentCommandActionIDs, ["new-thread", "refresh-workspace"])
    }

    func testSessionAndWorkspaceActionsRecordDynamicHistory() async {
        let client = MockRuntimeClient()
        let workspaceStore = InMemoryWorkspaceStore(
            workspaces: [PreviewFixtures.workspace, PreviewFixtures.secondaryWorkspace],
            selectedID: PreviewFixtures.workspace.id
        )
        let historyStore = InMemoryCommandHistoryStore()
        let viewModel = AppViewModel(
            runtimeClient: client,
            workspaceStore: workspaceStore,
            commandHistoryStore: historyStore,
            pinnedSessionsStore: InMemoryPinnedSessionStore()
        )

        await viewModel.bootstrap()
        await viewModel.openSession("thread-1", recordHistory: true)
        await viewModel.togglePinnedSession("thread-1", recordHistory: true)
        await viewModel.activateWorkspace(PreviewFixtures.secondaryWorkspace, recordHistory: true)

        XCTAssertEqual(
            viewModel.recentCommandActionIDs,
            [
                "workspace-\(PreviewFixtures.secondaryWorkspace.id.uuidString)",
                "session-pin-thread-1",
                "session-open-thread-1",
            ]
        )
    }
}
