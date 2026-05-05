//! K17 — Review purity.
//!
//! Two-part guard for the `/review` pass:
//!
//! 1. **Shape:** the `ReviewResult` type carries only `summary: String` +
//!    `issues: Vec<ReviewIssue>` (plus IDs). It is structurally incapable
//!    of carrying `MessagePart::FileChange` or `MessagePart::CommandExecution`
//!    variants. Proven via a serde round-trip over arbitrary inputs.
//!
//! 2. **Registry:** the `ToolKitBuilder::register_read_only` call site used by
//!    `coding_review_start` registers only read-only tools. The canonical
//!    mutating tools (`bash`, `write`, `edit`, `apply_patch`, `notebook_edit`)
//!    must be absent from a registry built that way. This is the runtime
//!    half of the guarantee — even a hallucinating reviewer LLM cannot reach
//!    a mutating tool because it isn't in the registry.

use std::sync::Arc;

use proptest::prelude::*;
use storage::messages::parts::ReviewIssue;
use tools_core::ToolRegistry;

use app_core::coding::review_handler::ReviewResult;

// ── Part 1: shape proptest ───────────────────────────────────────────

fn arb_severity() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("info".to_string()),
        Just("warning".to_string()),
        Just("error".to_string()),
    ]
}

fn arb_review_issue() -> impl Strategy<Value = ReviewIssue> {
    (
        arb_severity(),
        proptest::option::of("[a-z/_]{1,40}\\.rs"),
        proptest::option::of(1u32..10_000u32),
        "[A-Za-z0-9 .,!?]{0,80}",
        proptest::option::of("[A-Za-z0-9 .,!?]{0,80}"),
    )
        .prop_map(
            |(severity, file, line, description, suggestion)| ReviewIssue {
                severity,
                file,
                line,
                description,
                suggestion,
            },
        )
}

fn arb_review_result() -> impl Strategy<Value = ReviewResult> {
    (
        "[a-f0-9-]{8,36}",
        "thread:[a-z0-9-]{4,16}",
        "[A-Za-z0-9 .,!?]{0,160}",
        proptest::collection::vec(arb_review_issue(), 0..5),
    )
        .prop_map(|(review_id, thread_id, summary, issues)| ReviewResult {
            review_id,
            thread_id,
            summary,
            issues,
        })
}

proptest! {
    /// Serialize an arbitrary `ReviewResult` and assert the resulting JSON
    /// never contains the discriminator strings used by mutating message-part
    /// variants. This proves the type's serialized shape can never represent
    /// a file change or command execution, which is the structural guarantee
    /// that backs invariant K17.
    #[test]
    fn k17_review_result_shape_is_pure(result in arb_review_result()) {
        let json = serde_json::to_string(&result).expect("serialize");
        prop_assert!(!json.contains("\"file_change\""), "review must not encode file_change: {}", json);
        prop_assert!(!json.contains("\"command_execution\""), "review must not encode command_execution: {}", json);
        // Round-trip preserves the data.
        let back: ReviewResult = serde_json::from_str(&json).expect("roundtrip");
        prop_assert_eq!(back.review_id, result.review_id);
        prop_assert_eq!(back.issues.len(), result.issues.len());
    }
}

// ── Part 2: registry-construction test ───────────────────────────────

const MUTATING_TOOL_NAMES: &[&str] = &["bash", "write", "edit", "apply_patch", "notebook_edit"];

const EXPECTED_READ_ONLY_TOOL_NAMES: &[&str] = &[
    "read",
    "list_dir",
    "glob",
    "grep",
    "tool_search",
    "ask_user",
    "web_fetch",
];

fn build_test_kit() -> klynt_core::ToolKitBuilder {
    let layer1 = Arc::new(
        klynt_core::approval::Layer1::compile(&config::schema::CodingPermissions::default())
            .unwrap(),
    );
    let policy = Arc::new(klynt_execpolicy::Policy::empty());
    let privacy = Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(klynt_core::approval::PendingApprovalsMap::default());
    let bus = Arc::new(bus::DomainEventBus::new(16));
    let host_cache = Arc::new(klynt_core::approval::HostApprovalCache::default());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt
        .block_on(storage::StoragePool::connect_in_memory())
        .unwrap();
    let repos = storage::Repos::from_pool(&pool);

    klynt_core::ToolKitBuilder {
        cwd: std::path::PathBuf::from("/tmp"),
        layer1,
        policy,
        privacy,
        pending,
        bus,
        repos,
        host_cache,
        non_ui_policy: common::tool_channel::NonUiPolicy::Allow,
        hook_engine: None,
        snapshot_repo: None,
        session_key: String::new(),
        history_repo: None,
        mirror_learning_enabled: false,
        mirror_min_approvals: 0,
        mirror_cooldown_seconds: 0,
        repo_id: String::new(),
    }
}

#[test]
fn k17_read_only_registry_excludes_mutating_tools() {
    let kit = build_test_kit();
    let mut registry = ToolRegistry::new();
    kit.register_read_only(&mut registry);

    for name in MUTATING_TOOL_NAMES {
        assert!(
            registry.get(name).is_none(),
            "mutating tool '{}' must not appear in a read-only registry built for /review",
            name,
        );
    }
}

#[test]
fn k17_read_only_registry_contains_expected_read_tools() {
    let kit = build_test_kit();
    let mut registry = ToolRegistry::new();
    kit.register_read_only(&mut registry);

    for name in EXPECTED_READ_ONLY_TOOL_NAMES {
        assert!(
            registry.get(name).is_some(),
            "read-only tool '{}' missing from registry — review pass would be effectively useless",
            name,
        );
    }
}

#[test]
fn k17_register_all_includes_mutating_tools_baseline() {
    // Baseline check: the same kit, with `register_all`, DOES produce mutating tools.
    // This guards against the K17 read-only assertions passing trivially because
    // the mutating tools were renamed or removed entirely from the codebase —
    // in that case this test fails and the maintainer must update both lists.
    let kit = build_test_kit();
    let mut registry = ToolRegistry::new();
    kit.register_all(&mut registry);

    let mut found_at_least_one = false;
    for name in MUTATING_TOOL_NAMES {
        if registry.get(name).is_some() {
            found_at_least_one = true;
            break;
        }
    }
    assert!(
        found_at_least_one,
        "MUTATING_TOOL_NAMES list is stale — none of {:?} are registered by register_all; \
         update both MUTATING_TOOL_NAMES and EXPECTED_READ_ONLY_TOOL_NAMES",
        MUTATING_TOOL_NAMES,
    );
}
