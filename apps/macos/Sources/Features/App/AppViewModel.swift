import Domain
import AppKit
import Foundation
import Infrastructure
import Observation

private enum OnboardingDefaults {
    static let azureBaseURL = "https://YOUR_RESOURCE_NAME.openai.azure.com/openai/v1"
    static let azureModel = "gpt-5.2"
    static let azureAPIKeyEnv = "AZURE_OPENAI_API_KEY"
    static let localBaseURL = "http://localhost:1234/v1"
    static let localAPIKeyEnv = "LOCAL_OPENAI_API_KEY"
}

@MainActor
@Observable
public final class AppViewModel {
    public var screen: AppScreen = .loading
    public var workbench: WorkbenchViewModel?
    public var recentWorkspaces: [WorkspaceReference] = []
    public var selectedWorkspace: WorkspaceReference?
    public var selectedWorkspaceStatus: RuntimeStatusSummary?
    public var selectedWorkspaceHealth: HealthSummary?
    public var selectedModelsStatus: ModelsStatusSummary?
    public var selectedConfigurationSummary: ConfigurationSummary?
    public var availableModels: [ModelSummary] = []
    public var onboardingBaseURL = OnboardingDefaults.azureBaseURL
    public var onboardingModel = OnboardingDefaults.azureModel
    public var onboardingAPIKeyEnv = OnboardingDefaults.azureAPIKeyEnv
    public var runtimeDraftBaseURL = OnboardingDefaults.azureBaseURL
    public var runtimeDraftModel = OnboardingDefaults.azureModel
    public var runtimeDraftAPIKeyEnv = OnboardingDefaults.azureAPIKeyEnv
    public var isCommandPalettePresented = false
    public var showAdvancedSetup = false
    public var isPreparingWorkspace = false
    public var isInitializingWorkspace = false
    public var isSavingRuntimeSettings = false
    public var isApplyingQuickModel = false
    public private(set) var recentCommandActionIDs: [String] = []
    public var globalError: String?

    private let runtimeClient: MosaicRuntimeClient
    private let workspaceStore: WorkspaceStoring
    private let commandHistoryStore: CommandHistoryStoring

    public init(
        runtimeClient: MosaicRuntimeClient,
        workspaceStore: WorkspaceStoring,
        commandHistoryStore: CommandHistoryStoring = CommandHistoryStore()
    ) {
        self.runtimeClient = runtimeClient
        self.workspaceStore = workspaceStore
        self.commandHistoryStore = commandHistoryStore
    }

    public func bootstrap() async {
        screen = .loading
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        recentCommandActionIDs = await commandHistoryStore.recentCommandActionIDs()

        guard let workspace = await workspaceStore.selectedWorkspace() ?? recentWorkspaces.first else {
            screen = .setupHub
            return
        }

        await prepareWorkspace(workspace, openWorkbenchIfConfigured: true)
    }

    public func selectWorkspace(_ workspace: WorkspaceReference) async {
        await workspaceStore.select(workspaceID: workspace.id)
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        await prepareWorkspace(workspace, openWorkbenchIfConfigured: true)
    }

    public func previewWorkspace(_ workspace: WorkspaceReference) async {
        await workspaceStore.select(workspaceID: workspace.id)
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        await prepareWorkspace(workspace, openWorkbenchIfConfigured: false)
    }

    public func registerWorkspace(url: URL) async {
        let workspace = WorkspaceReference(
            name: FileManager.default.displayName(atPath: url.path),
            path: url.path
        )
        await workspaceStore.save(workspace: workspace)
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        await prepareWorkspace(workspace, openWorkbenchIfConfigured: false)
    }

    public func completeOnboarding() async {
        guard let workspace = selectedWorkspace else { return }
        isInitializingWorkspace = true
        defer { isInitializingWorkspace = false }
        do {
            _ = try await runtimeClient.setup(
                workspace: workspace,
                baseURL: onboardingBaseURL,
                model: onboardingModel,
                apiKeyEnv: onboardingAPIKeyEnv
            )
            showAdvancedSetup = false
            await prepareWorkspace(workspace, openWorkbenchIfConfigured: true)
        } catch {
            globalError = error.localizedDescription
        }
    }

    public func openSelectedWorkspace() async {
        guard let workspace = selectedWorkspace else { return }
        await prepareWorkspace(workspace, openWorkbenchIfConfigured: true)
    }

    public func showSetupHub() {
        screen = .setupHub
    }

    public func presentCommandPalette() {
        isCommandPalettePresented = true
    }

    public func dismissCommandPalette() {
        isCommandPalettePresented = false
    }

    public func revealSelectedWorkspaceInFinder() {
        guard let selectedWorkspace else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: selectedWorkspace.path)])
    }

    public func refreshSelectedWorkspace() async {
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        guard let workspace = selectedWorkspace else {
            screen = .setupHub
            return
        }
        await prepareWorkspace(workspace, openWorkbenchIfConfigured: screen == .workbench)
    }

    public func dismissError() {
        globalError = nil
    }

    public func recordCommandAction(_ actionID: String) async {
        await commandHistoryStore.recordCommandActionID(actionID)
        recentCommandActionIDs = await commandHistoryStore.recentCommandActionIDs()
    }

    public func clearRecentCommandActions() async {
        await commandHistoryStore.clearRecentCommandActionIDs()
        recentCommandActionIDs = []
    }

    public func refreshActiveWorkspace() async {
        await refreshSelectedWorkspace()
    }

    public func createNewThread() {
        workbench?.newThread()
    }

    public func sendCurrentPrompt() async {
        await workbench?.sendCurrentPrompt()
    }

    public func clearCurrentThread() async {
        await workbench?.clearSelectedThread()
    }

    public func toggleInspector() {
        workbench?.toggleInspector()
    }

    public var selectedWorkspaceConfigured: Bool {
        selectedWorkspaceStatus?.configured == true
    }

    public var setupStatusTitle: String {
        if isInitializingWorkspace {
            return "Initializing workspace"
        }
        if isPreparingWorkspace {
            return "Checking workspace"
        }
        if selectedWorkspaceConfigured {
            return "Ready to open"
        }
        if selectedWorkspace != nil {
            return "Needs configuration"
        }
        return "Choose a workspace"
    }

    public var setupStatusDetail: String {
        if let workspace = selectedWorkspace, selectedWorkspaceConfigured {
            let profile = selectedWorkspaceStatus?.profile ?? "default"
            let model = selectedModelsStatus?.effectiveModel ?? selectedWorkspaceStatus?.provider?.model ?? onboardingModel
            return "\(workspace.name) is configured with profile \(profile) and model \(model)."
        }
        if let workspace = selectedWorkspace {
            if onboardingRequiresAzureResourceHost {
                return "\(workspace.name) still needs your Azure OpenAI resource host before Mosaic can initialize the workspace."
            }
            return "\(workspace.name) is ready for an Azure OpenAI-backed setup."
        }
        return "Pick a local project folder first, then point Mosaic at your Azure OpenAI resource."
    }

    public var setupStatusTone: RuntimeStripState.Tone {
        if globalError != nil { return .failure }
        if selectedWorkspaceConfigured { return .success }
        if selectedWorkspace != nil { return .warning }
        return .quiet
    }

    public var currentModelLabel: String {
        selectedModelsStatus?.effectiveModel ?? selectedWorkspaceStatus?.provider?.model ?? onboardingModel
    }

    public var setupModelChoices: [String] {
        var seen = Set<String>()
        let candidates = availableModels.map(\.id)
            + [onboardingModel, currentModelLabel, "gpt-5.2", "gpt-5-mini", "gpt-4.1", "gpt-4.1-mini"]
        return candidates.filter {
            let value = $0.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !value.isEmpty, !seen.contains(value) else { return false }
            seen.insert(value)
            return true
        }
    }

    public var currentProfileLabel: String {
        selectedWorkspaceStatus?.profile ?? "default"
    }

    public var currentHealthLabel: String {
        selectedWorkspaceHealth?.overallStatus ?? "Pending"
    }

    public var currentBaseURLLabel: String {
        selectedConfigurationSummary?.provider.baseURL
            ?? selectedModelsStatus?.baseURL
            ?? selectedWorkspaceStatus?.provider?.baseURL
            ?? onboardingBaseURL
    }

    public var currentProviderKindLabel: String {
        selectedConfigurationSummary?.provider.kind
            ?? selectedWorkspaceStatus?.provider?.kind
            ?? "openai_compatible"
    }

    public var currentProviderLabel: String {
        providerLabel(forBaseURL: currentBaseURLLabel, fallbackKind: currentProviderKindLabel)
    }

    public var currentAPIKeyEnvLabel: String {
        selectedConfigurationSummary?.provider.apiKeyEnv
            ?? selectedModelsStatus?.apiKeyEnv
            ?? selectedWorkspaceStatus?.provider?.apiKeyEnv
            ?? onboardingAPIKeyEnv
    }

    public var canInitializeWorkspace: Bool {
        selectedWorkspace != nil
            && !selectedWorkspaceConfigured
            && !isPreparingWorkspace
            && !isInitializingWorkspace
            && !onboardingRequiresAzureResourceHost
            && !onboardingBaseURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !onboardingModel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !onboardingAPIKeyEnv.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    public var canOpenWorkspace: Bool {
        selectedWorkspaceConfigured && !isPreparingWorkspace && !isInitializingWorkspace
    }

    public func selectOnboardingModel(_ modelID: String) {
        onboardingModel = modelID
    }

    public func selectRuntimeModel(_ modelID: String) {
        runtimeDraftModel = modelID
    }

    public var onboardingRequiresAzureResourceHost: Bool {
        let normalized = onboardingBaseURL.lowercased()
        return normalized.contains("your_resource_name") || normalized.contains("<resource>")
    }

    public var runtimeDraftRequiresAzureResourceHost: Bool {
        let normalized = runtimeDraftBaseURL.lowercased()
        return normalized.contains("your_resource_name") || normalized.contains("<resource>")
    }

    public var onboardingProviderLabel: String {
        providerLabel(forBaseURL: onboardingBaseURL, fallbackKind: "openai_compatible")
    }

    public var runtimeDraftProviderLabel: String {
        providerLabel(forBaseURL: runtimeDraftBaseURL, fallbackKind: currentProviderKindLabel)
    }

    public var onboardingUsesAzurePreset: Bool {
        baseURLUsesAzure(onboardingBaseURL)
    }

    public var runtimeDraftUsesAzurePreset: Bool {
        baseURLUsesAzure(runtimeDraftBaseURL)
    }

    public var onboardingAzureResourceName: String {
        get { azureResourceName(from: onboardingBaseURL) ?? "" }
        set { onboardingBaseURL = buildAzureBaseURL(resourceName: newValue) }
    }

    public var runtimeDraftAzureResourceName: String {
        get { azureResourceName(from: runtimeDraftBaseURL) ?? "" }
        set { runtimeDraftBaseURL = buildAzureBaseURL(resourceName: newValue) }
    }

    public func applyAzurePreset() {
        onboardingBaseURL = OnboardingDefaults.azureBaseURL
        onboardingModel = OnboardingDefaults.azureModel
        onboardingAPIKeyEnv = OnboardingDefaults.azureAPIKeyEnv
    }

    public func applyLocalPreset() {
        onboardingBaseURL = OnboardingDefaults.localBaseURL
        onboardingAPIKeyEnv = OnboardingDefaults.localAPIKeyEnv
    }

    public func applyAzureRuntimePreset() {
        runtimeDraftBaseURL = OnboardingDefaults.azureBaseURL
        runtimeDraftModel = OnboardingDefaults.azureModel
        runtimeDraftAPIKeyEnv = OnboardingDefaults.azureAPIKeyEnv
    }

    public func applyLocalRuntimePreset() {
        runtimeDraftBaseURL = OnboardingDefaults.localBaseURL
        runtimeDraftAPIKeyEnv = OnboardingDefaults.localAPIKeyEnv
    }

    public var runtimeDraftHasChanges: Bool {
        runtimeDraftBaseURL.trimmingCharacters(in: .whitespacesAndNewlines) != currentBaseURLLabel
            || runtimeDraftModel.trimmingCharacters(in: .whitespacesAndNewlines) != currentModelLabel
            || runtimeDraftAPIKeyEnv.trimmingCharacters(in: .whitespacesAndNewlines) != currentAPIKeyEnvLabel
    }

    public var canSaveRuntimeSettings: Bool {
        selectedWorkspaceConfigured
            && !isPreparingWorkspace
            && !isInitializingWorkspace
            && !isSavingRuntimeSettings
            && !isApplyingQuickModel
            && !runtimeDraftRequiresAzureResourceHost
            && runtimeDraftHasChanges
    }

    public var canQuickSwitchModels: Bool {
        selectedWorkspaceConfigured
            && !isPreparingWorkspace
            && !isInitializingWorkspace
            && !isSavingRuntimeSettings
            && !isApplyingQuickModel
    }

    public func resetRuntimeDraft() {
        runtimeDraftBaseURL = currentBaseURLLabel
        runtimeDraftModel = currentModelLabel
        runtimeDraftAPIKeyEnv = currentAPIKeyEnvLabel
    }

    public func saveRuntimeSettings() async {
        guard let workspace = selectedWorkspace, selectedWorkspaceConfigured else { return }

        let trimmedBaseURL = runtimeDraftBaseURL.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedModel = runtimeDraftModel.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedAPIKeyEnv = runtimeDraftAPIKeyEnv.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !trimmedBaseURL.isEmpty, !trimmedModel.isEmpty, !trimmedAPIKeyEnv.isEmpty else {
            globalError = "Base URL, model, and API key env must not be empty."
            return
        }
        guard !runtimeDraftRequiresAzureResourceHost else {
            globalError = "Replace the Azure resource placeholder in the base URL before saving runtime settings."
            return
        }

        isSavingRuntimeSettings = true
        defer { isSavingRuntimeSettings = false }

        do {
            if trimmedBaseURL != currentBaseURLLabel {
                try await runtimeClient.configureSet(
                    workspace: workspace,
                    key: .providerBaseURL,
                    value: trimmedBaseURL
                )
            }

            if trimmedAPIKeyEnv != currentAPIKeyEnvLabel {
                try await runtimeClient.configureSet(
                    workspace: workspace,
                    key: .providerAPIKeyEnv,
                    value: trimmedAPIKeyEnv
                )
            }

            if trimmedModel != currentModelLabel {
                _ = try await runtimeClient.setModel(workspace: workspace, model: trimmedModel)
            }

            await prepareWorkspace(workspace, openWorkbenchIfConfigured: screen == .workbench)
        } catch {
            globalError = error.localizedDescription
        }
    }

    public func quickSwitchModel(_ modelID: String) async {
        guard let workspace = selectedWorkspace, selectedWorkspaceConfigured else { return }
        let trimmedModel = modelID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedModel.isEmpty, trimmedModel != currentModelLabel else { return }

        isApplyingQuickModel = true
        defer { isApplyingQuickModel = false }

        do {
            _ = try await runtimeClient.setModel(workspace: workspace, model: trimmedModel)
            await prepareWorkspace(workspace, openWorkbenchIfConfigured: screen == .workbench)
        } catch {
            globalError = error.localizedDescription
        }
    }

    private func prepareWorkspace(_ workspace: WorkspaceReference, openWorkbenchIfConfigured: Bool) async {
        selectedWorkspace = workspace
        recentWorkspaces = await workspaceStore.recentWorkspaces()
        isPreparingWorkspace = true
        defer { isPreparingWorkspace = false }
        do {
            let status = try await loadWorkspaceSummary(workspace: workspace)

            if status.configured && openWorkbenchIfConfigured {
                if workbench?.workspaceReference.id != workspace.id {
                    workbench = WorkbenchViewModel(
                        workspace: workspace,
                        recentWorkspaces: recentWorkspaces,
                        runtimeClient: runtimeClient
                    )
                }
                screen = .workbench
                await workbench?.refresh()
            } else {
                workbench = nil
                screen = .setupHub
            }
        } catch {
            selectedWorkspaceStatus = nil
            selectedWorkspaceHealth = nil
            selectedModelsStatus = nil
            selectedConfigurationSummary = nil
            availableModels = []
            workbench = nil
            screen = .setupHub
            globalError = error.localizedDescription
        }
    }

    private func loadWorkspaceSummary(workspace: WorkspaceReference) async throws -> RuntimeStatusSummary {
        let status = try await runtimeClient.status(workspace: workspace)
        selectedWorkspaceStatus = status
        if status.configured {
            selectedModelsStatus = try? await runtimeClient.modelsStatus(workspace: workspace)
            selectedWorkspaceHealth = try? await runtimeClient.health(workspace: workspace)
            selectedConfigurationSummary = try? await runtimeClient.configureShow(workspace: workspace)
            availableModels = (try? await runtimeClient.modelsList(workspace: workspace)) ?? []

            if let configured = selectedConfigurationSummary {
                onboardingBaseURL = configured.provider.baseURL
                onboardingModel = configured.provider.model
                onboardingAPIKeyEnv = configured.provider.apiKeyEnv
            } else if let modelsStatus = selectedModelsStatus {
                onboardingBaseURL = modelsStatus.baseURL
                onboardingModel = modelsStatus.effectiveModel
                onboardingAPIKeyEnv = modelsStatus.apiKeyEnv
            }

            runtimeDraftBaseURL = currentBaseURLLabel
            runtimeDraftModel = currentModelLabel
            runtimeDraftAPIKeyEnv = currentAPIKeyEnvLabel
        } else {
            selectedModelsStatus = nil
            selectedWorkspaceHealth = nil
            selectedConfigurationSummary = nil
            availableModels = []
            runtimeDraftBaseURL = onboardingBaseURL
            runtimeDraftModel = onboardingModel
            runtimeDraftAPIKeyEnv = onboardingAPIKeyEnv
        }
        return status
    }

    private func providerLabel(forBaseURL baseURL: String, fallbackKind: String) -> String {
        if baseURLUsesAzure(baseURL) {
            return "Azure OpenAI"
        }
        if baseURLUsesLocalRuntime(baseURL) {
            return "Local Server"
        }
        switch fallbackKind.lowercased() {
        case "openai_compatible":
            return "OpenAI-Compatible"
        default:
            return fallbackKind.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }

    private func baseURLUsesAzure(_ baseURL: String) -> Bool {
        let normalized = baseURL.lowercased()
        return normalized.contains(".openai.azure.com")
            || normalized.contains("your_resource_name")
            || normalized.contains("<resource>")
    }

    private func baseURLUsesLocalRuntime(_ baseURL: String) -> Bool {
        let normalized = baseURL.lowercased()
        return normalized.contains("localhost") || normalized.contains("127.0.0.1")
    }

    private func azureResourceName(from baseURL: String) -> String? {
        let normalized = baseURL
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()

        guard normalized.contains(".openai.azure.com") || normalized.contains("your_resource_name") else {
            return nil
        }

        let strippedScheme = normalized
            .replacingOccurrences(of: "https://", with: "")
            .replacingOccurrences(of: "http://", with: "")
        let host = strippedScheme.components(separatedBy: "/").first ?? strippedScheme
        let resource = host.replacingOccurrences(of: ".openai.azure.com", with: "")
        return resource == "your_resource_name" ? "" : resource
    }

    private func buildAzureBaseURL(resourceName: String) -> String {
        var normalized = resourceName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: "https://", with: "")
            .replacingOccurrences(of: "http://", with: "")

        normalized = normalized.components(separatedBy: "/").first ?? normalized
        normalized = normalized.replacingOccurrences(of: ".openai.azure.com", with: "")
        if normalized.isEmpty {
            normalized = "YOUR_RESOURCE_NAME"
        }

        return "https://\(normalized).openai.azure.com/openai/v1"
    }
}
