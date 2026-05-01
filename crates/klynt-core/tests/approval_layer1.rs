use config::schema::CodingPermissions;
use klynt_core::approval::{
    decision::{ApprovalDecision, ApprovalLayer},
    layer1::Layer1,
};

fn perms(allow: &[&str], deny: &[&str], ask: &[&str], default: &str) -> CodingPermissions {
    CodingPermissions {
        allow: allow.iter().map(|s| s.to_string()).collect(),
        deny: deny.iter().map(|s| s.to_string()).collect(),
        ask: ask.iter().map(|s| s.to_string()).collect(),
        default_if_no_match: default.into(),
        mirror_learning: false,
        mirror_cooldown_hours: 24,
        mirror_min_approvals: 5,
    }
}

#[test]
fn deny_beats_allow() {
    let p = perms(&["Bash(*)"], &["Bash(rm -rf *)"], &[], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    let d = l1.evaluate("bash", "rm -rf /tmp/x");
    if let ApprovalDecision::Auto { allowed, .. } = d {
        assert!(!allowed, "rm -rf must be denied");
    } else {
        panic!("expected Auto");
    }
}

#[test]
fn allow_matches() {
    let p = perms(&["Bash(git status*)"], &[], &["Bash(*)"], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    if let ApprovalDecision::Auto { allowed, layer, .. } = l1.evaluate("bash", "git status --short")
    {
        assert!(allowed);
        assert_eq!(layer, ApprovalLayer::Layer1Declarative);
    } else {
        panic!();
    }
}

#[test]
fn ask_falls_through() {
    let p = perms(&[], &[], &["Bash(*)"], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    assert!(matches!(
        l1.evaluate("bash", "anything"),
        ApprovalDecision::Ask { .. }
    ));
}

#[test]
fn no_match_uses_default() {
    let p = perms(&["Read(*)"], &[], &[], "ask");
    let l1 = Layer1::compile(&p).unwrap();
    assert!(matches!(
        l1.evaluate("bash", "echo hi"),
        ApprovalDecision::Ask { .. }
    ));
}
