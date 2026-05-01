use klynt_skill_loader::frontmatter::{KlyntFrontmatter, ReferenceLoadMode};

const FULL: &str = r#"---
name: refactor-helper
description: Helps refactor code.
allowed-tools: ["read", "edit", "grep"]
paths:
  - "**/*.rs"
  - "src/**/*.toml"
tags: ["refactor", "rust"]
sensitivity: "private"
references:
  - name: style-guide
    file: refs/style.md
    load: always
  - name: examples
    file: refs/examples.md
    load: on-demand
---
# Body
"#;

#[test]
fn parses_full_klynt_frontmatter() {
    let (fm, body) = KlyntFrontmatter::parse(FULL).unwrap();
    assert_eq!(fm.name, "refactor-helper");
    assert_eq!(
        fm.paths,
        vec!["**/*.rs".to_string(), "src/**/*.toml".to_string()]
    );
    assert_eq!(fm.tags, vec!["refactor".to_string(), "rust".to_string()]);
    assert_eq!(fm.sensitivity.as_deref(), Some("private"));
    assert_eq!(fm.references.len(), 2);
    assert!(matches!(fm.references[0].load, ReferenceLoadMode::Always));
    assert!(matches!(fm.references[1].load, ReferenceLoadMode::OnDemand));
    assert!(body.contains("# Body"));
}

#[test]
fn parses_minimal_frontmatter_with_defaults() {
    let raw = "---\nname: minimal\ndescription: Minimal.\n---\nBody\n";
    let (fm, _) = KlyntFrontmatter::parse(raw).unwrap();
    assert_eq!(fm.name, "minimal");
    assert!(fm.paths.is_empty());
    assert!(fm.tags.is_empty());
    assert!(fm.sensitivity.is_none());
    assert!(fm.references.is_empty());
}

#[test]
fn missing_frontmatter_fence_errors() {
    let raw = "name: bad\nNo fence here.\n";
    assert!(KlyntFrontmatter::parse(raw).is_err());
}

#[test]
fn missing_required_name_errors() {
    let raw = "---\ndescription: Missing name\n---\nBody\n";
    assert!(KlyntFrontmatter::parse(raw).is_err());
}

#[test]
fn unknown_load_mode_defaults_to_on_demand() {
    let raw = r#"---
name: test
description: Test
references:
  - name: foo
    file: foo.md
    load: bogus
---
Body
"#;
    let (fm, _) = KlyntFrontmatter::parse(raw).unwrap();
    assert!(matches!(fm.references[0].load, ReferenceLoadMode::OnDemand));
}
