use klynt_hooks::schema::HookConfig;

#[test]
fn parse_hooks_toml_basic() {
    let toml_src = r#"
[[hook]]
event = "PreToolUse"
matcher = "Bash(*)"
command = "scripts/log-bash.sh"
timeout_ms = 5000
fail_open = true

[[hook]]
event = "PostToolUse"
matcher = "Edit(./crates/**)"
command = "scripts/auto-format-rust.sh"
timeout_ms = 10000
"#;
    let cfg: HookConfig = toml::from_str(toml_src).expect("parse");
    assert_eq!(cfg.hook.len(), 2);
    assert_eq!(cfg.hook[0].matcher.as_deref(), Some("Bash(*)"));
    assert_eq!(cfg.hook[0].timeout_ms, Some(5000));
    assert_eq!(cfg.hook[0].fail_open, Some(true));
    assert_eq!(cfg.hook[1].command, "scripts/auto-format-rust.sh");
}
