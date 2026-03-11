import Domain
import Foundation

public struct WorkspaceRepositorySnapshot: Sendable {
    public var entries: [String: FileChange]

    public init(entries: [String: FileChange] = [:]) {
        self.entries = entries
    }
}

public actor WorkspaceGitInspector {
    private let runner: CLIProcessRunner

    public init(runner: CLIProcessRunner = CLIProcessRunner()) {
        self.runner = runner
    }

    public func capture(project: Project) async -> WorkspaceRepositorySnapshot {
        let workspaceURL = URL(fileURLWithPath: project.workspacePath, isDirectory: true)
        let isGitRepo = await canRunGit(in: workspaceURL)
        guard isGitRepo else { return WorkspaceRepositorySnapshot() }

        async let statusOutput = runGit(["status", "--porcelain"], in: workspaceURL)
        async let numstatOutput = runGit(["diff", "--numstat"], in: workspaceURL)
        async let diffOutput = runGit(["diff", "--no-ext-diff", "--minimal"], in: workspaceURL)

        let statusText = await statusOutput ?? ""
        let numstatText = await numstatOutput ?? ""
        let diffText = await diffOutput ?? ""

        let stats = parseNumStat(numstatText)
        let diffs = parseUnifiedDiff(diffText)
        let statuses = parseStatus(statusText)

        var entries: [String: FileChange] = [:]
        let allPaths = Set(statuses.keys)
            .union(stats.keys)
            .union(diffs.keys)

        for path in allPaths {
            let status = statuses[path]?.status ?? .modified
            let previousPath = statuses[path]?.previousPath
            let changeStats = stats[path] ?? (0, 0)
            entries[path] = FileChange(
                path: path,
                previousPath: previousPath,
                status: status,
                additions: changeStats.0,
                deletions: changeStats.1,
                diff: diffs[path] ?? "",
                isBinary: changeStats.0 < 0 || changeStats.1 < 0
            )
        }

        return WorkspaceRepositorySnapshot(entries: entries)
    }

    public func changes(
        from before: WorkspaceRepositorySnapshot,
        to after: WorkspaceRepositorySnapshot
    ) -> [FileChange] {
        let allPaths = Set(before.entries.keys).union(after.entries.keys)
        return allPaths.compactMap { path in
            let old = before.entries[path]
            let new = after.entries[path]
            guard old != new else { return nil }
            return new ?? old
        }
        .sorted { $0.path < $1.path }
    }

    private func canRunGit(in workspaceURL: URL) async -> Bool {
        guard let output = await runGit(["rev-parse", "--is-inside-work-tree"], in: workspaceURL) else {
            return false
        }
        return output.trimmingCharacters(in: .whitespacesAndNewlines) == "true"
    }

    private func runGit(_ arguments: [String], in workspaceURL: URL) async -> String? {
        let gitURL = URL(fileURLWithPath: "/usr/bin/git")
        guard FileManager.default.fileExists(atPath: gitURL.path) else { return nil }
        guard let output = try? await runner.run(
            executableURL: gitURL,
            arguments: arguments,
            workingDirectory: workspaceURL,
            timeout: .seconds(10)
        ) else {
            return nil
        }
        guard output.exitCode == 0 else { return nil }
        return String(data: output.stdout, encoding: .utf8)
    }

    private func parseStatus(_ text: String) -> [String: (status: FileChangeStatus, previousPath: String?)] {
        Dictionary(uniqueKeysWithValues: text.split(separator: "\n").compactMap { rawLine in
            let line = String(rawLine)
            guard line.count >= 4 else { return nil }
            let statusCode = String(line.prefix(2))
            let pathPayload = String(line.dropFirst(3))
            let parts = pathPayload.components(separatedBy: " -> ")
            let path = parts.last ?? pathPayload
            return (path, (status(for: statusCode), parts.count == 2 ? parts.first : nil))
        })
    }

    private func status(for code: String) -> FileChangeStatus {
        switch code.trimmingCharacters(in: .whitespaces) {
        case "A": .added
        case "M": .modified
        case "D": .deleted
        case "R": .renamed
        case "??": .untracked
        default: .modified
        }
    }

    private func parseNumStat(_ text: String) -> [String: (Int, Int)] {
        Dictionary(uniqueKeysWithValues: text.split(separator: "\n").compactMap { rawLine in
            let parts = rawLine.split(separator: "\t")
            guard parts.count >= 3 else { return nil }
            let additions = Int(parts[0]) ?? -1
            let deletions = Int(parts[1]) ?? -1
            return (String(parts[2]), (max(additions, 0), max(deletions, 0)))
        })
    }

    private func parseUnifiedDiff(_ text: String) -> [String: String] {
        var result: [String: String] = [:]
        var currentPath: String?
        var currentLines: [String] = []

        for line in text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init) {
            if line.hasPrefix("diff --git") {
                if let currentPath {
                    result[currentPath] = currentLines.joined(separator: "\n")
                }
                currentLines = [line]
                currentPath = parseDiffPath(line)
                continue
            }
            guard currentPath != nil else { continue }
            currentLines.append(line)
        }

        if let currentPath {
            result[currentPath] = currentLines.joined(separator: "\n")
        }
        return result
    }

    private func parseDiffPath(_ header: String) -> String? {
        let parts = header.split(separator: " ")
        guard parts.count >= 4 else { return nil }
        let rawPath = String(parts[3]).replacingOccurrences(of: "b/", with: "")
        return rawPath
    }
}
