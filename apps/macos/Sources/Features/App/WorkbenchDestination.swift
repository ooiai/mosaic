import Foundation

public enum WorkbenchDestination: String, CaseIterable, Identifiable, Sendable {
    case thread
    case automations
    case skills
    case settings

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .thread: "New thread"
        case .automations: "Automations"
        case .skills: "Skills"
        case .settings: "Settings"
        }
    }

    public var symbolName: String {
        switch self {
        case .thread: "square.and.pencil"
        case .automations: "clock.arrow.circlepath"
        case .skills: "square.grid.2x2"
        case .settings: "gearshape"
        }
    }
}

public enum SettingsSection: String, CaseIterable, Identifiable, Sendable {
    case general
    case configuration
    case personalization
    case markdown
    case debug

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .general: "General"
        case .configuration: "Configuration"
        case .personalization: "Personalization"
        case .markdown: "Markdown"
        case .debug: "Debug"
        }
    }

    public var symbolName: String {
        switch self {
        case .general: "gearshape"
        case .configuration: "slider.horizontal.3"
        case .personalization: "paintbrush"
        case .markdown: "doc.plaintext"
        case .debug: "ladybug"
        }
    }
}
