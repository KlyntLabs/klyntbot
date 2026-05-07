use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Benchmark Phase-2 critical paths that contribute to first-token latency.
///
/// Full e2e bench (warm runtime → process → first ContentChunk) requires a
/// scripted echo provider + in-memory storage + pre-built tool registry.
/// That harness is deferred until the agent test-helper crate stabilises.
/// These micro-benches cover the new Phase-2 surfaces that sit on the hot path.
///
/// Phase-2 perf pass applied (2026-05-02):
///   • SoulContextSource now uses mtime memoization to avoid redundant disk reads
///     when KLYNTBOT.md has not changed between turns.
///   • Tool-registry per-thread cache and SkillActivator LRU remain as future
///     optimisations if profiling shows they are needed.

fn bench_tool_search(c: &mut Criterion) {
    let meta: Vec<klynt_core::tools::tool_search::ToolMeta> = vec![
        ("bash", "Run a shell command"),
        ("read", "Read a file"),
        ("edit", "Edit a file in place"),
        ("write", "Write a file"),
        ("apply_patch", "Apply a unified-diff patch"),
        ("glob", "List files matching a glob pattern"),
        ("grep", "Search file contents with regex"),
        ("list_dir", "List directory contents"),
        ("ask_user", "Ask the user a question"),
        ("web_fetch", "Fetch a URL"),
        ("tool_search", "Search available tools"),
        ("enter_plan_mode", "Enter plan mode"),
        ("exit_plan_mode", "Exit plan mode"),
    ]
    .into_iter()
    .map(|(name, desc)| klynt_core::tools::tool_search::ToolMeta {
        name: name.into(),
        aliases: vec![],
        description: desc.into(),
    })
    .collect();

    let index = klynt_core::tools::tool_search::ToolIndex::build(&meta);

    c.bench_function("tool_search_10_results", |b| {
        b.iter(|| {
            let _ = index.search(black_box("read file"), 10);
        });
    });
}

criterion_group!(benches, bench_tool_search);
criterion_main!(benches);
