import SwiftUI

public struct ThemeTokens: Sendable {
    public let windowBackground: Color
    public let panelBackground: Color
    public let elevatedBackground: Color
    public let border: Color
    public let primaryText: Color
    public let secondaryText: Color
    public let tertiaryText: Color
    public let accent: Color
    public let success: Color
    public let warning: Color
    public let failure: Color

    public static func current(for colorScheme: ColorScheme) -> ThemeTokens {
        switch colorScheme {
        case .dark:
            ThemeTokens(
                windowBackground: Color(red: 0.08, green: 0.09, blue: 0.11),
                panelBackground: Color(red: 0.11, green: 0.12, blue: 0.14),
                elevatedBackground: Color(red: 0.15, green: 0.16, blue: 0.19),
                border: Color.white.opacity(0.08),
                primaryText: Color.white.opacity(0.94),
                secondaryText: Color.white.opacity(0.72),
                tertiaryText: Color.white.opacity(0.48),
                accent: Color(red: 0.46, green: 0.67, blue: 1.0),
                success: Color(red: 0.36, green: 0.81, blue: 0.56),
                warning: Color(red: 0.97, green: 0.71, blue: 0.25),
                failure: Color(red: 0.94, green: 0.37, blue: 0.39)
            )
        default:
            ThemeTokens(
                windowBackground: Color(red: 0.95, green: 0.96, blue: 0.98),
                panelBackground: Color.white.opacity(0.82),
                elevatedBackground: Color.white.opacity(0.96),
                border: Color.black.opacity(0.08),
                primaryText: Color.black.opacity(0.85),
                secondaryText: Color.black.opacity(0.65),
                tertiaryText: Color.black.opacity(0.4),
                accent: Color(red: 0.18, green: 0.44, blue: 0.92),
                success: Color(red: 0.19, green: 0.58, blue: 0.27),
                warning: Color(red: 0.79, green: 0.53, blue: 0.04),
                failure: Color(red: 0.75, green: 0.2, blue: 0.23)
            )
        }
    }
}
