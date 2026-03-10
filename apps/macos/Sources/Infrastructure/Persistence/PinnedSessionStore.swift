import Domain
import Foundation

public actor PinnedSessionStore: PinnedSessionsStoring {
    private let defaults: UserDefaults
    private let storageKey = "macos.pinned-sessions"

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func pinnedSessionIDs(for workspaceID: UUID) async -> [String] {
        load()[workspaceID.uuidString] ?? []
    }

    public func setPinnedSessionID(_ sessionID: String, pinned: Bool, workspaceID: UUID) async {
        let normalized = sessionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }

        var state = load()
        var entries = state[workspaceID.uuidString] ?? []
        entries.removeAll { $0 == normalized }
        if pinned {
            entries.insert(normalized, at: 0)
        }

        if entries.isEmpty {
            state.removeValue(forKey: workspaceID.uuidString)
        } else {
            state[workspaceID.uuidString] = entries
        }

        defaults.set(state, forKey: storageKey)
    }

    private func load() -> [String: [String]] {
        defaults.dictionary(forKey: storageKey) as? [String: [String]] ?? [:]
    }
}

public actor InMemoryPinnedSessionStore: PinnedSessionsStoring {
    private var state: [UUID: [String]]

    public init(state: [UUID: [String]] = [:]) {
        self.state = state
    }

    public func pinnedSessionIDs(for workspaceID: UUID) async -> [String] {
        state[workspaceID] ?? []
    }

    public func setPinnedSessionID(_ sessionID: String, pinned: Bool, workspaceID: UUID) async {
        let normalized = sessionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }

        var entries = state[workspaceID] ?? []
        entries.removeAll { $0 == normalized }
        if pinned {
            entries.insert(normalized, at: 0)
        }
        state[workspaceID] = entries
    }
}
