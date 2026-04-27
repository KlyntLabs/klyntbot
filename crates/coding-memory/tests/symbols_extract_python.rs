use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use std::path::Path;

const SRC: &str = r#"
def add(a, b):
    return a + b

class Counter:
    def __init__(self): self.n = 0
    def bump(self): self.n += 1

MAX = 42
"#;

#[test]
fn extracts_python_top_level() {
    let extractor = TreeSitterExtractor::new();
    let symbols = extractor.extract(Path::new("a.py"), SRC, "h2");
    let names: std::collections::HashSet<_> =
        symbols.iter().map(|s| s.symbol.as_str()).collect();
    for expected in ["add", "Counter", "__init__", "bump", "MAX"] {
        assert!(names.contains(expected), "missing {expected} in {names:?}");
    }
}
