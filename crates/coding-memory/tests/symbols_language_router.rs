use coding_memory::symbols::Language;
use std::path::Path;

#[test]
fn router_recognizes_supported_extensions() {
    assert_eq!(
        Language::from_path(Path::new("a/b/c.rs")),
        Some(Language::Rust)
    );
    assert_eq!(
        Language::from_path(Path::new("x.ts")),
        Some(Language::TypeScript)
    );
    assert_eq!(
        Language::from_path(Path::new("x.tsx")),
        Some(Language::TypeScript)
    );
    assert_eq!(
        Language::from_path(Path::new("x.js")),
        Some(Language::JavaScript)
    );
    assert_eq!(
        Language::from_path(Path::new("x.mjs")),
        Some(Language::JavaScript)
    );
    assert_eq!(
        Language::from_path(Path::new("x.cjs")),
        Some(Language::JavaScript)
    );
    assert_eq!(
        Language::from_path(Path::new("x.py")),
        Some(Language::Python)
    );
    assert_eq!(Language::from_path(Path::new("x.go")), Some(Language::Go));
}

#[test]
fn router_returns_none_for_unsupported() {
    assert_eq!(Language::from_path(Path::new("README.md")), None);
    assert_eq!(Language::from_path(Path::new("package.json")), None);
    assert_eq!(Language::from_path(Path::new("noext")), None);
    assert_eq!(Language::from_path(Path::new("x.unknown")), None);
}
