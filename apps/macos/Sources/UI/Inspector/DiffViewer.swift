import Domain
import SwiftUI

struct DiffViewer: View {
    let change: FileChange
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let lines = DiffParser.parse(change.diff)

        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(change.path)
                        .font(.system(size: 10.5, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                    if let previousPath = change.previousPath {
                        Text("from \(previousPath)")
                            .font(.system(size: 9.5))
                            .foregroundStyle(tokens.secondaryText)
                    }
                }

                Spacer()

                HStack(spacing: 10) {
                    Text("+\(change.additions)")
                        .foregroundStyle(tokens.success)
                    Text("-\(change.deletions)")
                        .foregroundStyle(tokens.failure)
                    Text(change.status.rawValue.uppercased())
                        .foregroundStyle(tokens.tertiaryText)
                }
                .font(.system(size: 9.5, weight: .semibold, design: .monospaced))
            }

            if change.isBinary {
                Text("Binary file change. Diff preview is unavailable.")
                    .font(.system(size: 11.5))
                    .foregroundStyle(tokens.secondaryText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
                    .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            } else if lines.isEmpty {
                Text("No diff preview available.")
                    .font(.system(size: 11.5))
                    .foregroundStyle(tokens.secondaryText)
            } else {
                ScrollView([.horizontal, .vertical], showsIndicators: true) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(lines) { line in
                            DiffLineRow(line: line)
                        }
                    }
                }
                .frame(minHeight: 220, maxHeight: 332)
                .background(tokens.codeBackground, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 9, style: .continuous)
                        .stroke(tokens.border, lineWidth: 1)
                )
            }
        }
    }
}

private struct DiffLineRow: View {
    let line: DiffLine
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 0) {
            lineNumber(line.oldLine, foreground: lineNumberColor(tokens: tokens))
            lineNumber(line.newLine, foreground: lineNumberColor(tokens: tokens))

            Text(line.text)
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(textColor(tokens: tokens))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 9)
                .padding(.vertical, 2.5)
        }
        .background(background(tokens: tokens))
    }

    private func lineNumber(_ value: Int?, foreground: Color) -> some View {
        Text(value.map(String.init) ?? "")
            .font(.system(size: 10, weight: .medium, design: .monospaced))
            .foregroundStyle(foreground)
            .frame(width: 34, alignment: .trailing)
            .padding(.vertical, 2.5)
            .padding(.trailing, 7)
    }

    private func lineNumberColor(tokens: ThemeTokens) -> Color {
        switch line.kind {
        case .added:
            return tokens.success.opacity(0.8)
        case .removed:
            return tokens.failure.opacity(0.8)
        default:
            return tokens.tertiaryText
        }
    }

    private func textColor(tokens: ThemeTokens) -> Color {
        switch line.kind {
        case .hunk:
            return tokens.accent
        case .meta:
            return tokens.secondaryText
        default:
            return tokens.primaryText
        }
    }

    private func background(tokens: ThemeTokens) -> Color {
        switch line.kind {
        case .added:
            return tokens.success.opacity(0.11)
        case .removed:
            return tokens.failure.opacity(0.11)
        case .hunk:
            return tokens.accentMuted.opacity(0.2)
        case .meta:
            return tokens.elevatedBackground.opacity(0.52)
        case .context:
            return .clear
        }
    }
}

private struct DiffLine: Identifiable {
    enum Kind {
        case meta
        case hunk
        case context
        case added
        case removed
    }

    let id = UUID()
    let kind: Kind
    let oldLine: Int?
    let newLine: Int?
    let text: String
}

private enum DiffParser {
    static func parse(_ diff: String) -> [DiffLine] {
        guard !diff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return [] }

        var oldLineNumber: Int?
        var newLineNumber: Int?
        let hunkRegex = try? NSRegularExpression(pattern: #"@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@"#)

        return diff.components(separatedBy: .newlines).map { line in
            if line.hasPrefix("@@") {
                if let hunkRegex,
                   let match = hunkRegex.firstMatch(in: line, range: NSRange(location: 0, length: line.utf16.count)),
                   let oldRange = Range(match.range(at: 1), in: line),
                   let newRange = Range(match.range(at: 2), in: line) {
                    oldLineNumber = Int(line[oldRange])
                    newLineNumber = Int(line[newRange])
                }
                return DiffLine(kind: .hunk, oldLine: nil, newLine: nil, text: line)
            }

            if line.hasPrefix("diff --git")
                || line.hasPrefix("index ")
                || line.hasPrefix("--- ")
                || line.hasPrefix("+++ ")
                || line == "\\ No newline at end of file" {
                return DiffLine(kind: .meta, oldLine: nil, newLine: nil, text: line)
            }

            if line.hasPrefix("+"), !line.hasPrefix("+++") {
                let current = DiffLine(kind: .added, oldLine: nil, newLine: newLineNumber, text: line)
                newLineNumber = (newLineNumber ?? 0) + 1
                return current
            }

            if line.hasPrefix("-"), !line.hasPrefix("---") {
                let current = DiffLine(kind: .removed, oldLine: oldLineNumber, newLine: nil, text: line)
                oldLineNumber = (oldLineNumber ?? 0) + 1
                return current
            }

            let current = DiffLine(kind: .context, oldLine: oldLineNumber, newLine: newLineNumber, text: line)
            if oldLineNumber != nil {
                oldLineNumber = (oldLineNumber ?? 0) + 1
            }
            if newLineNumber != nil {
                newLineNumber = (newLineNumber ?? 0) + 1
            }
            return current
        }
    }
}
