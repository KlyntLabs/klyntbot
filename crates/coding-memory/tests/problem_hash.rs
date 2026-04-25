use coding_memory::problem_hash::ProblemHash;

#[test]
fn same_prompt_same_hash() {
    assert_eq!(
        ProblemHash::of("fix the null pointer in parser"),
        ProblemHash::of("fix the null pointer in parser"),
    );
}

#[test]
fn whitespace_invariant() {
    assert_eq!(
        ProblemHash::of("fix   the\nnull\tpointer"),
        ProblemHash::of("fix the null pointer"),
    );
}

#[test]
fn case_invariant() {
    assert_eq!(
        ProblemHash::of("Fix The Null Pointer"),
        ProblemHash::of("fix the null pointer"),
    );
}

#[test]
fn different_prompts_different_hashes() {
    assert_ne!(
        ProblemHash::of("null pointer"),
        ProblemHash::of("off-by-one loop"),
    );
}

#[test]
fn hash_is_stable_hex_16() {
    let h = ProblemHash::of("fix the null pointer");
    let s = h.as_str();
    assert_eq!(s.len(), 16);
    assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
}
