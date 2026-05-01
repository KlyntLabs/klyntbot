use klynt_execpolicy::{Decision, Policy};

#[test]
fn append_session_allow_prefix_grants_immediate_allow() {
    let policy = Policy::empty();
    policy.append_session_allow_prefix(&["cargo nextest run"]);
    let dec = policy.eval(&["cargo", "nextest", "run", "--workspace"], None);
    assert_eq!(dec, Decision::Allow);
}

#[test]
fn session_allow_does_not_persist_to_disk() {
    let dir = tempfile::TempDir::new().unwrap();
    {
        let policy = Policy::load_from_dir(dir.path()).unwrap();
        policy.append_session_allow_prefix(&["foo bar"]);
    }
    // New load: previous session's runtime additions are gone.
    let policy2 = Policy::load_from_dir(dir.path()).unwrap();
    let dec = policy2.eval(&["foo", "bar", "baz"], None);
    assert_eq!(dec, Decision::FallThrough);
}
