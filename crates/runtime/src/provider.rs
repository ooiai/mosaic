use super::*;

impl AgentRuntime {
    pub(crate) fn provider_for_profile(
        &self,
        profile: &ProviderProfile,
    ) -> Result<Arc<dyn LlmProvider>> {
        match self.ctx.provider_override.clone() {
            Some(provider) => Ok(provider),
            None => self.ctx.profiles.build_provider(Some(&profile.name)),
        }
    }

    pub(crate) fn model_selection_trace(
        scope: impl Into<String>,
        requested_profile: Option<String>,
        profile: &ProviderProfile,
        reason: impl Into<String>,
    ) -> ModelSelectionTrace {
        ModelSelectionTrace {
            scope: scope.into(),
            requested_profile,
            selected_profile: profile.name.clone(),
            selected_model: profile.model.clone(),
            reason: reason.into(),
            context_window_chars: profile.capabilities.context_window_chars,
            budget_tier: profile.capabilities.budget_tier.clone(),
        }
    }

    pub(crate) fn effective_profile_trace(
        profile: &ProviderProfile,
        metadata: &ProviderTransportMetadata,
    ) -> EffectiveProfileTrace {
        EffectiveProfileTrace {
            profile: profile.name.clone(),
            provider_type: profile.provider_type.clone(),
            model: profile.model.clone(),
            base_url: metadata
                .base_url
                .clone()
                .or_else(|| profile.base_url.clone()),
            api_key_env: profile.api_key_env.clone(),
            api_key_present: profile.api_key_present(),
            timeout_ms: metadata.timeout_ms,
            max_retries: metadata.max_retries,
            supports_tools: profile.capabilities.supports_tools,
            supports_tool_call_shadow_messages: metadata.supports_tool_call_shadow_messages,
        }
    }

    pub(crate) fn provider_metadata_from_profile(
        profile: &ProviderProfile,
    ) -> ProviderTransportMetadata {
        let (timeout_ms, max_retries, supports_tool_call_shadow_messages) =
            match profile.provider_type.as_str() {
                "anthropic" => (60_000, 2, true),
                "ollama" => (90_000, 1, false),
                "mock" => (0, 0, false),
                _ => (45_000, 2, false),
            };

        let base_url = profile
            .base_url
            .clone()
            .or_else(|| match profile.provider_type.as_str() {
                "openai" | "openai-compatible" => Some("https://api.openai.com/v1".to_owned()),
                "anthropic" => Some("https://api.anthropic.com/v1".to_owned()),
                "ollama" => Some("http://127.0.0.1:11434".to_owned()),
                _ => None,
            });

        ProviderTransportMetadata {
            provider_type: profile.provider_type.clone(),
            base_url,
            timeout_ms,
            max_retries,
            supports_tool_call_shadow_messages,
        }
    }

    pub(crate) fn provider_attempt_trace(attempt: &ProviderAttempt) -> ProviderAttemptTrace {
        ProviderAttemptTrace {
            attempt: attempt.attempt,
            max_attempts: attempt.max_attempts,
            status: attempt.status.clone(),
            error_kind: attempt.error_kind.clone(),
            status_code: attempt.status_code,
            retryable: attempt.retryable,
            message: attempt.message.clone(),
        }
    }

    pub(crate) fn provider_failure_trace(error: &ProviderError) -> ProviderFailureTrace {
        ProviderFailureTrace {
            kind: error.kind_label().to_owned(),
            status_code: error.status_code,
            retryable: error.retryable,
            message: error.public_message().to_owned(),
        }
    }

    pub(crate) fn trace_provider_attempts(
        &self,
        trace: &mut RunTrace,
        attempts: &[ProviderAttempt],
    ) {
        for attempt in attempts {
            trace.add_provider_attempt(Self::provider_attempt_trace(attempt));
        }
    }

    pub(crate) fn trace_provider_error(
        &self,
        profile: &ProviderProfile,
        trace: &mut RunTrace,
        error: &ProviderError,
    ) {
        self.trace_provider_attempts(trace, &error.attempts);
        trace.bind_provider_failure(Self::provider_failure_trace(error));
        warn!(
            run_id = %trace.run_id,
            provider_type = %profile.provider_type,
            profile = %profile.name,
            model = %profile.model,
            error_kind = %error.kind_label(),
            status_code = ?error.status_code,
            retryable = error.retryable,
            "provider call failed"
        );
    }

    pub(crate) fn emit_provider_retry_events(
        &self,
        profile: &ProviderProfile,
        attempts: &[ProviderAttempt],
    ) {
        for attempt in attempts.iter().filter(|attempt| attempt.status == "retry") {
            self.emit(RunEvent::ProviderRetry {
                provider_type: profile.provider_type.clone(),
                profile: profile.name.clone(),
                model: profile.model.clone(),
                attempt: attempt.attempt,
                max_attempts: attempt.max_attempts,
                kind: attempt
                    .error_kind
                    .clone()
                    .unwrap_or_else(|| "unknown".to_owned()),
                status_code: attempt.status_code,
                retryable: attempt.retryable,
                error: attempt
                    .message
                    .clone()
                    .unwrap_or_else(|| "provider retry".to_owned()),
            });
        }
    }
}
