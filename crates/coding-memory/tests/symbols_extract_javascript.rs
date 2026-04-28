use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use std::path::Path;

const SRC: &str = r#"
function add(a, b) { return a + b; }

class Counter {
    constructor() { this.n = 0; }
    bump() { this.n += 1; }
}

const MAX = 42;

const greet = () => "hello";
"#;

#[test]
fn extracts_javascript_top_level() {
    let extractor = TreeSitterExtractor::new();
    let symbols = extractor.extract(Path::new("a.js"), SRC, "h1");
    let names: std::collections::HashSet<_> = symbols.iter().map(|s| s.symbol.as_str()).collect();
    for expected in ["add", "Counter", "constructor", "bump", "MAX", "greet"] {
        assert!(names.contains(expected), "missing {expected} in {names:?}");
    }
}
