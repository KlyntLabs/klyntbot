//! KimiInstaller tests.

use app_core::coding_memory::kimi_installer::KimiInstaller;

#[test]
fn install_writes_managed_block_to_kimi_hooks_json() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("hooks.json");
    KimiInstaller::install(&cfg, std::path::Path::new("/usr/local/bin/hook")).unwrap();
    let body = std::fs::read_to_string(&cfg).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let entries = v["klyntbot"].as_array().unwrap();
    assert_eq!(entries.len(), 13, "expected 13 hooks, got {}", entries.len());
}

#[test]
fn uninstall_removes_klyntbot_section() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("hooks.json");
    std::fs::write(&cfg, r#"{"user":[{"x":1}],"klyntbot":[{"y":2}]}"#).unwrap();
    KimiInstaller::uninstall(&cfg).unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(v.get("klyntbot").is_none());
    assert!(v.get("user").is_some());
}
