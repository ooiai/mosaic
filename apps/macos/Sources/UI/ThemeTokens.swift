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
                windowBackground: Color(red: 0.078, green: 0.082, blue: 0.091),
                sidebarBackground: Color(red: 0.094, green: 0.106, blue: 0.124),
                panelBackground: Color(red: 0.112, green: 0.118, blue: 0.132),
                elevatedBackground: Color(red: 0.132, green: 0.140, blue: 0.157),
                overlayBackground: Color.black.opacity(0.34),
                codeBackground: Color(red: 0.061, green: 0.071, blue: 0.093),
                logBackground: Color(red: 0.054, green: 0.058, blue: 0.068),
                border: Color.white.opacity(0.085),
                accent: Color(red: 0.38, green: 0.67, blue: 0.96),
                accentMuted: Color(red: 0.18, green: 0.30, blue: 0.46),
                selection: Color(red: 0.21, green: 0.31, blue: 0.43),
                primaryText: Color.white.opacity(0.94),
                secondaryText: Color.white.opacity(0.72),
                tertiaryText: Color.white.opacity(0.46),
                success: Color(red: 0.41, green: 0.77, blue: 0.60),
                warning: Color(red: 0.91, green: 0.69, blue: 0.31),
                failure: Color(red: 0.90, green: 0.40, blue: 0.42)
            )
        default:
            ThemeTokens(
                windowBackground: Color(red: 0.978, green: 0.979, blue: 0.982),
                sidebarBackground: Color(red: 0.905, green: 0.929, blue: 0.955),
                panelBackground: Color.white.opacity(0.94),
                elevatedBackground: Color(red: 0.965, green: 0.969, blue: 0.976),
                overlayBackground: Color.black.opacity(0.12),
                codeBackground: Color(red: 0.939, green: 0.947, blue: 0.969),
                logBackground: Color(red: 0.948, green: 0.952, blue: 0.959),
                border: Color.black.opacity(0.075),
                accent: Color(red: 0.14, green: 0.39, blue: 0.78),
                accentMuted: Color(red: 0.82, green: 0.88, blue: 0.95),
                selection: Color(red: 0.85, green: 0.90, blue: 0.95),
                primaryText: Color.black.opacity(0.90),
                secondaryText: Color.black.opacity(0.66),
                tertiaryText: Color.black.opacity(0.44),
                success: Color(red: 0.18, green: 0.58, blue: 0.39),
                warning: Color(red: 0.78, green: 0.54, blue: 0.15),
                failure: Color(red: 0.77, green: 0.27, blue: 0.29)
            )
        }
    }
}
