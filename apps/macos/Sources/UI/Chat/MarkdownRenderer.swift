import AppKit
import Domain
import SwiftUI

private enum RenderedBlock: Identifiable, Hashable {
    case markdown(UUID, String)
    case divider(UUID)
    case code(UUID, String?, String)
    case log(UUID, String)
    case status(UUID, String)
    case table(UUID, [String], [[String]])
    case image(UUID, String, String?)

    var id: UUID {
        switch self {
        case let .markdown(id, _): id
        case let .divider(id): id
        case let .code(id, _, _): id
        case let .log(id, _): id
        case let .status(id, _): id
        case let .table(id, _, _): id
        case let .image(id, _, _): id
        }
    }
}

struct MarkdownRenderer: View {
    let text: String
    let settings: AppSettings
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let blocks = MarkdownParser.parse(text)
        VStack(alignment: .leading, spacing: 10) {
            ForEach(blocks) { block in
                switch block {
                case let .markdown(_, markdown):
                    MarkdownTextBlock(text: markdown)
                case .divider:
                    Divider()
                case let .code(_, language, code):
                    CodeBlockView(language: language, code: code, settings: settings)
                case let .log(_, log):
                    LogBlockView(text: log, settings: settings)
                case let .status(_, status):
                    StatusBlockView(text: status)
                case let .table(_, headers, rows):
                    MarkdownTableView(headers: headers, rows: rows)
                case let .image(_, url, alt):
                    if settings.markdown.renderImages {
                        MarkdownImageView(urlString: url, alt: alt)
                    } else {
                        MarkdownTextBlock(text: alt ?? url)
                    }
                }
            }
        }
    }
}

private struct MarkdownTextBlock: View {
    let text: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        if let attributed = try? AttributedString(markdown: text) {
            Text(attributed)
                .font(.system(size: 14.5))
                .foregroundStyle(tokens.primaryText)
                .tint(tokens.accent)
                .lineSpacing(4)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            Text(text)
                .font(.system(size: 14.5))
                .lineSpacing(4)
                .foregroundStyle(tokens.primaryText)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct CodeBlockView: View {
    let language: String?
    let code: String
    let settings: AppSettings
    @Environment(\.colorScheme) private var colorScheme
    @State private var expanded = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let lines = code.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        let collapse = settings.markdown.collapseLongContent && lines.count > 24 && !expanded
        let displayedLines = collapse ? Array(lines.prefix(24)) : lines
        let displayed = displayedLines.joined(separator: "\n")

        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text((language ?? "code").uppercased())
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Text("· \(lines.count) lines")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Spacer()
                if collapse {
                    BlockHeaderAction(title: "Show More", accent: tokens.accent) { expanded = true }
                }
                BlockHeaderAction(title: "Copy", accent: tokens.accent) {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(code, forType: .string)
                }
            }

            Group {
                if settings.markdown.wrapCode {
                    codeBody(displayedLines: displayedLines, displayed: displayed, tokens: tokens)
                } else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        codeBody(displayedLines: displayedLines, displayed: displayed, tokens: tokens)
                            .fixedSize(horizontal: true, vertical: true)
                    }
                }
            }
        }
        .padding(12)
        .background(tokens.codeBackground, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }

    private func codeBody(displayedLines: [String], displayed: String, tokens: ThemeTokens) -> some View {
        HStack(alignment: .top, spacing: 12) {
            if settings.markdown.showLineNumbers {
                Text(lineNumbers(count: displayedLines.count))
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                    .multilineTextAlignment(.trailing)
            }

            Text(CodeSyntaxHighlighter.highlight(displayed, language: language, tokens: tokens))
                .font(.system(size: 12, design: .monospaced))
                .lineSpacing(2)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func lineNumbers(count: Int) -> String {
        (1...max(count, 1)).map(String.init).joined(separator: "\n")
    }
}

private struct LogBlockView: View {
    let text: String
    let settings: AppSettings
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)

        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("LOG")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Text("· \(lines.count) lines")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Spacer()
                BlockHeaderAction(title: "Copy", accent: tokens.accent) {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                }
            }

            Group {
                if settings.markdown.wrapCode {
                    logBody(lines: lines, tokens: tokens)
                } else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        logBody(lines: lines, tokens: tokens)
                            .fixedSize(horizontal: true, vertical: true)
                    }
                }
            }
        }
        .padding(12)
        .background(tokens.logBackground, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }

    private func logBody(lines: [String], tokens: ThemeTokens) -> some View {
        HStack(alignment: .top, spacing: 12) {
            if settings.markdown.showLineNumbers {
                Text((1...max(lines.count, 1)).map(String.init).joined(separator: "\n"))
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                    .multilineTextAlignment(.trailing)
            }

            Text(text)
                .font(.system(size: 12, design: .monospaced))
                .lineSpacing(2)
                .foregroundStyle(tokens.secondaryText)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct StatusBlockView: View {
    let text: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "bolt.circle.fill")
                .foregroundStyle(tokens.accent)
            VStack(alignment: .leading, spacing: 4) {
                Text("STATUS")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
                Text(text)
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.primaryText)
                    .textSelection(.enabled)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tokens.accentMuted.opacity(colorScheme == .dark ? 0.55 : 0.75), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(tokens.accent.opacity(0.18), lineWidth: 1)
        )
    }
}

private struct MarkdownTableView: View {
    let headers: [String]
    let rows: [[String]]
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(spacing: 0) {
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: headers.count), spacing: 0) {
                ForEach(headers, id: \.self) { header in
                    Text(header)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(8)
                        .background(tokens.elevatedBackground)
                }
                ForEach(rows.indices, id: \.self) { rowIndex in
                    ForEach(rows[rowIndex].indices, id: \.self) { columnIndex in
                        Text(rows[rowIndex][columnIndex])
                            .font(.system(size: 11))
                            .foregroundStyle(tokens.secondaryText)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(8)
                    }
                }
            }
        }
        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}

private struct BlockHeaderAction: View {
    let title: String
    let accent: Color
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            Text(title)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(accent)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(
                    (isHovered ? tokens.panelBackground.opacity(0.9) : tokens.panelBackground.opacity(0.55)),
                    in: Capsule()
                )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct MarkdownImageView: View {
    let urlString: String
    let alt: String?
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        if let url = URL(string: urlString) {
            AsyncImage(url: url) { image in
                image
                    .resizable()
                    .scaledToFit()
            } placeholder: {
                ProgressView()
            }
            .frame(maxHeight: 240)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(8)
            .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        } else {
            Text(alt ?? urlString)
                .foregroundStyle(tokens.secondaryText)
        }
    }
}

private enum MarkdownParser {
    static func parse(_ text: String) -> [RenderedBlock] {
        let lines = text.components(separatedBy: .newlines)
        var blocks: [RenderedBlock] = []
        var currentMarkdown: [String] = []
        var codeLanguage: String?
        var codeLines: [String] = []
        var inCode = false
        var index = 0

        func flushMarkdown() {
            guard !currentMarkdown.isEmpty else { return }
            let joined = currentMarkdown.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
            if !joined.isEmpty {
                blocks.append(.markdown(UUID(), joined))
            }
            currentMarkdown.removeAll()
        }

        while index < lines.count {
            let line = lines[index]

            if line.hasPrefix("```") {
                if inCode {
                    let content = codeLines.joined(separator: "\n")
                    let language = codeLanguage?.lowercased()
                    if language == "log" || language == "console" {
                        blocks.append(.log(UUID(), content))
                    } else if language == "status" {
                        blocks.append(.status(UUID(), content))
                    } else {
                        blocks.append(.code(UUID(), language, content))
                    }
                    codeLines.removeAll()
                    codeLanguage = nil
                    inCode = false
                } else {
                    flushMarkdown()
                    inCode = true
                    codeLanguage = String(line.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
                }
                index += 1
                continue
            }

            if inCode {
                codeLines.append(line)
                index += 1
                continue
            }

            if line.trimmingCharacters(in: .whitespaces) == "---" {
                flushMarkdown()
                blocks.append(.divider(UUID()))
                index += 1
                continue
            }

            if let image = parseImage(line) {
                flushMarkdown()
                blocks.append(.image(UUID(), image.url, image.alt))
                index += 1
                continue
            }

            if isTableHeader(line), index + 1 < lines.count, isTableDivider(lines[index + 1]) {
                flushMarkdown()
                let headers = splitTableRow(line)
                var rows: [[String]] = []
                index += 2
                while index < lines.count, lines[index].contains("|") {
                    rows.append(splitTableRow(lines[index]))
                    index += 1
                }
                blocks.append(.table(UUID(), headers, rows))
                continue
            }

            currentMarkdown.append(line)
            index += 1
        }

        flushMarkdown()
        return blocks
    }

    private static func parseImage(_ line: String) -> (alt: String?, url: String)? {
        let pattern = #"!\[(.*?)\]\((.*?)\)"#
        guard
            let regex = try? NSRegularExpression(pattern: pattern),
            let match = regex.firstMatch(in: line, range: NSRange(location: 0, length: line.utf16.count)),
            let altRange = Range(match.range(at: 1), in: line),
            let urlRange = Range(match.range(at: 2), in: line)
        else {
            return nil
        }
        return (String(line[altRange]), String(line[urlRange]))
    }

    private static func isTableHeader(_ line: String) -> Bool {
        line.contains("|") && splitTableRow(line).count > 1
    }

    private static func isTableDivider(_ line: String) -> Bool {
        line.replacingOccurrences(of: "|", with: "")
            .trimmingCharacters(in: .whitespaces)
            .allSatisfy { $0 == "-" || $0 == ":" }
    }

    private static func splitTableRow(_ line: String) -> [String] {
        line.split(separator: "|").map { $0.trimmingCharacters(in: .whitespaces) }
    }
}

private enum CodeSyntaxHighlighter {
    static func highlight(_ code: String, language: String?, tokens: ThemeTokens) -> AttributedString {
        var attributed = AttributedString(code)
        attributed.foregroundColor = tokens.primaryText
        attributed.font = .system(size: 12, design: .monospaced)

        let stringPatterns: [(String, Color)] = [
            (#""[^"]*""#, tokens.warning),
            (#"\b(true|false|null)\b"#, tokens.warning),
            (#"\b(func|struct|enum|class|let|var|if|else|return|async|await|throw|try|case|switch|for|while|guard|import|public|private|actor)\b"#, tokens.accent),
            (#"\b\d+\b"#, tokens.success),
            (#"//.*$"#, tokens.tertiaryText),
            (#"^[-+].*$"#, tokens.accent),
        ]

        for (pattern, color) in stringPatterns {
            let options: NSRegularExpression.Options = pattern == #"//.*$"# || pattern == #"^[-+].*$"# ? [.anchorsMatchLines] : []
            guard let regex = try? NSRegularExpression(pattern: pattern, options: options) else { continue }
            for match in regex.matches(in: code, range: NSRange(location: 0, length: code.utf16.count)) {
                guard let range = Range(match.range, in: code),
                      let attributedRange = Range(range, in: attributed) else { continue }
                attributed[attributedRange].foregroundColor = color
                if language == "diff", code[range].hasPrefix("-") {
                    attributed[attributedRange].foregroundColor = tokens.failure
                }
            }
        }

        return attributed
    }
}
