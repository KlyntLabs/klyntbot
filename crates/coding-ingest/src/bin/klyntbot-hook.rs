//! `klyntbot-hook` — shell binary users' coding CLIs spawn per hook.
//!
//! Usage:
//!   klyntbot-hook <source> [hook-event]     # normal forwarding
//!   klyntbot-hook status                    # socket/buffer/daemon report
//!
//!   source ∈ { claude-code, codex, kimi-cli, opencode }
//!
//! Exits 0 on success, 2 on bad args, 1 on fatal IO. Never blocks the parent.

use coding_ingest::adapters::{claude_code::ClaudeCodeAdapter, IngestAdapter};
use coding_ingest::desktop_lock::is_desktop_alive;
use coding_ingest::event::AgentEvent;
use coding_ingest::excludes::{default_exclude_globs, ExcludeSet};
use coding_ingest::hook_client::HookClient;
use coding_ingest::scope_resolver::resolve_scope;
use coding_ingest::store::IngestEventLogRepo;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  klyntbot-hook <source> [hook-event]
  klyntbot-hook status
  source ∈ { claude-code, codex, kimi-cli, opencode }
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(first) = args.first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    if first == "status" {
        return run_status();
    }

    let source = first.clone();
    let hook_event = args.get(1).cloned().unwrap_or_else(|| "unknown".into());

    if !matches!(
        source.as_str(),
        "claude-code" | "codex" | "kimi-cli" | "opencode"
    ) {
        eprintln!("unknown source `{source}`\n{USAGE}");
        return ExitCode::from(2);
    }

    let mut raw = Vec::with_capacity(8 * 1024);
    if let Err(e) = io::stdin().read_to_end(&mut raw) {
        eprintln!("klyntbot-hook: stdin read: {e}");
        return ExitCode::from(1);
    }

    // Only Claude Code is implemented end-to-end in Phase 2.
    if source != "claude-code" {
        eprintln!("klyntbot-hook: source `{source}` not yet wired (Phase 7)");
        return ExitCode::SUCCESS;
    }

    let event = match ClaudeCodeAdapter.parse(&hook_event, &raw) {
        Ok(Some(e)) => e,
        Ok(None) => return ExitCode::SUCCESS, // silently ignore hooks we don't record
        Err(e) => {
            eprintln!("klyntbot-hook: parse: {e}");
            return ExitCode::from(1);
        }
    };
    let event = enrich_with_scope(event);

    // Defense-in-depth: drop excluded events before they hit transport.
    let excludes = ExcludeSet::compile(&default_exclude_globs())
        .unwrap_or_else(|_| ExcludeSet::compile(&[]).expect("empty glob set"));
    if excludes.should_drop(&event) {
        return ExitCode::SUCCESS;
    }

    let home = home_dir();
    let client = HookClient::new(
        home.join("ingest.sock"),
        home.join("ingest-buffer.jsonl"),
        home.join(".hook-warn.stamp"),
    );
    // Fire-and-forget — bounded by 200ms socket deadline inside HookClient.
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("klyntbot-hook: runtime: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = rt.block_on(client.send(&event)) {
        eprintln!("klyntbot-hook: send: {e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run_status() -> ExitCode {
    let home = home_dir();
    let sock = home.join("ingest.sock");
    let lock = home.join("desktop.lock");
    let buf = home.join("ingest-buffer.jsonl");
    let db = home.join("data.db");
    let alive = is_desktop_alive(&lock);
    let buf_size = std::fs::metadata(&buf).map(|m| m.len()).unwrap_or(0);
    let last_distilled = read_last_distilled(&db).unwrap_or_else(|| "unknown".into());

    println!(
        "socket:           {} ({})",
        sock.display(),
        if sock.exists() { "present" } else { "absent" }
    );
    println!(
        "desktop.lock:     {} ({})",
        lock.display(),
        if alive { "alive" } else { "stale or missing" }
    );
    println!("buffer:           {} ({} bytes)", buf.display(), buf_size);
    println!("last distilled:   {last_distilled}");
    ExitCode::SUCCESS
}

/// Best-effort read of the most recent processed-row timestamp. Returns
/// `None` on any failure — status must always succeed.
fn read_last_distilled(db_path: &std::path::Path) -> Option<String> {
    if !db_path.exists() {
        return None;
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    rt.block_on(async {
        let opts = SqliteConnectOptions::new()
            .filename(db_path)
            .read_only(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_millis(500))
            .connect_with(opts)
            .await
            .ok()?;
        let repo = IngestEventLogRepo::new(pool);
        repo.last_distilled_at().await.ok().flatten()
    })
}

fn home_dir() -> PathBuf {
    let raw = std::env::var("KLYNTBOT_HOME").ok();
    let root = match raw.as_deref() {
        Some(p) if !p.trim().is_empty() => expand_tilde(p),
        _ => {
            let h = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(h).join(".klyntbot")
        }
    };
    let _ = std::fs::create_dir_all(&root);
    root
}

/// Expand a leading `~` against `$HOME`. dotenv-loaded env vars carry the
/// literal string (no shell expansion), so `KLYNTBOT_HOME=~/.klyntbot-dev`
/// would otherwise create a relative `~/` directory under cwd.
fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(h) = std::env::var("HOME") {
            return PathBuf::from(h).join(rest);
        }
    }
    if p == "~" {
        if let Ok(h) = std::env::var("HOME") {
            return PathBuf::from(h);
        }
    }
    PathBuf::from(p)
}

fn enrich_with_scope(event: AgentEvent) -> AgentEvent {
    let AgentEvent::V1(mut v1) = event;
    if v1.repo.is_none() {
        v1.repo = resolve_scope(&v1.cwd);
    }
    AgentEvent::V1(v1)
}
