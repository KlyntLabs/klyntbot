use coding_memory::{SymbolExtractor, TreeSitterExtractor};
use std::path::Path;

const SRC: &str = r#"
export function add(a: number, b: number): number { return a + b; }

export class Counter {
    private n = 0;
    public bump(): void { this.n += 1; }
}

export interface Greeter { hello(): void; }

export type Mode = "fast" | "slow";

export const MAX = 42;
"#;

#[test]
fn extracts_typescript_top_level() {
    let extractor = TreeSitterExtractor::new();
    let symbols = extractor.extract(Path::new("a.ts"), SRC, "h1");
    let names: std::collections::HashSet<_> =
        symbols.iter().map(|s| s.symbol.as_str()).collect();
    for expected in ["add", "Counter", "bump", "Greeter", "Mode", "MAX"] {
        assert!(names.contains(expected), "missing {expected} in {names:?}");
    }
}
