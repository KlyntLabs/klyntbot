//! CodexInstaller tests.

use app_core::coding_memory::codex_installer::CodexInstaller;

#[test]
fn install_is_noop_when_no_config_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    CodexInstaller::install(&cfg, std::path::Path::new("/usr/local/bin/hook")).unwrap();
    assert!(
        !cfg.exists(),
        "codex install must not create config; it is poll-only"
    );
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
