//! `klyntbot-hook` — thin shim that calls into `coding_ingest::hook_cli::run`.
//!
//! Kept for backwards compatibility. Production users get the same logic via
//! `desktop --hook` (the desktop binary's hook subcommand path), so a separate
//! binary is no longer required for new installs.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(coding_ingest::hook_cli::run(args));
}
