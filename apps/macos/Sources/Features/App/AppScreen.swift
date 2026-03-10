import Domain
import Foundation

public enum AppScreen: Equatable {
    case loading
    case workspacePicker
    case onboarding(WorkspaceReference)
    case workbench
    case error(String)
}
