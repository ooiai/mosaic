use mosaic_runtime::events::RunEvent;
use mosaic_tui::{build_tui_event_buffer, build_tui_event_sink};

#[test]
fn tui_event_buffer_roundtrips_runtime_events_via_public_sink() {
    let buffer = build_tui_event_buffer();
    let sink = build_tui_event_sink(buffer.clone());

    sink.emit(RunEvent::RunStarted {
        run_id: "run-1".to_owned(),
        input: "hello".to_owned(),
    });
    sink.emit(RunEvent::RunFinished {
        run_id: "run-1".to_owned(),
        output_preview: "world".to_owned(),
    });

    let events = buffer.drain();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], RunEvent::RunStarted { .. }));
    assert!(matches!(events[1], RunEvent::RunFinished { .. }));
}
