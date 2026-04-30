use klynt_sandbox::policy::{FsConstraints, SandboxPolicy};
use landlock::{
    ABI, Access, AccessFs, CompatLevel, PathBeneath, PathFd, RestrictionStatus, Ruleset,
    RulesetAttr, RulesetCreatedAttr, RulesetStatus,
};

/// Reserved exit codes for the helper:
///   124 = timeout-by-parent (used by parent runner via SIGKILL)
///   125 = sandbox unavailable (Landlock returned NotEnforced)
///   126 = sandbox setup failed (other)
/// Any other code is the wrapped program's own exit code.
pub const EXIT_SANDBOX_UNAVAILABLE: i32 = 125;
pub const EXIT_SANDBOX_SETUP_FAILED: i32 = 126;

pub fn apply_no_new_privs() -> Result<(), String> {
    // SAFETY: prctl is libc; PR_SET_NO_NEW_PRIVS=38, value 1 enables.
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(format!(
            "prctl(PR_SET_NO_NEW_PRIVS) failed: {}", std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Applies Landlock filesystem restrictions for the current process.
/// Returns Ok(()) on FullyEnforced; Err with reserved exit code intent on degraded paths.
pub fn apply_landlock(p: &SandboxPolicy) -> Result<RestrictionStatus, String> {
    let abi = ABI::new_current();
    if abi == ABI::Unsupported {
        return Err("landlock unavailable: kernel < 5.13 or denied".into());
    }

    let cwd = match &p.fs {
        FsConstraints::WriteCwdReadAll { cwd } => cwd,
        FsConstraints::ReadCwdOnly { cwd } => cwd,
        FsConstraints::None => return Ok(RestrictionStatus {
            ruleset: RulesetStatus::FullyEnforced, no_new_privs: true,
        }), // nothing to enforce
    };

    let read_all = AccessFs::from_read(abi);
    let write_set = match &p.fs {
        FsConstraints::WriteCwdReadAll { .. } => AccessFs::from_all(abi),
        FsConstraints::ReadCwdOnly { .. }     => AccessFs::from_read(abi),
        FsConstraints::None => unreachable!(),
    };

    let root_fd = PathFd::new("/").map_err(|e| format!("PathFd /: {e}"))?;
    let cwd_fd = PathFd::new(cwd.as_os_str())
        .map_err(|e| format!("PathFd {}: {e}", cwd.display()))?;

    let status = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi)).map_err(|e| format!("handle_access: {e}"))?
        .create().map_err(|e| format!("Ruleset::create: {e}"))?
        .add_rule(PathBeneath::new(root_fd, read_all))
            .map_err(|e| format!("add_rule root ro: {e}"))?
        .add_rule(PathBeneath::new(cwd_fd, write_set))
            .map_err(|e| format!("add_rule cwd rw: {e}"))?
        .restrict_self().map_err(|e| format!("restrict_self: {e}"))?;

    Ok(status)
}
