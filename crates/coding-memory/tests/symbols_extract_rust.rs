use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use std::path::Path;

const SRC: &str = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

pub struct Counter { n: u32 }

impl Counter {
    pub fn new() -> Self { Self { n: 0 } }
    pub fn bump(&mut self) { self.n += 1; }
}

pub enum Mode { Fast, Slow }

pub trait Greet { fn hello(&self); }

pub const MAX: u32 = 42;
"#;

#[test]
fn extracts_rust_top_level_symbols() {
    let extractor = TreeSitterExtractor::new();
    let symbols = extractor.extract(Path::new("src/lib.rs"), SRC, "deadbeef");
    let names: std::collections::HashSet<_> = symbols.iter().map(|s| s.symbol.as_str()).collect();
    for expected in ["add", "Counter", "new", "bump", "Mode", "Greet", "MAX"] {
        assert!(names.contains(expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn rust_symbols_carry_git_hash_and_kind() {
    let extractor = TreeSitterExtractor::new();
    let symbols = extractor.extract(Path::new("src/lib.rs"), SRC, "deadbeef");
    let func = symbols
        .iter()
        .find(|s| s.symbol == "add")
        .expect("add not found");
    assert_eq!(func.git_hash, "deadbeef");
    assert_eq!(func.kind, "function");
    assert_eq!(func.file_path, Path::new("src/lib.rs"));
    let span = func.byte_span.expect("byte_span populated");
    assert!(span.0 < span.1);
}
