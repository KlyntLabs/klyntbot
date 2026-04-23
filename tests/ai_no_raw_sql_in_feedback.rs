use std::fs;
use std::path::Path;

#[test]
fn feedback_rs_has_no_raw_sql() {
    let path = Path::new("crates/cognitive/src/services/reforge/feedback.rs");
    let src = fs::read_to_string(path).expect("read feedback.rs");

    for pat in ["sqlx::query!", "sqlx::query_scalar!", "sqlx::query(", "sqlx::query_scalar("] {
        assert!(
            !src.contains(pat),
            "feedback.rs contains forbidden pattern: {}",
            pat
        );
    }

    for verb in ["SELECT ", "INSERT ", "UPDATE ", "DELETE "] {
        assert!(
            !src.contains(verb),
            "feedback.rs contains inline SQL verb: {}",
            verb
        );
    }
}
