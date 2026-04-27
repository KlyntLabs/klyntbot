//! Phase-6 architectural invariants (proptest).

use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use proptest::prelude::*;
use std::path::Path;

proptest! {
    /// Parsing the same source twice must yield identical anchored-symbol vectors.
    #[test]
    fn parse_stability(
        body in prop::collection::vec("[a-z][a-z0-9_]{0,8}", 0..6)
    ) {
        let mut src = String::new();
        for name in &body {
            src.push_str(&format!("fn {name}() {{}}\n"));
        }
        let extractor = TreeSitterExtractor::new();
        let first = extractor.extract(Path::new("x.rs"), &src, "h");
        let second = extractor.extract(Path::new("x.rs"), &src, "h");
        prop_assert_eq!(first, second);
    }
}
