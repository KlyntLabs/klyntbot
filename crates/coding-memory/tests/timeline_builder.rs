use coding_memory::recall::timeline_builder::TimelineBuilder;
use jiff::Timestamp;

#[test]
fn entries_sorted_descending_by_when() {
    let ids = [uuid::Uuid::new_v4(), uuid::Uuid::new_v4()];
    let now = Timestamp::now();
    let inputs = vec![
        coding_memory::recall::timeline_builder::TimelineInput {
            id: ids[0],
            kind: "fix_attempt".into(),
            when: now,
            snippet: "older".into(),
            related_ids: vec![],
        },
        coding_memory::recall::timeline_builder::TimelineInput {
            id: ids[1],
            kind: "fix_attempt".into(),
            when: now.saturating_add(jiff::SignedDuration::from_secs(60)).unwrap(),
            snippet: "newer".into(),
            related_ids: vec![],
        },
    ];
    let out = TimelineBuilder::new().build(inputs);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].snippet, "newer");
    assert_eq!(out[1].snippet, "older");
}
