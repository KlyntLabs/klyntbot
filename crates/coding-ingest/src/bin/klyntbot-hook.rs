//! `klyntbot-hook` — shell binary users' coding CLIs spawn per hook.
//!
//! Phase 1: parses CLI arg (which adapter to use), reads stdin, logs to
//! stderr only. No socket writes — wire-up lands in Phase 2.
//!
//! Usage: `klyntbot-hook <source> [hook-event-name]`
//!
//!   source ∈ { claude-code, codex, kimi-cli, opencode }
//!
//! Exits 0 on success, 2 on bad args, 1 on read failure. Never blocks the
//! parent CLI — all IO has a hard timeout in Phase 2.

use std::io::{self, Read};
use std::process::ExitCode;

const USAGE: &str = "\
usage: klyntbot-hook <source> [hook-event]
  source ∈ { claude-code, codex, kimi-cli, opencode }
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let source = match args.next() {
        Some(s) => s,
        None => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let source_ok = matches!(
        source.as_str(),
        "claude-code" | "codex" | "kimi-cli" | "opencode"
    );
    if !source_ok {
        eprintln!("unknown source `{source}`\n{USAGE}");
        return ExitCode::from(2);
    }

    let hook_event = args.next().unwrap_or_else(|| "unknown".to_string());

    let mut raw = Vec::with_capacity(8 * 1024);
    if let Err(e) = io::stdin().read_to_end(&mut raw) {
        eprintln!("klyntbot-hook: stdin read failed: {e}");
        return ExitCode::from(1);
    }

    // Phase 1: observational only — log presence, never transmit.
    eprintln!(
        "klyntbot-hook: source={source} hook_event={hook_event} bytes={} (phase 1 stub — not forwarded)",
        raw.len()
    );

    ExitCode::SUCCESS
}
