// crates/klynt-sandbox/tests/helper_locator.rs
#![cfg(target_os = "linux")]
use klynt_sandbox::linux::locate_helper;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;

#[test]
fn locates_helper_next_to_current_exe() {
    let parent_dir = tempfile::tempdir().unwrap();
    let helper_path = parent_dir.path().join("klynt-sandbox-helper");
    File::create(&helper_path).unwrap();
    fs::set_permissions(&helper_path, fs::Permissions::from_mode(0o755)).unwrap();
    let parent_exe = parent_dir.path().join("klyntbot");
    File::create(&parent_exe).unwrap();
    let found = locate_helper(Some(&parent_exe)).expect("located");
    assert_eq!(found, helper_path);
}

#[test]
fn returns_err_when_helper_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let fake_exe = dir.path().join("klyntbot");
    File::create(&fake_exe).unwrap();
    assert!(locate_helper(Some(&fake_exe)).is_err());
}
