use coding_memory::scope::AnchoredSymbol;
use coding_memory::symbols::SymbolCache;
use std::path::PathBuf;

fn anchor(name: &str) -> AnchoredSymbol {
    AnchoredSymbol {
        file_path: PathBuf::from("a.rs"),
        symbol: name.into(),
        kind: "function".into(),
        git_hash: "h".into(),
        byte_span: Some((0, 1)),
    }
}

#[test]
fn cache_hit_returns_value() {
    let cache = SymbolCache::with_capacity(4);
    let key_path = PathBuf::from("a.rs");
    let key_hash = [1u8; 32];
    cache.insert(key_path.clone(), key_hash, vec![anchor("foo")]);
    let got = cache.get(&key_path, &key_hash).expect("hit");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].symbol, "foo");
}

#[test]
fn cache_miss_on_different_hash() {
    let cache = SymbolCache::with_capacity(4);
    let key_path = PathBuf::from("a.rs");
    cache.insert(key_path.clone(), [1u8; 32], vec![anchor("foo")]);
    assert!(cache.get(&key_path, &[2u8; 32]).is_none());
}

#[test]
fn cache_evicts_when_over_capacity() {
    let cache = SymbolCache::with_capacity(2);
    cache.insert(PathBuf::from("a.rs"), [1u8; 32], vec![anchor("a")]);
    cache.insert(PathBuf::from("b.rs"), [2u8; 32], vec![anchor("b")]);
    cache.insert(PathBuf::from("c.rs"), [3u8; 32], vec![anchor("c")]);
    // Oldest (a.rs) should now be evicted.
    assert!(cache.get(&PathBuf::from("a.rs"), &[1u8; 32]).is_none());
    assert!(cache.get(&PathBuf::from("b.rs"), &[2u8; 32]).is_some());
    assert!(cache.get(&PathBuf::from("c.rs"), &[3u8; 32]).is_some());
}

#[test]
fn extractor_uses_cache_on_repeat_call() {
    use coding_memory::{SymbolExtractor, TreeSitterExtractor};
    use std::path::Path;

    let extractor = TreeSitterExtractor::new();
    let src = "fn a() {}\n";
    let first = extractor.extract(Path::new("x.rs"), src, "h");
    let second = extractor.extract(Path::new("x.rs"), src, "h");
    assert_eq!(first, second);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].symbol, "a");
}
