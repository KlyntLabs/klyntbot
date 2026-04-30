use klynt_sandbox::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use std::path::PathBuf;

#[test]
fn policy_for_cwd_only_writes() {
    let p = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/x"));
    assert_eq!(p.cwd, PathBuf::from("/tmp/x"));
    assert!(matches!(p.network, NetworkConstraints::Block));
    assert!(matches!(p.fs, FsConstraints::WriteCwdReadAll { .. }));
    assert!(!p.policy_hash().is_empty());
}
