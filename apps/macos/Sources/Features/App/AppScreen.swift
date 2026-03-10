import Domain
import Foundation

public enum AppScreen: Equatable {
    case loading
    case setupHub
    case workbench
    case error(String)
}
