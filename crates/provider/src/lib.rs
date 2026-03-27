mod capabilities;
mod errors;
mod profile;
mod types;
pub mod vendors;

#[cfg(test)]
mod tests;

pub use capabilities::{
    ModelCapabilities, ScheduledProfile, SchedulingIntent, SchedulingRequest,
    tool_definition_from_metadata, tool_is_visible_to_model, validate_step_tools_support,
};
pub use errors::{ProviderError, ProviderErrorKind, public_error_message};
pub use profile::{ProviderProfile, ProviderProfileRegistry};
pub use types::{
    CompletionResponse, LlmProvider, Message, ProviderAttempt, ProviderCompletion,
    ProviderTransportMetadata, Role, ToolCall, ToolDefinition,
};
pub use vendors::{
    AnthropicProvider, AzureProvider, MockProvider, OllamaProvider, OpenAiCompatibleProvider,
    OpenAiProvider, build_provider_from_profile,
};
