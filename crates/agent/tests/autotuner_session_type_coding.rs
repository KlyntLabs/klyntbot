//! When the agent loop runs with a coding-memory scope_repo_id present,
//! the autotuner's ShadowContext must carry session_type = Some("coding").

use agent::autotuner::hooks::derive_session_type;

#[test]
fn coding_repo_yields_session_type_coding() {
    let st = derive_session_type(/*scope_repo_id*/ Some("github.com/foo/bar"));
    assert_eq!(st.as_deref(), Some("coding"));
}

#[test]
fn personal_yields_no_session_type_tag() {
    let st = derive_session_type(/*scope_repo_id*/ None);
    assert_eq!(st, None);
}
