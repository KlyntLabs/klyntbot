use klynt_core::privacy::PrivacyGuard;
use std::path::Path;

#[test]
fn excludes_secret_files() {
    let g = PrivacyGuard::from_globs(&["**/.env", "**/*.key", "secrets/**"]).unwrap();
    assert!(g.is_excluded(Path::new("/repo/.env")));
    assert!(g.is_excluded(Path::new("/repo/keys/api.key")));
    assert!(g.is_excluded(Path::new("secrets/db.json")));
    assert!(!g.is_excluded(Path::new("/repo/src/main.rs")));
}

#[test]
fn bash_command_paths_inspected() {
    let g = PrivacyGuard::from_globs(&["**/.env"]).unwrap();
    assert!(g.bash_command_touches_excluded("cat .env"));
    assert!(!g.bash_command_touches_excluded("cargo build"));
}
