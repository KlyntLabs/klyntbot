#![cfg(target_os = "linux")]
use klynt_sandbox::bwrap::build_bwrap_args;
use klynt_sandbox::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use std::path::PathBuf;

#[test]
fn cwd_writes_only_with_block_network() {
    let p = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/work"));
    let args = build_bwrap_args(&p, "/usr/bin/echo", &["hi"]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();

    // Namespace flags
    assert!(argv.contains(&"--unshare-user"));
    assert!(argv.contains(&"--unshare-pid"));
    assert!(argv.contains(&"--unshare-net")); // network blocked
    assert!(argv.contains(&"--die-with-parent"));
    assert!(argv.contains(&"--new-session"));

    // Filesystem
    assert!(argv.contains(&"--ro-bind"));
    let bind_idx = argv.iter().position(|s| *s == "--bind").unwrap();
    assert_eq!(argv[bind_idx + 1], "/tmp/work");
    assert_eq!(argv[bind_idx + 2], "/tmp/work");

    // /proc and /dev
    assert!(argv.windows(2).any(|w| w[0] == "--proc" && w[1] == "/proc"));
    assert!(argv.windows(2).any(|w| w[0] == "--dev" && w[1] == "/dev"));

    // chdir to cwd
    assert!(argv
        .windows(2)
        .any(|w| w[0] == "--chdir" && w[1] == "/tmp/work"));

    // Delimiter then program/args at end
    let dash = argv.iter().rposition(|s| *s == "--").unwrap();
    assert_eq!(argv[dash + 1], "/usr/bin/echo");
    assert_eq!(argv[dash + 2], "hi");
}

#[test]
fn read_only_policy_omits_writable_bind() {
    let p = SandboxPolicy::read_only(PathBuf::from("/tmp/ro"));
    let args = build_bwrap_args(&p, "/usr/bin/cat", &["/tmp/ro/file"]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    // Read-only: no --bind, only --ro-bind
    assert!(!argv.contains(&"--bind"));
    assert!(argv.contains(&"--ro-bind"));
}

#[test]
fn allow_network_omits_unshare_net() {
    let mut p = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/n"));
    p.network = NetworkConstraints::Allow;
    let args = build_bwrap_args(&p, "/usr/bin/curl", &["http://example.com"]);
    assert!(!args.iter().any(|s| s == "--unshare-net"));
}

#[test]
fn fs_constraints_none_blocks_all_writes() {
    let p = SandboxPolicy {
        cwd: PathBuf::from("/tmp/n"),
        fs: FsConstraints::None,
        network: NetworkConstraints::Block,
        allow_process_fork: false,
    };
    let args = build_bwrap_args(&p, "/bin/true", &[]);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    // No writable bind for any path when FsConstraints::None
    assert!(!argv.contains(&"--bind"));
}

#[test]
fn helper_policy_roundtrip() {
    use klynt_sandbox::helper_proto::{HelperMode, HelperPolicy};
    use klynt_sandbox::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
    use std::path::PathBuf;

    let p = HelperPolicy {
        mode: HelperMode::WithBwrap,
        sandbox: SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp/x")),
    };
    let encoded = p.to_base64_json().unwrap();
    let parsed = HelperPolicy::from_base64_json(&encoded).unwrap();
    assert_eq!(parsed.mode, HelperMode::WithBwrap);
    assert!(matches!(
        parsed.sandbox.fs,
        FsConstraints::WriteCwdReadAll { .. }
    ));
    assert!(matches!(parsed.sandbox.network, NetworkConstraints::Block));
}
