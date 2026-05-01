#![cfg(target_os = "linux")]

use crate::policy::{FsConstraints, NetworkConstraints, SandboxPolicy};
use std::path::Path;

/// Builds the argv for `/usr/bin/bwrap`. Does NOT include the bwrap binary
/// path itself — caller invokes Command::new("/usr/bin/bwrap").args(...).
pub fn build_bwrap_args(policy: &SandboxPolicy, program: &str, args: &[&str]) -> Vec<String> {
    let cwd = policy.cwd.to_string_lossy().into_owned();
    let mut a: Vec<String> = Vec::with_capacity(32);

    // Namespace isolation
    a.extend(
        ["--unshare-user", "--unshare-pid"]
            .into_iter()
            .map(String::from),
    );
    if matches!(policy.network, NetworkConstraints::Block) {
        a.push("--unshare-net".into());
    }
    a.push("--die-with-parent".into());
    a.push("--new-session".into());

    // Read-only root mount (essential system dirs)
    for p in ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc"] {
        a.push("--ro-bind".into());
        a.push(p.into());
        a.push(p.into());
    }

    // /proc, /dev, /tmp
    a.push("--proc".into());
    a.push("/proc".into());
    a.push("--dev".into());
    a.push("/dev".into());
    a.push("--tmpfs".into());
    a.push("/tmp".into());

    // Filesystem constraints
    match &policy.fs {
        FsConstraints::WriteCwdReadAll { cwd: w } => {
            let wcwd = w.to_string_lossy().into_owned();
            a.push("--bind".into());
            a.push(wcwd.clone());
            a.push(wcwd);
        }
        FsConstraints::ReadCwdOnly { cwd: r } => {
            let rcwd = r.to_string_lossy().into_owned();
            a.push("--ro-bind".into());
            a.push(rcwd.clone());
            a.push(rcwd);
        }
        FsConstraints::None => {
            // No additional bind beyond /tmp tmpfs above.
        }
    }

    a.push("--chdir".into());
    a.push(cwd);
    a.push("--".into());

    // Inner command: program + args
    a.push(program.into());
    a.extend(args.iter().map(|s| s.to_string()));

    a
}
