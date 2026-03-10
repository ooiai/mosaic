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
                windowBackground: Color(red: 0.035, green: 0.055, blue: 0.08),
                panelBackground: Color(red: 0.075, green: 0.10, blue: 0.14),
                elevatedBackground: Color(red: 0.11, green: 0.15, blue: 0.20),
                border: Color.white.opacity(0.1),
                primaryText: Color.white.opacity(0.95),
                secondaryText: Color.white.opacity(0.74),
                tertiaryText: Color.white.opacity(0.52),
                accent: Color(red: 0.20, green: 0.72, blue: 0.95),
                success: Color(red: 0.38, green: 0.82, blue: 0.62),
                warning: Color(red: 0.98, green: 0.75, blue: 0.31),
                failure: Color(red: 0.94, green: 0.40, blue: 0.42)
            )
        default:
            ThemeTokens(
                windowBackground: Color(red: 0.95, green: 0.97, blue: 0.985),
                panelBackground: Color.white.opacity(0.84),
                elevatedBackground: Color.white.opacity(0.97),
                border: Color.black.opacity(0.07),
                primaryText: Color.black.opacity(0.86),
                secondaryText: Color.black.opacity(0.63),
                tertiaryText: Color.black.opacity(0.42),
                accent: Color(red: 0.08, green: 0.52, blue: 0.86),
                success: Color(red: 0.17, green: 0.60, blue: 0.35),
                warning: Color(red: 0.82, green: 0.56, blue: 0.10),
                failure: Color(red: 0.77, green: 0.23, blue: 0.25)
            )
        }
    }
}
