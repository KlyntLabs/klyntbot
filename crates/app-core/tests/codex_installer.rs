//! CodexInstaller tests.

use app_core::coding_memory::codex_installer::CodexInstaller;

#[test]
fn install_writes_managed_hooks_block_to_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    CodexInstaller::install(&cfg, std::path::Path::new("/usr/local/bin/hook")).unwrap();
    let body = std::fs::read_to_string(&cfg).unwrap();
    assert!(body.contains("klyntbot-managed:start"));
    assert!(body.contains("klyntbot-managed:end"));
    assert!(body.contains("event = \"session.start\""));
    assert!(body.contains("event = \"session.end\""));
}

#[test]
fn uninstall_strips_only_managed_block() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    let body = "[other]\nkey=1\n# klyntbot-managed:start\n[[hooks]]\nevent=\"session.start\"\ncommand=\"x\"\n# klyntbot-managed:end\n";
    std::fs::write(&cfg, body).unwrap();
    CodexInstaller::uninstall(&cfg).unwrap();
    let after = std::fs::read_to_string(&cfg).unwrap();
    assert!(after.contains("[other]"));
    assert!(!after.contains("klyntbot-managed"));
}
