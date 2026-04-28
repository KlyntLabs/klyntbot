//! KimiInstaller tests.

use app_core::coding_memory::kimi_installer::KimiInstaller;

#[test]
fn install_writes_managed_block_to_kimi_config_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    KimiInstaller::install(&cfg, std::path::Path::new("/usr/local/bin/hook")).unwrap();
    let body = std::fs::read_to_string(&cfg).unwrap();
    assert!(body.contains("klyntbot-managed:start"));
    assert!(body.contains("klyntbot-managed:end"));
    assert!(body.contains("event = \"SessionStart\""));
    assert!(body.contains("event = \"SessionEnd\""));
    assert!(body.contains("event = \"PreToolUse\""));
}

#[test]
fn uninstall_removes_managed_block_only() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    let body = "[user]\nkey = 1\n# klyntbot-managed:start\n[[hooks]]\nevent = \"SessionStart\"\ncommand = \"x\"\n# klyntbot-managed:end\n";
    std::fs::write(&cfg, body).unwrap();
    KimiInstaller::uninstall(&cfg).unwrap();
    let after = std::fs::read_to_string(&cfg).unwrap();
    assert!(after.contains("[user]"));
    assert!(!after.contains("klyntbot-managed"));
}
