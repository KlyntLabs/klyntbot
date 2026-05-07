use klynt_hooks::events::{pre_compact::PreCompactInput, pre_file_edit::PreFileEditInput};

#[test]
fn extension_event_inputs_serialize_round_trip() {
    let pre_compact = PreCompactInput {
        session_id: "test-session".to_string(),
        message_count: 100,
        current_tokens: 50_000,
        context_window: 200_000,
        base: Default::default(),
    };
    let s = serde_json::to_string(&pre_compact).unwrap();
    let back: PreCompactInput = serde_json::from_str(&s).unwrap();
    assert_eq!(back.message_count, 100);
}

#[test]
fn pre_file_edit_input_carries_diff_preview() {
    let input = PreFileEditInput {
        session_id: "s1".into(),
        tool: "edit".into(),
        path: "src/main.rs".into(),
        op: "edit".into(),
        diff_preview: "@@ -1 +1 @@\n-old\n+new\n".into(),
        bytes_before: 100,
        bytes_after: 103,
        base: Default::default(),
    };
    let s = serde_json::to_string(&input).unwrap();
    assert!(s.contains("diff_preview"));
}
