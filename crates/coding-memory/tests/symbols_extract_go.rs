use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use std::path::Path;

const SRC: &str = r#"
package x

func Add(a, b int) int { return a + b }

type Counter struct { n int }

func (c *Counter) Bump() { c.n += 1 }

const Max = 42

var Default = Counter{}
"#;

#[test]
fn extracts_go_top_level() {
    let extractor = TreeSitterExtractor::new();
    let symbols = extractor.extract(Path::new("a.go"), SRC, "h3");
    let names: std::collections::HashSet<_> = symbols.iter().map(|s| s.symbol.as_str()).collect();
    for expected in ["Add", "Counter", "Bump", "Max", "Default"] {
        assert!(names.contains(expected), "missing {expected} in {names:?}");
    }
}
