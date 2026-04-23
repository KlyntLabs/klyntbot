use ai_core::RecallDomain;

#[test]
fn notes_round_trips() {
    assert_eq!(RecallDomain::Notes.as_str(), "notes");
    assert_eq!(
        RecallDomain::from_str_or_general("notes"),
        RecallDomain::Notes
    );
}

#[test]
fn language_learning_round_trips() {
    assert_eq!(RecallDomain::LanguageLearning.as_str(), "language_learning");
    assert_eq!(
        RecallDomain::from_str_or_general("language_learning"),
        RecallDomain::LanguageLearning,
    );
}

#[test]
fn unknown_returns_general() {
    assert_eq!(
        RecallDomain::from_str_or_general("nonsense"),
        RecallDomain::General
    );
}
