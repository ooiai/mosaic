use mosaic_provider::{LlmProvider, Message, MockProvider, Role, ToolDefinition};

#[tokio::test]
async fn mock_provider_completes_tool_request_through_public_trait() {
    let provider = MockProvider;
    let completion = provider
        .complete(
            &[Message {
                role: Role::User,
                content: "What time is it?".to_owned(),
                tool_call_id: None,
            }],
            Some(&[ToolDefinition {
                name: "time_now".to_owned(),
                description: "return utc time".to_owned(),
                input_schema: serde_json::json!({"type": "object"}),
            }]),
        )
        .await
        .expect("mock completion should succeed");

    assert_eq!(completion.attempts.len(), 1);
    assert_eq!(completion.response.tool_calls.len(), 1);
    assert_eq!(completion.response.tool_calls[0].name, "time_now");
}
