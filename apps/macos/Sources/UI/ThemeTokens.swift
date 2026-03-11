import SwiftUI

public struct ThemeTokens: Sendable {
    public let windowBackground: Color
    public let sidebarBackground: Color
    public let panelBackground: Color
    public let elevatedBackground: Color
    public let overlayBackground: Color
    public let codeBackground: Color
    public let logBackground: Color
    public let border: Color
    public let accent: Color
    public let accentMuted: Color
    public let selection: Color
    public let primaryText: Color
    public let secondaryText: Color
    public let tertiaryText: Color
    public let success: Color
    public let warning: Color
    public let failure: Color

    public static func current(for colorScheme: ColorScheme) -> ThemeTokens {
        switch colorScheme {
        case .dark:
            ThemeTokens(
                windowBackground: Color(red: 0.055, green: 0.059, blue: 0.067),
                sidebarBackground: Color(red: 0.070, green: 0.074, blue: 0.084),
                panelBackground: Color(red: 0.086, green: 0.091, blue: 0.103),
                elevatedBackground: Color(red: 0.112, green: 0.118, blue: 0.132),
                overlayBackground: Color.black.opacity(0.34),
                codeBackground: Color(red: 0.047, green: 0.055, blue: 0.072),
                logBackground: Color(red: 0.038, green: 0.042, blue: 0.050),
                border: Color.white.opacity(0.08),
                accent: Color(red: 0.32, green: 0.61, blue: 0.94),
                accentMuted: Color(red: 0.18, green: 0.30, blue: 0.44),
                selection: Color(red: 0.17, green: 0.31, blue: 0.48),
                primaryText: Color.white.opacity(0.94),
                secondaryText: Color.white.opacity(0.70),
                tertiaryText: Color.white.opacity(0.44),
                success: Color(red: 0.41, green: 0.77, blue: 0.60),
                warning: Color(red: 0.91, green: 0.69, blue: 0.31),
                failure: Color(red: 0.90, green: 0.40, blue: 0.42)
            )
        default:
            ThemeTokens(
                windowBackground: Color(red: 0.948, green: 0.954, blue: 0.966),
                sidebarBackground: Color(red: 0.968, green: 0.972, blue: 0.980),
                panelBackground: Color.white.opacity(0.84),
                elevatedBackground: Color.white.opacity(0.97),
                overlayBackground: Color.black.opacity(0.12),
                codeBackground: Color(red: 0.928, green: 0.944, blue: 0.976),
                logBackground: Color(red: 0.940, green: 0.946, blue: 0.956),
                border: Color.black.opacity(0.07),
                accent: Color(red: 0.16, green: 0.43, blue: 0.80),
                accentMuted: Color(red: 0.77, green: 0.86, blue: 0.95),
                selection: Color(red: 0.84, green: 0.90, blue: 0.97),
                primaryText: Color.black.opacity(0.88),
                secondaryText: Color.black.opacity(0.63),
                tertiaryText: Color.black.opacity(0.42),
                success: Color(red: 0.18, green: 0.58, blue: 0.39),
                warning: Color(red: 0.78, green: 0.54, blue: 0.15),
                failure: Color(red: 0.77, green: 0.27, blue: 0.29)
            )
        }
    }
}
