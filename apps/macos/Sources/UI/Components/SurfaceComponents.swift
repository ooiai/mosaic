import Domain
import Features
import SwiftUI

enum WorkbenchChromeMetrics {
    static let threadContentWidth: CGFloat = 760
    static let composerWidth: CGFloat = 760
    static let assistantMessageWidth: CGFloat = 720
    static let userMessageWidth: CGFloat = 676
    static let systemMessageWidth: CGFloat = 724
}

struct PanelCard<Content: View>: View {
    let content: Content
    @Environment(\.colorScheme) private var colorScheme

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        content
            .padding(18)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
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

struct PaneSectionTitle: View {
    let title: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Text(title)
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(tokens.primaryText)
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
    let isEnabled: Bool
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    init(systemImage: String, accent: Color? = nil, isEnabled: Bool = true) {
        self.systemImage = systemImage
        self.accent = accent
        self.isEnabled = isEnabled
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Image(systemName: systemImage)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(isEnabled ? (accent ?? tokens.primaryText) : tokens.tertiaryText)
            .frame(width: 26, height: 26)
            .background(
                (isHovered && isEnabled ? tokens.panelBackground.opacity(0.96) : tokens.elevatedBackground),
                in: RoundedRectangle(cornerRadius: 9, style: .continuous)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
            .opacity(isEnabled ? 1 : 0.62)
            .onHover { isHovered = $0 }
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

struct WorkbenchPageHeader<Trailing: View>: View {
    let title: String
    let subtitle: String
    let badge: String?
    let trailing: Trailing
    @Environment(\.colorScheme) private var colorScheme

    init(title: String, subtitle: String, badge: String? = nil, @ViewBuilder trailing: () -> Trailing) {
        self.title = title
        self.subtitle = subtitle
        self.badge = badge
        self.trailing = trailing()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Text(title)
                        .font(.system(size: 24, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)

                    if let badge {
                        Text(badge)
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(tokens.secondaryText)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 2)
                            .background(tokens.elevatedBackground, in: Capsule())
                    }
                }

                Text(subtitle)
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
            }

            Spacer(minLength: 24)

            trailing
        }
    }
}

struct PageHeaderSecondaryButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: systemImage)
                Text(title)
            }
            .font(.system(size: 11.5, weight: .medium))
            .foregroundStyle(tokens.secondaryText)
            .padding(.horizontal, 9)
            .padding(.vertical, 6)
            .background((isHovered ? tokens.panelBackground : Color.clear), in: Capsule())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

struct PageHeaderPrimaryButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: systemImage)
                Text(title)
            }
            .font(.system(size: 11.5, weight: .semibold))
            .foregroundStyle(tokens.windowBackground)
            .padding(.horizontal, 13)
            .padding(.vertical, 7)
            .background((isHovered ? tokens.primaryText.opacity(0.88) : tokens.primaryText), in: Capsule())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

struct PageSearchField: View {
    let placeholder: String
    @Binding var text: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(tokens.tertiaryText)

            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(tokens.primaryText)
        }
        .padding(.horizontal, 11)
        .padding(.vertical, 8)
        .frame(width: 180)
        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 11, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}

struct CatalogSurfaceCard<Content: View>: View {
    let minHeight: CGFloat?
    let content: Content
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    init(minHeight: CGFloat? = nil, @ViewBuilder content: () -> Content) {
        self.minHeight = minHeight
        self.content = content()
    }

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        content
            .frame(maxWidth: .infinity, minHeight: minHeight, alignment: .topLeading)
            .padding(14)
            .background((isHovered ? tokens.elevatedBackground : tokens.panelBackground), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
            .onHover { isHovered = $0 }
    }
}

struct CatalogIconBadge: View {
    let systemImage: String
    let tint: Color

    var body: some View {
        Image(systemName: systemImage)
            .font(.system(size: 15, weight: .semibold))
            .foregroundStyle(tint)
            .frame(width: 38, height: 38)
            .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

struct CatalogPlusButton: View {
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Image(systemName: "plus")
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(tokens.secondaryText)
            .frame(width: 24, height: 24)
            .background((isHovered ? tokens.panelBackground : tokens.elevatedBackground), in: Circle())
            .onHover { isHovered = $0 }
    }
}

struct SidebarNavButton: View {
    let title: String
    let systemImage: String
    let isSelected: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: systemImage)
                    .font(.system(size: 12.5, weight: .medium))
                    .frame(width: 16)
                Text(title)
                    .font(.system(size: 13.5, weight: .medium))
                Spacer()
            }
            .foregroundStyle(isSelected ? tokens.primaryText : tokens.secondaryText)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                (isSelected ? tokens.selection : (isHovered ? tokens.panelBackground.opacity(0.72) : Color.clear)),
                in: RoundedRectangle(cornerRadius: 12, style: .continuous)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

struct SuggestionPromptCard: View {
    let title: String
    let symbolName: String
    let tint: Color
    let action: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false
    @State private var isPressed = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: action) {
            VStack(alignment: .leading, spacing: 10) {
                Image(systemName: symbolName)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(tint)
                    .frame(width: 20, height: 20)
                    .background(tint.opacity(0.11), in: RoundedRectangle(cornerRadius: 7, style: .continuous))

                Text(title)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(tokens.primaryText)
                    .multilineTextAlignment(.leading)
                    .lineLimit(3)

                Spacer(minLength: 0)
            }
            .frame(width: 204, height: 96, alignment: .topLeading)
            .padding(13)
            .background(background(tokens: tokens), in: RoundedRectangle(cornerRadius: 15, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 15, style: .continuous)
                    .stroke(isHovered ? tokens.accent.opacity(0.16) : tokens.border, lineWidth: 1)
            )
            .scaleEffect(isPressed ? 0.992 : 1)
            .shadow(color: isHovered && colorScheme == .light ? Color.black.opacity(0.035) : .clear, radius: 12, y: 5)
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in isPressed = true }
                .onEnded { _ in isPressed = false }
        )
    }

    private func background(tokens: ThemeTokens) -> Color {
        if isPressed {
            return tokens.elevatedBackground
        }
        return isHovered ? tokens.elevatedBackground : tokens.panelBackground
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
        .font(.system(size: 10, weight: .medium))
        .foregroundStyle(tokens.secondaryText)
        .padding(.horizontal, 7)
        .padding(.vertical, 4)
        .background(tokens.elevatedBackground.opacity(0.72), in: Capsule())
    }
}
