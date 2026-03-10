import Domain
import Foundation

public actor WorkspaceStore: WorkspaceStoring {
    private let defaults: UserDefaults
    private let workspacesKey = "macos.workspaces"
    private let selectedWorkspaceKey = "macos.selected-workspace-id"

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func recentWorkspaces() async -> [WorkspaceReference] {
        loadWorkspaces().sorted { lhs, rhs in
            (lhs.lastOpenedAt ?? .distantPast) > (rhs.lastOpenedAt ?? .distantPast)
        }
    }

    public func selectedWorkspace() async -> WorkspaceReference? {
        guard
            let raw = defaults.string(forKey: selectedWorkspaceKey),
            let id = UUID(uuidString: raw)
        else {
            return nil
        }
        return loadWorkspaces().first(where: { $0.id == id })
    }

    public func save(workspace: WorkspaceReference) async {
        var workspaces = loadWorkspaces()
        let updated = WorkspaceReference(
            id: workspace.id,
            name: workspace.name,
            path: workspace.path,
            lastOpenedAt: Date()
        )
        if let index = workspaces.firstIndex(where: { $0.id == workspace.id }) {
            workspaces[index] = updated
        } else {
            workspaces.append(updated)
        }
        persist(workspaces)
        defaults.set(updated.id.uuidString, forKey: selectedWorkspaceKey)
    }

    public func select(workspaceID: UUID) async {
        defaults.set(workspaceID.uuidString, forKey: selectedWorkspaceKey)
    }

    private func loadWorkspaces() -> [WorkspaceReference] {
        guard
            let data = defaults.data(forKey: workspacesKey),
            let decoded = try? JSONDecoder().decode([WorkspaceReference].self, from: data)
        else {
            return []
        }
        return decoded
    }

    private func persist(_ workspaces: [WorkspaceReference]) {
        if let data = try? JSONEncoder().encode(workspaces) {
            defaults.set(data, forKey: workspacesKey)
        }
    }
}

public actor InMemoryWorkspaceStore: WorkspaceStoring {
    private var workspaces: [WorkspaceReference]
    private var selectedID: UUID?

    public init(
        workspaces: [WorkspaceReference] = [],
        selectedID: UUID? = nil
    ) {
        self.workspaces = workspaces
        self.selectedID = selectedID
    }

    public func recentWorkspaces() async -> [WorkspaceReference] { workspaces }

    public func selectedWorkspace() async -> WorkspaceReference? {
        guard let selectedID else { return nil }
        return workspaces.first(where: { $0.id == selectedID })
    }

    public func save(workspace: WorkspaceReference) async {
        if let index = workspaces.firstIndex(where: { $0.id == workspace.id }) {
            workspaces[index] = workspace
        } else {
            workspaces.append(workspace)
        }
        selectedID = workspace.id
    }

    public func select(workspaceID: UUID) async {
        selectedID = workspaceID
    }
}
