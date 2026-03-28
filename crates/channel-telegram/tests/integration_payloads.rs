use mosaic_channel_telegram::{TelegramUpdate, normalize_update};

#[test]
fn normalizes_a_realistic_telegram_webhook_payload() {
    let update: TelegramUpdate = serde_json::from_value(serde_json::json!({
        "update_id": 73001,
        "message": {
            "message_id": 501,
            "text": "status please",
            "message_thread_id": 77,
            "chat": {
                "id": -100200300,
                "type": "supergroup",
                "title": "Ops Room",
                "username": "ops_room"
            },
            "from": {
                "id": 991,
                "username": "operator_991",
                "first_name": "Mosaic",
                "last_name": "Operator"
            }
        }
    }))
    .expect("payload should deserialize");

    let normalized = normalize_update(update).expect("payload should normalize");

    assert_eq!(
        normalized.session_hint.as_deref(),
        Some("telegram--100200300-77")
    );
    assert_eq!(normalized.thread_id.as_deref(), Some("77"));
    assert_eq!(normalized.conversation_id, "telegram:chat:-100200300");
    assert_eq!(
        normalized.reply_target,
        "telegram:chat:-100200300:thread:77:message:501"
    );
    assert_eq!(normalized.display_name.as_deref(), Some("Mosaic Operator"));
}
