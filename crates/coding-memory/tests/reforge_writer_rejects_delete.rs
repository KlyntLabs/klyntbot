//! ReforgeWriter must reject raw DELETE operations.

use coding_memory::reforge::writer::ReforgeWriter;

#[test]
fn reforge_writer_rejects_delete() {
    let writer = ReforgeWriter::new();
    let result = writer.reject_delete("semantic_facts", "test reason");
    assert!(
        result.is_err(),
        "expected DELETE to be rejected"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains("rejected DELETE"), "expected 'rejected DELETE' in error: {err}");
}
