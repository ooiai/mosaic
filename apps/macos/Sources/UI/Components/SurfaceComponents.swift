import Domain
import Features
import SwiftUI

struct PanelCard<Content: View>: View {
    let content: Content
    @Environment(\.colorScheme) private var colorScheme

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        content
            .padding(14)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
    }
}

struct SectionHeader: View {
    let title: String
    let trailing: String?
    @Environment(\.colorScheme) private var colorScheme

    init(_ title: String, trailing: String? = nil) {
        self.title = title
        self.trailing = trailing
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack {
            Text(title.uppercased())
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            Spacer()
            if let trailing {
                Text(trailing)
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }
        }
    }
}

struct StatusChip: View {
    let title: String
    let state: SessionState
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)
        let accent: Color = switch state {
        case .idle: tokens.tertiaryText
        case .waiting: tokens.warning
        case .running: tokens.accent
        case .failed: tokens.failure
        case .cancelled: tokens.warning
        case .done: tokens.success
        }

        HStack(spacing: 6) {
            Circle()
                .fill(accent)
                .frame(width: 6, height: 6)
            Text(title)
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
        }
        .foregroundStyle(accent)
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(accent.opacity(0.12), in: Capsule())
    }
}

struct MetricChip: View {
    let title: String
    let value: String
    let accent: Color
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Circle()
                .fill(accent)
                .frame(width: 5, height: 5)
            Text(title.uppercased())
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.tertiaryText)
            Text(value)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
                .lineLimit(1)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(tokens.elevatedBackground, in: Capsule())
    }
}

struct ToolbarActionButton: View {
    let systemImage: String
    let accent: Color?
    @Environment(\.colorScheme) private var colorScheme

    init(systemImage: String, accent: Color? = nil) {
        self.systemImage = systemImage
        self.accent = accent
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Image(systemName: systemImage)
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(accent ?? tokens.primaryText)
            .frame(width: 30, height: 28)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
    }
}

struct EmptyStateCard: View {
    let eyebrow: String
    let title: String
    let detail: String
    let actionTitle: String
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        VStack(alignment: .leading, spacing: 14) {
            Text(eyebrow.uppercased())
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(tokens.accent)
            Text(title)
                .font(.system(size: 28, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
            Text(detail)
                .font(.system(size: 14))
                .foregroundStyle(tokens.secondaryText)
            Button(actionTitle, action: action)
                .buttonStyle(.borderedProminent)
        }
        .padding(28)
        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}
