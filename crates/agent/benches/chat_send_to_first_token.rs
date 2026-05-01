use criterion::{black_box, criterion_group, criterion_main, Criterion};
use storage::repos::ApprovalHistorySummary;

/// Benchmark Phase-2 critical paths that contribute to first-token latency.
///
/// Full e2e bench (warm runtime → process → first ContentChunk) requires a
/// scripted echo provider + in-memory storage + pre-built tool registry.
/// That harness is deferred until the agent test-helper crate stabilises.
/// These micro-benches cover the new Phase-2 surfaces that sit on the hot path.

fn bench_layer3_approval_eval(c: &mut Criterion) {
    let cfg = klynt_core::approval::layer3::Layer3Config {
        enabled: true,
        min_approvals: 5,
        cooldown_seconds: 86400,
    };
    let summary = ApprovalHistorySummary {
        approval_count: 10,
        denial_count: 0,
        last_decided_at: Some(1),
    };
    let now = 86400 * 2;

    c.bench_function("layer3_mirror_approval_eval", |b| {
        b.iter(|| {
            let _ = klynt_core::approval::layer3::evaluate(
                black_box(&cfg),
                black_box(&summary),
                black_box(now),
            );
        });
    });
}

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

fn bench_args_hash(c: &mut Criterion) {
    let bash_json = r#"{"command":"cat foo.txt | grep bar"}"#;
    let edit_json = r#"{"path":"/some/path/to/file.rs","content":"fn main() {}"}"#;
    c.bench_function("layer3_args_hash_bash", |b| {
        b.iter(|| {
            let _ = klynt_core::approval::layer3::args_hash_for_relevance(
                black_box("bash"),
                black_box(bash_json),
            );
        });
    });
    c.bench_function("layer3_args_hash_edit", |b| {
        b.iter(|| {
            let _ = klynt_core::approval::layer3::args_hash_for_relevance(
                black_box("edit"),
                black_box(edit_json),
            );
        });
    });
}

criterion_group!(benches, bench_layer3_approval_eval, bench_tool_search, bench_args_hash);
criterion_main!(benches);
