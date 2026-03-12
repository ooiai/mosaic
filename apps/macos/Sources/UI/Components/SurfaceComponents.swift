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
            .padding(16)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
            .shadow(color: colorScheme == .light ? Color.black.opacity(0.03) : .clear, radius: 18, y: 8)
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
            Text(value)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(tokens.primaryText)
                .lineLimit(1)
            if !title.isEmpty {
                Text(title.uppercased())
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(tokens.tertiaryText)
            }
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
            .frame(width: 32, height: 30)
            .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 11, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 11, style: .continuous)
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

struct SidebarNavButton: View {
    let title: String
    let systemImage: String
    let isSelected: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: systemImage)
                    .font(.system(size: 13, weight: .medium))
                    .frame(width: 16)
                Text(title)
                    .font(.system(size: 14, weight: .medium))
                Spacer()
            }
            .foregroundStyle(isSelected ? tokens.primaryText : tokens.secondaryText)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(
                (isSelected ? tokens.selection : Color.clear),
                in: RoundedRectangle(cornerRadius: 12, style: .continuous)
            )
        }
        .buttonStyle(.plain)
    }
}

struct SuggestionPromptCard: View {
    let title: String
    let symbolName: String
    let tint: Color
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            VStack(alignment: .leading, spacing: 14) {
                Image(systemName: symbolName)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(tint)
                    .frame(width: 24, height: 24)
                    .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 8, style: .continuous))

                Text(title)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(tokens.primaryText)
                    .multilineTextAlignment(.leading)
                    .lineLimit(3)

                Spacer(minLength: 0)
            }
            .frame(width: 230, height: 118, alignment: .topLeading)
            .padding(16)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }
}

struct FooterPill: View {
    let title: String
    let systemImage: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 6) {
            Image(systemName: systemImage)
            Text(title)
        }
        .font(.system(size: 11, weight: .medium))
        .foregroundStyle(tokens.secondaryText)
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
    }
}
