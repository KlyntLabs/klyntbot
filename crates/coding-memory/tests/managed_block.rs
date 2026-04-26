use coding_memory::reforge::managed_block::{ManagedBlock, ManagedBlockError};
use std::fs;
use tempfile::tempdir;

const SAMPLE: &str = "\
# Notes

User content above.

<!-- klyntbot:managed:start | generated: 2026-04-22T03:00Z | cycle: 1 -->
- managed line 1
- managed line 2
<!-- klyntbot:managed:end -->

User content below.
";

#[test]
fn parses_user_outside_managed_inside() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("CLAUDE.md");
    fs::write(&path, SAMPLE).unwrap();

    let block = ManagedBlock::read(&path).expect("parse");
    assert!(block.before.contains("User content above"));
    assert!(block.inside.contains("managed line 1"));
    assert!(block.after.contains("User content below"));
}

#[test]
fn write_rebuilds_file_with_new_managed_body_only() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("CLAUDE.md");
    fs::write(&path, SAMPLE).unwrap();

    let block = ManagedBlock::read(&path).expect("parse");
    let new_body = "- replaced line\n";
    block
        .write_with_new_inside(&path, new_body, "cycle-2")
        .expect("write");

    let written = fs::read_to_string(&path).unwrap();
    assert!(written.contains("User content above"));
    assert!(written.contains("- replaced line"));
    assert!(!written.contains("managed line 1"));
    assert!(written.contains("User content below"));
}

#[test]
fn write_inserts_managed_block_when_absent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("FRESH.md");
    let block = ManagedBlock::default();
    block
        .write_with_new_inside(&path, "- hello\n", "cycle-1")
        .expect("write");

    let written = fs::read_to_string(&path).unwrap();
    assert!(written.starts_with("<!-- klyntbot:managed:start"));
    assert!(written.contains("- hello"));
}

#[test]
fn detects_user_modification_to_managed_range() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("CLAUDE.md");
    fs::write(&path, SAMPLE).unwrap();

    let block = ManagedBlock::read(&path).expect("parse");
    let prior_hash = block.inside_hash();
    // Simulate the user editing inside.
    let mutated = SAMPLE.replace("managed line 1", "user-edited line");
    fs::write(&path, &mutated).unwrap();
    let block2 = ManagedBlock::read(&path).expect("parse");
    let outcome =
        block2.write_with_new_inside_if_unchanged(&path, "- new\n", "cycle-2", Some(&prior_hash));
    assert!(matches!(outcome, Err(ManagedBlockError::UserConflict)));
}
