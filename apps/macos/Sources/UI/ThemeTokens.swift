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
                windowBackground: Color(red: 0.043, green: 0.047, blue: 0.056),
                panelBackground: Color(red: 0.074, green: 0.079, blue: 0.092),
                elevatedBackground: Color(red: 0.106, green: 0.112, blue: 0.129),
                border: Color.white.opacity(0.085),
                primaryText: Color.white.opacity(0.94),
                secondaryText: Color.white.opacity(0.68),
                tertiaryText: Color.white.opacity(0.46),
                accent: Color(red: 0.33, green: 0.64, blue: 0.96),
                success: Color(red: 0.39, green: 0.78, blue: 0.60),
                warning: Color(red: 0.93, green: 0.70, blue: 0.33),
                failure: Color(red: 0.91, green: 0.42, blue: 0.45)
            )
        default:
            ThemeTokens(
                windowBackground: Color(red: 0.948, green: 0.955, blue: 0.968),
                panelBackground: Color.white.opacity(0.88),
                elevatedBackground: Color.white.opacity(0.97),
                border: Color.black.opacity(0.065),
                primaryText: Color.black.opacity(0.87),
                secondaryText: Color.black.opacity(0.62),
                tertiaryText: Color.black.opacity(0.42),
                accent: Color(red: 0.15, green: 0.46, blue: 0.84),
                success: Color(red: 0.18, green: 0.58, blue: 0.39),
                warning: Color(red: 0.78, green: 0.53, blue: 0.14),
                failure: Color(red: 0.75, green: 0.26, blue: 0.28)
            )
        }
    }
}
