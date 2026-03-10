import Domain
import Foundation

public struct CLIEnvelope<Payload: Decodable>: Decodable {
    public let ok: Bool
    public let payload: Payload

    enum CodingKeys: String, CodingKey {
        case ok
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        ok = try container.decode(Bool.self, forKey: .ok)
        payload = try Payload(from: decoder)
    }
}

public struct CLIErrorPayload: Decodable, Equatable {
    public let code: String
    public let message: String
    public let exitCode: Int

    enum CodingKeys: String, CodingKey {
        case code
        case message
        case exitCode = "exit_code"
    }
}

public struct CLIErrorEnvelope: Decodable, Equatable {
    public let ok: Bool
    public let error: CLIErrorPayload
}

public struct CLISetupPayload: Decodable {
    let profile: String
    let mode: String
    let configPath: String

    enum CodingKeys: String, CodingKey {
        case profile
        case mode
        case configPath = "config_path"
    }

    public func toDomain() -> SetupSummary {
        SetupSummary(profile: profile, mode: mode, configPath: configPath)
    }
}

public struct CLIStatusPayload: Decodable {
    struct Provider: Decodable {
        let apiKeyEnv: String?
        let baseURL: String?
        let kind: String?
        let model: String?

        enum CodingKeys: String, CodingKey {
            case apiKeyEnv = "api_key_env"
            case baseURL = "base_url"
            case kind
            case model
        }
    }

    let configured: Bool
    let configPath: String
    let profile: String?
    let latestSession: String?
    let defaultAgentID: String?
    let agentsCount: Int
    let stateMode: String?
    let provider: Provider?

    enum CodingKeys: String, CodingKey {
        case configured
        case configPath = "config_path"
        case profile
        case latestSession = "latest_session"
        case defaultAgentID = "default_agent_id"
        case agentsCount = "agents_count"
        case stateMode = "state_mode"
        case provider
    }

    public func toDomain() -> RuntimeStatusSummary {
        RuntimeStatusSummary(
            configured: configured,
            configPath: configPath,
            profile: profile,
            latestSession: latestSession,
            defaultAgentID: defaultAgentID,
            agentsCount: agentsCount,
            stateMode: stateMode,
            provider: provider.map {
                .init(
                    apiKeyEnv: $0.apiKeyEnv,
                    baseURL: $0.baseURL,
                    kind: $0.kind,
                    model: $0.model
                )
            }
        )
    }
}

public struct CLIHealthPayload: Decodable {
    struct Check: Decodable {
        let name: String
        let detail: String
        let status: String
    }

    let type: String
    let checks: [Check]

    public func toDomain() -> HealthSummary {
        HealthSummary(
            type: type,
            checks: checks.map { .init(name: $0.name, detail: $0.detail, status: $0.status) }
        )
    }
}

public struct CLIModelsStatusPayload: Decodable {
    let profile: String?
    let currentModel: String
    let effectiveModel: String
    let baseURL: String
    let apiKeyEnv: String
    let aliases: [String: String]
    let fallbacks: [String]

    enum CodingKeys: String, CodingKey {
        case profile
        case currentModel = "current_model"
        case effectiveModel = "effective_model"
        case baseURL = "base_url"
        case apiKeyEnv = "api_key_env"
        case aliases
        case fallbacks
    }

    public func toDomain() -> ModelsStatusSummary {
        ModelsStatusSummary(
            profile: profile,
            currentModel: currentModel,
            effectiveModel: effectiveModel,
            baseURL: baseURL,
            apiKeyEnv: apiKeyEnv,
            aliases: aliases,
            fallbacks: fallbacks
        )
    }
}

public struct CLIModelsListPayload: Decodable {
    struct Model: Decodable {
        let id: String
        let ownedBy: String?

        enum CodingKeys: String, CodingKey {
            case id
            case ownedBy = "owned_by"
        }
    }

    let models: [Model]

    public func toDomain() -> [ModelSummary] {
        models.map { .init(id: $0.id, ownedBy: $0.ownedBy) }
    }
}

public struct CLIConfigurePayload: Decodable {
    struct Config: Decodable {
        struct Profile: Decodable {
            struct Agent: Decodable {
                let maxTurns: Int
                let temperature: Double

                enum CodingKeys: String, CodingKey {
                    case maxTurns = "max_turns"
                    case temperature
                }
            }

            struct Provider: Decodable {
                let apiKeyEnv: String
                let baseURL: String
                let kind: String
                let model: String

                enum CodingKeys: String, CodingKey {
                    case apiKeyEnv = "api_key_env"
                    case baseURL = "base_url"
                    case kind
                    case model
                }
            }

            let agent: Agent
            let provider: Provider
        }

        struct State: Decodable {
            let mode: String
            let projectDir: String

            enum CodingKeys: String, CodingKey {
                case mode
                case projectDir = "project_dir"
            }
        }

        let profile: Profile
        let profileName: String
        let state: State

        enum CodingKeys: String, CodingKey {
            case profile
            case profileName = "profile_name"
            case state
        }
    }

    let config: Config

    public func toDomain() -> ConfigurationSummary {
        ConfigurationSummary(
            profileName: config.profileName,
            agent: .init(
                maxTurns: config.profile.agent.maxTurns,
                temperature: config.profile.agent.temperature
            ),
            provider: .init(
                apiKeyEnv: config.profile.provider.apiKeyEnv,
                baseURL: config.profile.provider.baseURL,
                kind: config.profile.provider.kind,
                model: config.profile.provider.model
            ),
            stateMode: config.state.mode,
            projectDir: config.state.projectDir
        )
    }
}

public struct CLIPromptPayload: Decodable {
    let sessionID: String
    let response: String
    let profile: String
    let agentID: String?
    let turns: Int

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case response
        case profile
        case agentID = "agent_id"
        case turns
    }

    public func toDomain() -> PromptResponse {
        PromptResponse(
            sessionID: sessionID,
            response: response,
            profile: profile,
            agentID: agentID,
            turns: turns
        )
    }
}

public struct CLISessionsPayload: Decodable {
    struct Session: Decodable {
        let sessionID: String
        let eventCount: Int
        let lastUpdated: String

        enum CodingKeys: String, CodingKey {
            case sessionID = "session_id"
            case eventCount = "event_count"
            case lastUpdated = "last_updated"
        }
    }

    let sessions: [Session]

    public func toDomain() -> [SessionSummaryData] {
        sessions.map { .init(id: $0.sessionID, eventCount: $0.eventCount, lastUpdated: $0.lastUpdated) }
    }
}

public struct CLISessionTranscriptPayload: Decodable {
    struct Event: Decodable {
        struct Payload: Decodable {
            let text: String?
        }

        let id: String
        let sessionID: String
        let type: String
        let timestamp: String
        let payload: Payload

        enum CodingKeys: String, CodingKey {
            case id
            case sessionID = "session_id"
            case type
            case timestamp = "ts"
            case payload
        }
    }

    let sessionID: String
    let events: [Event]

    enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case events
    }

    public func toDomain() -> SessionTranscript {
        SessionTranscript(
            sessionID: sessionID,
            events: events.map {
                .init(
                    id: $0.id,
                    sessionID: $0.sessionID,
                    type: SessionEvent.Kind(rawValue: $0.type) ?? .system,
                    timestamp: $0.timestamp,
                    text: $0.payload.text ?? ""
                )
            }
        )
    }
}
