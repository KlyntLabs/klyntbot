use klynt_execpolicy::{Decision, Policy};
use std::fs;
use tempfile::TempDir;

#[test]
fn parse_simple_prefix_rule_allow() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("git.rules"),
        r#"
prefix_rule(["git", "status"], decision="allow")
"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let cmd = vec!["git".to_string(), "status".to_string()];
    let dec = policy.eval(&cmd.iter().map(String::as_str).collect::<Vec<_>>(), None);
    assert_eq!(dec, Decision::Allow);
}

#[test]
fn parse_prefix_rule_ask() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("git.rules"),
        r#"
prefix_rule(["git", "push"], decision="ask")
"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let cmd: Vec<&str> = vec!["git", "push"];
    let dec = policy.eval(&cmd, None);
    assert_eq!(dec, Decision::Ask);
}

#[test]
fn parse_prefix_rule_forbid_via_forbidden_keyword() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("danger.rules"),
        r#"
prefix_rule(["rm", "-rf", "/"], decision="forbidden")
"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["rm", "-rf", "/"], None);
    assert_eq!(dec, Decision::Forbid);
}

#[test]
fn no_rule_falls_through() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("git.rules"),
        r#"prefix_rule(["git", "status"], decision="allow")"#,
    ).unwrap();

    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["ls", "-la"], None);
    assert_eq!(dec, Decision::FallThrough);
}

#[test]
fn empty_dir_returns_empty_policy() {
    let dir = TempDir::new().unwrap();
    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["git", "status"], None);
    assert_eq!(dec, Decision::FallThrough);
}

#[test]
fn nonexistent_dir_returns_empty_policy() {
    let path = std::path::Path::new("/nonexistent-klynt-rules-dir-12345");
    let policy = Policy::load_from_dir(path).expect("load");
    let dec = policy.eval(&["git", "status"], None);
    assert_eq!(dec, Decision::FallThrough);
}

#[test]
fn invalid_rule_file_is_skipped() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("good.rules"), r#"prefix_rule(["git", "status"], decision="allow")"#).unwrap();
    fs::write(dir.path().join("bad.rules"), r#"this is not valid starlark"#).unwrap();
    let policy = Policy::load_from_dir(dir.path()).expect("load");
    let dec = policy.eval(&["git", "status"], None);
    assert_eq!(dec, Decision::Allow);
}
