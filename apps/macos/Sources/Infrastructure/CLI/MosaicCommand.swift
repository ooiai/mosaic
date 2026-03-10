import Domain
import Foundation

public enum MosaicCommand: Sendable {
    case setup(baseURL: String, model: String, apiKeyEnv: String)
    case status
    case health
    case configureShow
    case configureSet(key: RuntimeConfigKey, value: String)
    case modelsStatus
    case modelsList
    case modelsSet(model: String)
    case ask(prompt: String)
    case chat(prompt: String, sessionID: String?)
    case sessionList
    case sessionShow(id: String)
    case sessionClear(id: String)

    public var arguments: [String] {
        var base = ["--project-state", "--json"]
        switch self {
        case let .setup(baseURL, model, apiKeyEnv):
            base.append(contentsOf: [
                "setup",
                "--base-url", baseURL,
                "--api-key-env", apiKeyEnv,
                "--model", model,
            ])
        case .status:
            base.append("status")
        case .health:
            base.append("health")
        case .configureShow:
            base.append(contentsOf: ["configure", "--show"])
        case let .configureSet(key, value):
            base.append(contentsOf: ["configure", "set", key.rawValue, value])
        case .modelsStatus:
            base.append(contentsOf: ["models", "status"])
        case .modelsList:
            base.append(contentsOf: ["models", "list"])
        case let .modelsSet(model):
            base.append(contentsOf: ["models", "set", model])
        case let .ask(prompt):
            base.append(contentsOf: ["ask", prompt])
        case let .chat(prompt, sessionID):
            base.append(contentsOf: ["chat", "--prompt", prompt])
            if let sessionID, !sessionID.isEmpty {
                base.append(contentsOf: ["--session", sessionID])
            }
        case .sessionList:
            base.append(contentsOf: ["session", "list"])
        case let .sessionShow(id):
            base.append(contentsOf: ["session", "show", id])
        case let .sessionClear(id):
            base.append(contentsOf: ["session", "clear", id])
        }
        return base
    }
}
