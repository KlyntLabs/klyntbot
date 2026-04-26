//! Invariant — bytes outside managed markers are preserved across writes.

use coding_memory::reforge::managed_block::ManagedBlock;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn user_content_outside_markers_is_preserved(
        prefix in "[a-zA-Z0-9\\n]{0,200}",
        suffix in "[a-zA-Z0-9\\n]{0,200}",
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        let original = format!("{}\n<!-- klyntbot:managed:start -->\nold\n<!-- klyntbot:managed:end -->\n{}", prefix, suffix);
        std::fs::write(&path, &original).unwrap();

        let block = ManagedBlock::read(&path).unwrap_or_default();
        block.write_with_new_inside(&path, "new body", "c1").unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        prop_assert!(result.contains(&prefix), "prefix lost");
        prop_assert!(result.contains(&suffix), "suffix lost");
        prop_assert!(result.contains("new body"), "new body missing");
        prop_assert!(!result.contains("old"), "old body still present");
    }
}
