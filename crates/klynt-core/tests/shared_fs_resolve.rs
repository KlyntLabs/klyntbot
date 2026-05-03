use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::shared::fs_resolve::{resolve_under_cwd, FsResolveError};
use std::path::PathBuf;

#[test]
fn resolves_relative_path_under_cwd() {
    let cwd = PathBuf::from("/tmp");
    let r = resolve_under_cwd("foo.txt", &cwd, &PrivacyGuard::from_globs(&[]).unwrap()).unwrap();
    // /tmp may symlink to /private/tmp on macOS; assert against canonical cwd.
    let expected = cwd.canonicalize().unwrap_or(cwd).join("foo.txt");
    assert_eq!(r, expected);
}

#[test]
fn rejects_path_outside_cwd() {
    let cwd = PathBuf::from("/tmp/work");
    let r = resolve_under_cwd("/etc/passwd", &cwd, &PrivacyGuard::from_globs(&[]).unwrap());
    assert!(matches!(r, Err(FsResolveError::OutsideCwd { .. })));
}

#[test]
fn rejects_privacy_excluded_path() {
    let g = PrivacyGuard::from_globs(&["**/.env"]).unwrap();
    let r = resolve_under_cwd("config/.env", &PathBuf::from("/tmp"), &g);
    assert!(matches!(r, Err(FsResolveError::PrivacyDenied { .. })));
}

#[test]
fn expands_tilde() {
    // Just verify it doesn't error; actual home expansion depends on $HOME env.
    std::env::set_var("HOME", "/tmp");
    let cwd = PathBuf::from("/tmp");
    let r = resolve_under_cwd(
        "~/sub/file.txt",
        &cwd,
        &PrivacyGuard::from_globs(&[]).unwrap(),
    )
    .unwrap();
    let expected = cwd.canonicalize().unwrap_or(cwd).join("sub/file.txt");
    assert_eq!(r, expected);
}
