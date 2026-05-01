use klynt_execpolicy::{Decision, Policy};

#[test]
fn empty_policy_falls_through() {
    let p = Policy::empty();
    assert!(matches!(
        p.eval(&["bash", "-c", "echo hi"], None),
        Decision::FallThrough
    ));
}

#[test]
fn load_from_dir_missing_is_empty() {
    let p = Policy::load_from_dir(std::path::Path::new("/tmp/does-not-exist-zz")).unwrap();
    assert!(matches!(p.eval(&["x"], None), Decision::FallThrough));
}
