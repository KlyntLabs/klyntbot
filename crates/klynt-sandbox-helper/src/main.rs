//! klynt-sandbox-helper — Linux Landlock + seccomp child binary.
//!
//! Vendored from `codex-rs/linux-sandbox/`. On Linux, applies
//! `no_new_privs` + a Landlock ruleset derived from the parsed CLI
//! policy, then `execvp`s the target program. On non-Linux platforms
//! the binary prints an error and exits with code 2.

#[cfg(target_os = "linux")]
mod cli;
#[cfg(target_os = "linux")]
mod landlock_apply;

#[cfg(target_os = "linux")]
fn main() {
    use landlock::RulesetStatus;
    use landlock_apply::{
        apply_landlock, apply_no_new_privs, EXIT_SANDBOX_SETUP_FAILED, EXIT_SANDBOX_UNAVAILABLE,
    };
    use std::os::unix::process::CommandExt as _;

    let argv: Vec<String> = std::env::args().collect();
    let parsed = match cli::parse(&argv) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("klynt-sandbox-helper: {e}");
            std::process::exit(EXIT_SANDBOX_SETUP_FAILED);
        }
    };

    if let Err(e) = apply_no_new_privs() {
        eprintln!("klynt-sandbox-helper: {e}");
        std::process::exit(EXIT_SANDBOX_SETUP_FAILED);
    }

    match apply_landlock(&parsed.policy.sandbox) {
        Ok(status) => {
            if status.ruleset != RulesetStatus::FullyEnforced
                && parsed.policy.mode == klynt_sandbox::helper_proto::HelperMode::LandlockOnly
            {
                // Landlock-only mode + not fully enforced = sandbox is missing.
                eprintln!(
                    "klynt-sandbox-helper: landlock not fully enforced ({:?})",
                    status.ruleset
                );
                std::process::exit(EXIT_SANDBOX_UNAVAILABLE);
            }
        }
        Err(e) => {
            eprintln!("klynt-sandbox-helper: landlock setup: {e}");
            std::process::exit(EXIT_SANDBOX_UNAVAILABLE);
        }
    }

    // execvp the target. CommandExt::exec returns only on failure.
    let mut cmd = std::process::Command::new(&parsed.program);
    cmd.args(&parsed.args);
    let err = cmd.exec();
    eprintln!("klynt-sandbox-helper: exec {}: {err}", parsed.program);
    std::process::exit(EXIT_SANDBOX_SETUP_FAILED);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("klynt-sandbox-helper: not supported on this platform");
    std::process::exit(2);
}
