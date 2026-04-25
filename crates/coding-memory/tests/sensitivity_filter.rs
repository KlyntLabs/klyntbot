use coding_memory::reforge::sensitivity_filter::filter_for_externalization;
use coding_memory::reforge::types::SerializableSemanticFact;

fn fact(id: &str, sensitivity: &str) -> SerializableSemanticFact {
    SerializableSemanticFact {
        id: id.into(),
        subject: "s".into(),
        predicate: "p".into(),
        object: "o".into(),
        confidence: 0.8,
        memory_type: "fact".into(),
        sensitivity: sensitivity.into(),
        scope_repo_id: None,
        valid_from: jiff::Timestamp::now().to_string(),
    }
}

#[test]
fn drops_high_and_excluded_keeps_normal() {
    let input = vec![
        fact("a", "normal"),
        fact("b", "high"),
        fact("c", "excluded"),
        fact("d", "normal"),
    ];
    let kept = filter_for_externalization(&input);
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].id, "a");
    assert_eq!(kept[1].id, "d");
}
