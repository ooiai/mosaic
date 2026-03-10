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
        let normalizedPath = normalize(path: workspace.path)
        let updated = WorkspaceReference(
            id: workspace.id,
            name: workspace.name,
            path: workspace.path,
            lastOpenedAt: Date()
        )
        if let index = workspaces.firstIndex(where: {
            $0.id == workspace.id || normalize(path: $0.path) == normalizedPath
        }) {
            let preservedID = workspaces[index].id
            workspaces[index] = WorkspaceReference(
                id: preservedID,
                name: updated.name,
                path: updated.path,
                lastOpenedAt: updated.lastOpenedAt
            )
        } else {
            workspaces.append(updated)
        }
        persist(workspaces)
        defaults.set((workspaces.first { normalize(path: $0.path) == normalizedPath }?.id ?? updated.id).uuidString, forKey: selectedWorkspaceKey)
    }

    public func select(workspaceID: UUID) async {
        var workspaces = loadWorkspaces()
        if let index = workspaces.firstIndex(where: { $0.id == workspaceID }) {
            workspaces[index].lastOpenedAt = Date()
            persist(workspaces)
        }
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

    private func normalize(path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
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
        let normalizedPath = URL(fileURLWithPath: workspace.path).standardizedFileURL.path
        if let index = workspaces.firstIndex(where: {
            $0.id == workspace.id || URL(fileURLWithPath: $0.path).standardizedFileURL.path == normalizedPath
        }) {
            workspaces[index] = workspace
        } else {
            workspaces.append(workspace)
        }
        selectedID = workspace.id
    }

    public func select(workspaceID: UUID) async {
        if let index = workspaces.firstIndex(where: { $0.id == workspaceID }) {
            workspaces[index].lastOpenedAt = Date()
        }
        selectedID = workspaceID
    }
}
