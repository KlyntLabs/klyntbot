use config::Config;

#[test]
fn coding_permissions_parse() {
    let json = r#"{
      "coding": {
        "permissions": {
          "allow": ["Bash(git status*)"],
          "deny": ["Bash(rm -rf *)"],
          "ask": ["Bash(*)"],
          "defaultIfNoMatch": "ask",
          "mirrorLearning": false
        },
        "sandbox": { "enforce": true }
      }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    let perms = &cfg.coding.permissions;
    assert_eq!(perms.allow, vec!["Bash(git status*)".to_string()]);
    assert_eq!(perms.deny.len(), 1);
    assert_eq!(perms.default_if_no_match, "ask");
    assert!(!perms.mirror_learning);
    assert!(cfg.coding.sandbox.enforce);
}
