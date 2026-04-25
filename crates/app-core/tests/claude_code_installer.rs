use app_core::coding_memory::installer::ClaudeCodeInstaller;
use serde_json::Value;
use tempfile::TempDir;

fn read(p: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn install_into_empty_dir_creates_settings_file() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    let hook = std::path::PathBuf::from("/usr/local/bin/klyntbot-hook");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    let v = read(&settings);
    let hooks = v.get("hooks").unwrap().as_object().unwrap();
    for event in [
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Stop",
        "PreCompact",
    ] {
        assert!(hooks.contains_key(event), "missing {event}");
    }
}

#[test]
fn install_preserves_unrelated_user_hooks() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{
        "hooks": {
            "SessionStart": [{"matcher":"my-own","hooks":[{"type":"command","command":"echo mine"}]}]
        }
    }"#).unwrap();
    let hook = std::path::PathBuf::from("/bin/kh");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    let v = read(&settings);
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "user + klyntbot entries must both exist");
}

#[test]
fn install_twice_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    let hook = std::path::PathBuf::from("/bin/kh");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    let v = read(&settings);
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 1, "second install must not duplicate");
}

#[test]
fn uninstall_removes_managed_entries_and_leaves_user_ones() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{
        "hooks": {
            "SessionStart": [{"matcher":"my-own","hooks":[{"type":"command","command":"echo mine"}]}]
        }
    }"#).unwrap();
    let hook = std::path::PathBuf::from("/bin/kh");
    ClaudeCodeInstaller::install(&settings, &hook).unwrap();
    ClaudeCodeInstaller::uninstall(&settings).unwrap();
    let v = read(&settings);
    let arr = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["matcher"], "my-own");
}

#[test]
fn install_creates_backup_of_preexisting_file() {
    let dir = TempDir::new().unwrap();
    let settings = dir.path().join("settings.json");
    std::fs::write(&settings, r#"{"hooks":{}}"#).unwrap();
    ClaudeCodeInstaller::install(&settings, &std::path::PathBuf::from("/bin/kh")).unwrap();
    let backups: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.contains("klyntbot-backup"))
        .collect();
    assert_eq!(backups.len(), 1);
}
