//! Hook CLI entry point — shared by the standalone `klyntbot-hook` binary
//! and the desktop binary's `--hook` subcommand path.
//!
//! Same logic, two callers. Use [`run`] to forward events; use [`run_status`]
//! to print a diagnostic report.

use crate::adapters::{claude_code::ClaudeCodeAdapter, IngestAdapter};
use crate::desktop_lock::is_desktop_alive;
use crate::event::AgentEvent;
use crate::excludes::{default_exclude_globs, ExcludeSet};
use crate::hook_client::HookClient;
use crate::scope_resolver::resolve_scope;
use crate::store::IngestEventLogRepo;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::io::{self, Read};
use std::path::PathBuf;

const USAGE: &str = "\
usage:
  klyntbot-hook <source> [hook-event]
  klyntbot-hook status
  klyntbot-hook context [--session-start | --user-prompt-submit <text>] [--repo <repo>]
  klyntbot-hook git-post-commit
  source ∈ { claude-code, codex, kimi-cli, opencode }
";

/// Run the hook CLI with the given args (excluding argv[0]).
pub fn run(args: Vec<String>) -> i32 {
    let Some(first) = args.first() else {
        eprintln!("{USAGE}");
        return 2;
    };

    if first == "status" {
        return run_status();
    }

    if first == "context" {
        return run_context(args);
    }

    if first == "git-post-commit" {
        return run_git_post_commit();
    }

    let source = first.clone();
    let hook_event = args.get(1).cloned().unwrap_or_else(|| "unknown".into());

    if !matches!(
        source.as_str(),
        "claude-code" | "codex" | "kimi-cli" | "opencode"
    ) {
        eprintln!("unknown source `{source}`\n{USAGE}");
        return 2;
    }

    let mut raw = Vec::with_capacity(8 * 1024);
    if let Err(e) = io::stdin().read_to_end(&mut raw) {
        eprintln!("klyntbot-hook: stdin read: {e}");
        return 1;
    }

    if source != "claude-code" {
        eprintln!("klyntbot-hook: source `{source}` not yet wired (Phase 7)");
        return 0;
    }

    let event = match ClaudeCodeAdapter.parse(&hook_event, &raw) {
        Ok(Some(e)) => e,
        Ok(None) => return 0,
        Err(e) => {
            eprintln!("klyntbot-hook: parse: {e}");
            return 1;
        }
    };
    let event = enrich_with_scope(event);

    let excludes = ExcludeSet::compile(&default_exclude_globs())
        .unwrap_or_else(|_| ExcludeSet::compile(&[]).expect("empty glob set"));
    if excludes.should_drop(&event) {
        return 0;
    }

    let home = home_dir();
    let client = HookClient::new(
        home.join("ingest.sock"),
        home.join("ingest-buffer.jsonl"),
        home.join(".hook-warn.stamp"),
    );
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("klyntbot-hook: runtime: {e}");
            return 1;
        }
    };
    if let Err(e) = rt.block_on(client.send(&event)) {
        eprintln!("klyntbot-hook: send: {e}");
        return 1;
    }
    0
}

fn run_context(args: Vec<String>) -> i32 {
    let mut session_start = false;
    let mut user_prompt: Option<String> = None;
    let mut repo: Option<String> = None;
    let mut iter = args.iter().skip(1);
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--session-start" => session_start = true,
            "--user-prompt-submit" => {
                user_prompt = iter.next().map(|s| s.to_string());
            }
            "--repo" => repo = iter.next().map(|s| s.to_string()),
            _ => {}
        }
    }

    let home = home_dir();
    let socket = crate::transport::UnixIngestSocket::new(home.join("ingest.sock"));
    let payload = if session_start {
        serde_json::json!({"op": "render_session_start", "repo": repo})
    } else if let Some(q) = user_prompt {
        serde_json::json!({"op": "render_user_prompt", "query": q, "repo": repo})
    } else {
        eprintln!("context requires --session-start or --user-prompt-submit");
        return 2;
    };

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("klyntbot-hook: runtime: {e}");
            return 1;
        }
    };

    match rt.block_on(crate::transport::request_response(
        &socket,
        &payload,
        std::time::Duration::from_millis(800),
    )) {
        Ok(resp) => {
            if let Some(md) = resp.get("markdown").and_then(|v| v.as_str()) {
                print!("{md}");
            } else {
                print!("<!-- klyntbot recall unavailable -->");
            }
        }
        Err(_) => {
            print!("<!-- klyntbot recall unavailable -->");
        }
    }
    0
}

fn run_status() -> i32 {
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
    0
}

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

fn run_git_post_commit() -> i32 {
    use crate::adapters::git_post_commit::GitPostCommitAdapter;

    let mut raw = Vec::with_capacity(2 * 1024);
    if let Err(e) = std::io::Read::read_to_end(&mut std::io::stdin(), &mut raw) {
        eprintln!("klyntbot-hook git-post-commit: stdin read: {e}");
        return 1;
    }
    let event = match GitPostCommitAdapter::parse(&raw) {
        Ok(Some(e)) => e,
        Ok(None) => return 0,
        Err(e) => {
            eprintln!("klyntbot-hook git-post-commit: parse: {e}");
            return 1;
        }
    };

    let home = home_dir();
    let client = HookClient::new(
        home.join("ingest.sock"),
        home.join("ingest-buffer.jsonl"),
        home.join(".hook-warn.stamp"),
    );
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("klyntbot-hook git-post-commit: runtime: {e}");
            return 1;
        }
    };
    if let Err(e) = rt.block_on(client.send(&event)) {
        eprintln!("klyntbot-hook git-post-commit: send: {e}");
        return 1;
    }
    0
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
