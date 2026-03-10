import Domain
import Foundation

public actor CommandHistoryStore: CommandHistoryStoring {
    private let defaults: UserDefaults
    private let commandHistoryKey = "macos.command-history"
    private let maxEntries: Int

    public init(defaults: UserDefaults = .standard, maxEntries: Int = 8) {
        self.defaults = defaults
        self.maxEntries = maxEntries
    }

    public func recentCommandActionIDs() async -> [String] {
        load()
    }

    public func recordCommandActionID(_ actionID: String) async {
        let normalized = actionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }

        var entries = load()
        entries.removeAll { $0 == normalized }
        entries.insert(normalized, at: 0)
        if entries.count > maxEntries {
            entries = Array(entries.prefix(maxEntries))
        }
        defaults.set(entries, forKey: commandHistoryKey)
    }

    public func clearRecentCommandActionIDs() async {
        defaults.removeObject(forKey: commandHistoryKey)
    }

    private func load() -> [String] {
        (defaults.array(forKey: commandHistoryKey) as? [String]) ?? []
    }
}

public actor InMemoryCommandHistoryStore: CommandHistoryStoring {
    private var entries: [String]
    private let maxEntries: Int

    public init(entries: [String] = [], maxEntries: Int = 8) {
        self.entries = entries
        self.maxEntries = maxEntries
    }

    public func recentCommandActionIDs() async -> [String] {
        entries
    }

    public func recordCommandActionID(_ actionID: String) async {
        let normalized = actionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }
        entries.removeAll { $0 == normalized }
        entries.insert(normalized, at: 0)
        if entries.count > maxEntries {
            entries = Array(entries.prefix(maxEntries))
        }
    }

    public func clearRecentCommandActionIDs() async {
        entries = []
    }
}
